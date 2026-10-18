use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// Keep the original metadata so editing a note never erases pdfpc settings.
pub struct SpeakerNotes {
    pub path: PathBuf,
    metadata: Value,
    pub text: Vec<String>,
    labels: Vec<String>,
    pub dirty: bool,
    legacy: bool,
    original_source: Option<String>,
    initial_text: Vec<String>,
}

impl SpeakerNotes {
    pub fn load(pdf: &Path, labels: &[String], native: &[String]) -> Result<Self> {
        let path = pdf.with_extension("pdfpc");
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => Some(source),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("Cannot read {}", path.display())),
        };
        Self::parse(path, source.as_deref(), labels, native)
    }

    fn parse(
        path: PathBuf,
        source: Option<&str>,
        labels: &[String],
        native: &[String],
    ) -> Result<Self> {
        let mut notes = Self {
            path,
            metadata: json!({"pdfpcFormat": 2, "disableMarkdown": true, "pages": []}),
            text: labels
                .iter()
                .enumerate()
                .map(|(i, _)| native.get(i).cloned().unwrap_or_default())
                .collect(),
            labels: labels.to_vec(),
            dirty: false,
            legacy: false,
            original_source: source.map(str::to_owned),
            initial_text: Vec::new(),
        };
        if let Some(source) = source {
            if source.trim_start().starts_with('{') {
                let metadata: Value = serde_json::from_str(source).context("Invalid pdfpc JSON")?;
                if !metadata.is_object()
                    || metadata["pdfpcFormat"]
                        .as_u64()
                        .is_none_or(|v| v > 2 || v == 0)
                {
                    bail!("Unsupported pdfpc format; the notes file will not be overwritten");
                }
                if let Some(pages) = metadata.get("pages") {
                    let pages = pages.as_array().context("pdfpc pages must be an array")?;
                    for page in pages {
                        let idx = notes
                            .resolve_page(page)
                            .context("Invalid pdfpc page index")?;
                        if let Some(note) = page.get("note") {
                            let note = note.as_str().context("pdfpc note must be text")?;
                            if let Some(text) = notes.text.get_mut(idx) {
                                *text = note.to_owned();
                            }
                        }
                    }
                }
                notes.metadata = metadata;
            } else {
                notes.parse_legacy(source)?;
                notes.legacy = true;
            }
        }
        notes.initial_text = notes.text.clone();
        Ok(notes)
    }

    fn overlay(&self, idx: usize) -> usize {
        let start = self.labels[..=idx]
            .iter()
            .rposition(|label| label != &self.labels[idx])
            .map_or(0, |p| p + 1);
        idx - start
    }

    fn resolve_page(&self, page: &Value) -> Option<usize> {
        let idx = usize::try_from(page["idx"].as_u64()?).ok()?;
        let label = page["label"].as_str().unwrap_or("");
        let overlay = page["overlay"].as_u64().unwrap_or(0) as usize;
        if !label.is_empty() {
            if self.labels.get(idx).is_some_and(|l| l == label) && self.overlay(idx) == overlay {
                return Some(idx);
            }
            if let Some(first) = self.labels.iter().position(|l| l == label) {
                if let Some(target) = first.checked_add(overlay) {
                    if self.labels.get(target).is_some_and(|l| l == label) {
                        return Some(target);
                    }
                }
            }
        }
        Some(idx)
    }

    fn parse_legacy(&mut self, source: &str) -> Result<()> {
        let mut sections: BTreeMap<String, String> = BTreeMap::new();
        let mut section = String::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[')
                && trimmed.ends_with(']')
                && trimmed[1..trimmed.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_alphabetic() || c == '_')
            {
                section = trimmed[1..trimmed.len() - 1].to_owned();
            } else {
                sections
                    .entry(section.clone())
                    .or_default()
                    .push_str(&format!("{line}\n"));
            }
        }
        if !sections.contains_key("notes") {
            bail!("Unrecognized legacy pdfpc file");
        }
        for (old, new) in [
            ("duration", "duration"),
            ("last_minutes", "lastMinutes"),
            ("font_size", "noteFontSize"),
            ("last_saved_slide", "savedSlide"),
            ("end_user_slide", "endSlide"),
        ] {
            if let Some(value) = sections.get(old) {
                self.metadata[new] = json!(value.trim().parse::<i64>()?);
            }
        }
        for (old, new) in [
            ("start_time", "startTime"),
            ("end_time", "endTime"),
            ("notes_position", "beamerNotePosition"),
        ] {
            if let Some(value) = sections.get(old) {
                self.metadata[new] = json!(value.trim());
            }
        }
        let skipped: Vec<usize> = sections
            .get("skip")
            .into_iter()
            .flat_map(|s| s.split(','))
            .filter_map(|s| s.trim().parse::<usize>().ok()?.checked_sub(1))
            .collect();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for i in 0..self.labels.len() {
            if i == 0 || (self.labels[i] != self.labels[i - 1] && !skipped.contains(&i)) {
                groups.push(Vec::new());
            }
            groups.last_mut().unwrap().push(i);
        }
        for block in sections["notes"].split("### ").skip(1) {
            if let Some((header, text)) = block.split_once('\n') {
                let group = header
                    .trim()
                    .parse::<usize>()?
                    .checked_sub(1)
                    .context("Legacy slide numbers start at 1")?;
                let text = text
                    .trim_end()
                    .replace("\\[", "[")
                    .replace("\\]", "]")
                    .replace("\\#", "#");
                if let Some(pages) = groups.get(group) {
                    for &idx in pages {
                        self.text[idx] = text.clone();
                    }
                }
            }
        }
        for idx in skipped {
            if idx < self.labels.len() {
                let page = json!({"idx": idx, "label": self.labels[idx], "overlay": self.overlay(idx), "forcedOverlay": true});
                self.metadata["pages"].as_array_mut().unwrap().push(page);
            }
        }
        Ok(())
    }

    pub fn beamer_position(&self) -> &str {
        self.metadata["beamerNotePosition"]
            .as_str()
            .unwrap_or("none")
    }

    pub fn set_beamer_position(&mut self, position: &str) {
        if self.beamer_position() != position {
            self.metadata["beamerNotePosition"] = json!(position);
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let current_source = match std::fs::read_to_string(&self.path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if current_source != self.original_source {
            bail!("The pdfpc file changed outside Keyframe. Copy your edits before reopening the document");
        }
        let mut metadata = self.metadata.clone();
        metadata["pdfpcFormat"] = json!(2);
        if metadata.get("pages").is_none() {
            metadata["pages"] = json!([]);
        }
        let mut pages = metadata["pages"]
            .as_array()
            .context("Invalid pages metadata")?
            .clone();
        for (idx, text) in self.text.iter().enumerate() {
            if let Some(page) = pages.iter_mut().find(|p| self.resolve_page(p) == Some(idx)) {
                page["idx"] = json!(idx);
                page["label"] = json!(self.labels[idx]);
                page["overlay"] = json!(self.overlay(idx));
                page["note"] = json!(text);
            } else if !text.is_empty() || text != &self.initial_text[idx] {
                pages.push(json!({"idx": idx, "label": self.labels[idx], "overlay": self.overlay(idx), "note": text}));
            }
        }
        metadata["pages"] = json!(pages);
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, &metadata)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        if self.legacy {
            let mut backup = self.path.as_os_str().to_os_string();
            backup.push("~");
            // Keep any existing backup intact.
            let mut suffix = 0;
            let mut backup_file = loop {
                let mut candidate = backup.clone();
                if suffix > 0 {
                    candidate.push(format!(".{suffix}"));
                }
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(candidate)
                {
                    Ok(file) => break file,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        suffix += 1;
                    }
                    Err(e) => return Err(e.into()),
                }
            };
            backup_file.write_all(&std::fs::read(&self.path)?)?;
            backup_file.sync_all()?;
        }
        file.persist(&self.path).map_err(|e| e.error)?;
        self.original_source = Some(serde_json::to_string_pretty(&metadata)? + "\n");
        self.metadata = metadata;
        self.initial_text = self.text.clone();
        self.dirty = false;
        self.legacy = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn labels() -> Vec<String> {
        vec!["1".into(), "2".into(), "2".into()]
    }
    #[test]
    fn pdfpc_roundtrip_preserves_settings_and_overlays() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.pdfpc");
        let source = r#"{"pdfpcFormat":2,"duration":20,"disableMarkdown":false,"pages":[{"idx":2,"label":"2","overlay":1,"note":"**Hi**","hidden":true}]}"#;
        std::fs::write(&path, source).unwrap();
        let mut notes = SpeakerNotes::parse(path.clone(), Some(source), &labels(), &[]).unwrap();
        assert_eq!(notes.text[2], "**Hi**");
        notes.text[0] = "Introduction\nSecond line".into();
        notes.dirty = true;
        notes.save().unwrap();
        let data: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(data["duration"], 20);
        assert_eq!(data["disableMarkdown"], false);
        assert_eq!(data["pages"][0]["hidden"], true);
        assert_eq!(data["pages"][0]["overlay"], 1);
        let restored = SpeakerNotes::load(&path.with_extension("pdf"), &labels(), &[]).unwrap();
        assert_eq!(restored.text, notes.text);
    }
    #[test]
    fn legacy_notes_apply_to_all_overlays_and_keep_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.pdfpc");
        let source = "[duration]\n15\n[notes]\n### 1\nHello\n### 2\nUse \\[demo\\]";
        std::fs::write(&path, source).unwrap();
        let mut notes = SpeakerNotes::parse(path.clone(), Some(source), &labels(), &[]).unwrap();
        assert_eq!(notes.text, ["Hello", "Use [demo]", "Use [demo]"]);
        notes.dirty = true;
        notes.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(path.with_file_name("deck.pdfpc~")).unwrap(),
            source
        );
    }
    #[test]
    fn shifted_page_indices_resolve_by_label_and_overlay() {
        let source =
            r#"{"pdfpcFormat":2,"pages":[{"idx":0,"label":"2","overlay":1,"note":"Overlay"}]}"#;
        let notes = SpeakerNotes::parse("deck.pdfpc".into(), Some(source), &labels(), &[]).unwrap();
        assert_eq!(notes.text[2], "Overlay");
    }
    #[test]
    fn unsupported_or_corrupt_sidecars_are_rejected() {
        for source in [r#"{"pdfpcFormat":99}"#, "{oops", "random data"] {
            assert!(
                SpeakerNotes::parse("deck.pdfpc".into(), Some(source), &labels(), &[]).is_err()
            );
        }
    }

    #[test]
    fn external_edits_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.pdfpc");
        let mut notes = SpeakerNotes::parse(path.clone(), None, &labels(), &[]).unwrap();
        notes.text[0] = "My edits".into();
        notes.dirty = true;
        std::fs::write(&path, "external changes").unwrap();
        assert!(notes.save().is_err());
        assert!(notes.dirty);
        assert_eq!(notes.text[0], "My edits");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "external changes");
    }

    #[test]
    fn clearing_native_notes_survives_reopening_and_repeated_saves() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("deck.pdf");
        let native = vec!["Annotation note".to_owned()];
        let mut notes = SpeakerNotes::load(&pdf, &labels(), &native).unwrap();
        notes.text[0].clear();
        notes.dirty = true;
        notes.save().unwrap();
        notes.text[1] = "Another note".into();
        notes.dirty = true;
        notes.save().unwrap();
        let loaded = SpeakerNotes::load(&pdf, &labels(), &native).unwrap();
        assert!(loaded.text[0].is_empty());
        assert_eq!(loaded.text[1], "Another note");
    }
}
