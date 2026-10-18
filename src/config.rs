use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// User-facing configuration, loaded from (and saveable to) a TOML file in
/// the platform config directory:
///   Linux:   ~/.config/keyframe/config.toml
///   macOS:   ~/Library/Application Support/dev.keyframe.keyframe/config.toml
///   Windows: %APPDATA%\keyframe\keyframe\config\config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// How many pages ahead *and* behind the current page to keep rendered
    /// and cached. `1` (the default) means: current, previous, and next.
    pub cache_window: usize,
    /// RGBA texture budget for prefetched pages. The current slide is always kept.
    pub cache_memory_mb: usize,

    /// Rendering resolution in dots-per-inch. Higher = crisper on large /
    /// high-DPI displays, but slower to render and more memory per page.
    pub render_dpi: f32,

    /// Background color shown around/behind the page (letterboxing), as
    /// [r, g, b] in 0..=255.
    pub background_color: [u8; 3],

    /// Start the presenter in fullscreen (typical for a "presenter" app).
    pub start_fullscreen: bool,

    /// Enable vsync (recommended; avoids tearing and unnecessary redraws).
    pub vsync: bool,

    /// Optional keybind overrides. Keys are action names (see `Action`'s
    /// `config_name()`), values are lists of key strings, e.g.:
    ///
    /// [keybinds]
    /// next_page = ["Right", "Space", "j", "l"]
    /// toggle_help = ["?", "F1"]
    ///
    /// Any action not listed here keeps its built-in default (vim + normie)
    /// bindings. Listing an action replaces its defaults entirely.
    pub keybinds: HashMap<String, Vec<String>>,
    /// Most recently opened documents, newest first.
    pub recent_documents: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_window: 1,
            cache_memory_mb: 24,
            render_dpi: 150.0,
            background_color: [18, 18, 18],
            start_fullscreen: false,
            vsync: true,
            keybinds: HashMap::new(),
            recent_documents: Vec::new(),
        }
    }
}

impl Config {
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not locate the configuration directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("dev", "keyframe", "keyframe")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Load config from disk, falling back to defaults if missing/invalid.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(cfg) => cfg,
                Err(err) => {
                    eprintln!("warning: failed to parse {}: {err}", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Write a config file with defaults + comments, if one doesn't exist
    /// yet. Handy so users have something to edit.
    pub fn ensure_default_file(&self) -> anyhow::Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
