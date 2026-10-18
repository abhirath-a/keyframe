use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Color32, TextureHandle, TextureOptions, ViewportCommand};

use crate::config::Config;
use crate::help::show_help_overlay;
use crate::keybinds::{resolve_keybinds, triggered_action, Action, KeyCombo};
use crate::notes::SpeakerNotes;
use crate::pdf_worker::{PageCache, PdfWorker};

pub struct PresenterApp {
    config: Config,
    keybinds: HashMap<Action, Vec<KeyCombo>>,

    worker: Option<PdfWorker>,
    cache: PageCache,
    textures: HashMap<usize, TextureHandle>,

    current: usize,
    show_help: bool,
    fullscreen: bool,

    load_error: Option<String>,
    document_path: Option<PathBuf>,
    show_settings: bool,
    settings_draft: Config,
    status: Option<String>,
    notes: Option<SpeakerNotes>,
    show_presenter: bool,
    notes_edited_at: Option<Instant>,
    notes_error: Option<String>,
    window_title: String,
}

impl PresenterApp {
    pub fn new(initial_path: Option<PathBuf>, config: Config, start_fullscreen: bool) -> Self {
        let keybinds = resolve_keybinds(&config);
        let mut app = Self {
            settings_draft: config.clone(),
            config,
            keybinds,
            worker: None,
            cache: PageCache::new(),
            textures: HashMap::new(),
            current: 0,
            show_help: false,
            fullscreen: start_fullscreen,
            load_error: None,
            document_path: None,
            show_settings: false,
            status: None,
            notes: None,
            show_presenter: false,
            notes_edited_at: None,
            notes_error: None,
            window_title: "Keyframe".into(),
        };
        if let Some(path) = initial_path {
            app.open_document(path);
        }
        app
    }

    fn open_document(&mut self, path: PathBuf) {
        if !self.save_notes() {
            return;
        }
        match PdfWorker::open(&path, self.config.render_dpi) {
            Ok(worker) => {
                if worker.page_count == 0 {
                    self.load_error = Some("This document has no slides.".into());
                    return;
                }
                let notes = SpeakerNotes::load(&path, &worker.labels, &worker.native_notes);
                self.notes_error = notes
                    .as_ref()
                    .err()
                    .map(|e| format!("Could not load speaker notes: {e}"));
                self.notes = notes.ok();
                self.notes_edited_at = None;
                self.worker = Some(worker);
                self.cache = PageCache::new();
                self.textures.clear();
                self.current = 0;
                self.load_error = None;
                self.document_path = Some(path.clone());
                self.config.recent_documents.retain(|p| p != &path);
                self.config.recent_documents.insert(0, path);
                self.config.recent_documents.truncate(8);
                self.save_config();
            }
            Err(e) => {
                self.load_error = Some(e.to_string());
            }
        }
    }

    fn save_config(&mut self) {
        self.status = self
            .config
            .save()
            .err()
            .map(|e| format!("Could not save preferences: {e}"));
    }

    fn pick_document(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open a presentation")
            .add_filter("PDF presentation", &["pdf"])
            .pick_file()
        {
            self.open_document(path);
        }
    }

    fn close_document(&mut self) {
        if !self.save_notes() {
            return;
        }
        self.worker = None;
        self.notes = None;
        self.notes_error = None;
        self.show_presenter = false;
        self.document_path = None;
        self.cache = PageCache::new();
        self.textures.clear();
        self.current = 0;
        self.load_error = None;
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, fullscreen: bool) {
        self.fullscreen = fullscreen;
        ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            ViewportCommand::Fullscreen(fullscreen),
        );
    }

    fn page_count(&self) -> usize {
        self.worker.as_ref().map(|w| w.page_count).unwrap_or(0)
    }

    fn go_to(&mut self, page: usize) {
        let count = self.page_count();
        if count == 0 {
            return;
        }
        self.current = page.min(count - 1);
    }

    fn handle_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::NextPage => self.go_to(self.current + 1),
            Action::PrevPage => self.go_to(self.current.saturating_sub(1)),
            Action::FirstPage => self.go_to(0),
            Action::LastPage => self.go_to(self.page_count().saturating_sub(1)),
            Action::ToggleHelp => self.show_help = !self.show_help,
            Action::ToggleFullscreen => {
                self.set_fullscreen(ctx, !self.fullscreen);
            }
            Action::Quit => {
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Close)
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        if !ctx.wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
                self.pick_document();
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                if self.show_settings {
                    self.show_settings = false;
                } else if self.show_help {
                    self.show_help = false;
                } else if self.fullscreen {
                    self.set_fullscreen(ctx, false);
                }
            } else if !self.show_settings {
                let action = ctx.input(|i| triggered_action(&self.keybinds, i));
                if let Some(action) = action {
                    self.handle_action(action, ctx);
                }
            }
        }

        // Drag-and-drop a PDF onto the window to open it (handy since
        // double-clicking the app on macOS/Windows won't pass a CLI arg).
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.first() {
            if let Some(path) = &file.path {
                self.open_document(path.clone());
            }
        }
    }

    fn refresh_cache_and_textures(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.worker else { return };
        worker.set_repaint_context(ctx);

        let radius = self
            .config
            .cache_window
            .max(usize::from(self.show_presenter));
        let budget = self.config.cache_memory_mb.clamp(4, 128) * 1024 * 1024;
        // Prune before draining so obsolete results never become GPU textures.
        self.cache
            .prepare_window(worker, self.current, radius, budget);

        for (page, image) in self.cache.drain_finished(worker) {
            let tex = ctx.load_texture(format!("page-{page}"), image, TextureOptions::LINEAR);
            self.textures.insert(page, tex);
            self.cache.mark_uploaded(page);
        }

        self.cache
            .refresh_with_budget(worker, self.current, radius, budget);

        // Drop textures for pages that fell out of the cache window.
        let keep = self.cache.cached_pages();
        self.textures.retain(|p, _| keep.contains(p));
    }

    fn render_page_area(&mut self, ui: &mut egui::Ui) {
        let Some(_worker) = &self.worker else {
            self.welcome(ui);
            return;
        };

        ui.centered_and_justified(|ui| {
            if let Some(tex) = self.textures.get(&self.current) {
                let uv = self.slide_region();
                let img_size = tex.size_vec2() * uv.size();
                let avail = ui.available_size();
                let scale = (avail.x / img_size.x).min(avail.y / img_size.y).min(1.0e6);
                let draw_size = img_size * scale.max(0.0001);
                ui.add(egui::Image::new((tex.id(), draw_size)).uv(uv));
            } else if let Some(error) = self.cache.errors.get(&self.current) {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    format!("Could not render slide {}: {error}", self.current + 1),
                );
            } else {
                ui.spinner();
            }
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        let recent = self.config.recent_documents.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space((ui.available_height() * 0.12).max(20.0));
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("KEYFRAME")
                        .size(14.0)
                        .color(Color32::from_rgb(130, 170, 240)),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Your slides. Center stage.")
                        .size(32.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label("Open a PDF to preview your deck, then present it fullscreen.");
                ui.add_space(24.0);
                if ui
                    .add_sized(
                        [200.0, 48.0],
                        egui::Button::new("Open PDF…").fill(Color32::from_rgb(48, 86, 150)),
                    )
                    .clicked()
                {
                    self.pick_document();
                }
                ui.add_space(10.0);
                ui.weak("Or drop a PDF anywhere in this window · Ctrl/Cmd+O");
                if !recent.is_empty() {
                    ui.add_space(32.0);
                    ui.label(egui::RichText::new("RECENT PRESENTATIONS").small().strong());
                    ui.add_space(8.0);
                    for path in recent {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if ui
                            .add_sized([360.0, 36.0], egui::Button::new(name.as_ref()))
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            self.open_document(path);
                        }
                    }
                }
            });
        });
    }

    fn desktop_chrome(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label(egui::RichText::new("Keyframe").strong());
                ui.separator();
                ui.menu_button("File", |ui| {
                    if ui.button("Open PDF…    Ctrl/Cmd+O").clicked() {
                        ui.close_menu();
                        self.pick_document();
                    }
                    ui.menu_button("Open recent", |ui| {
                        if self.config.recent_documents.is_empty() {
                            ui.weak("No recent documents");
                        }
                        for path in self.config.recent_documents.clone() {
                            if ui
                                .button(path.file_name().unwrap_or_default().to_string_lossy())
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                ui.close_menu();
                                self.open_document(path);
                            }
                        }
                        ui.separator();
                        if ui.button("Clear recent documents").clicked() {
                            self.config.recent_documents.clear();
                            self.save_config();
                            ui.close_menu();
                        }
                    });
                    if ui
                        .add_enabled(self.worker.is_some(), egui::Button::new("Close document"))
                        .clicked()
                    {
                        self.close_document();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
                if ui.button("Settings").clicked() {
                    self.settings_draft = self.config.clone();
                    self.show_settings = true;
                }
                if ui.button("Shortcuts").clicked() {
                    self.show_help = !self.show_help;
                }
                if ui
                    .add_enabled(self.worker.is_some(), egui::Button::new("Speaker notes"))
                    .clicked()
                {
                    self.show_presenter = !self.show_presenter;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(self.worker.is_some(), egui::Button::new("Present  ▶"))
                        .clicked()
                    {
                        self.show_presenter = true;
                        self.set_fullscreen(ctx, true);
                    }
                    if let Some(path) = &self.document_path {
                        ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                            .on_hover_text(path.display().to_string());
                    }
                });
            });
        });
        if self.worker.is_some() {
            egui::TopBottomPanel::bottom("navigation").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(self.current > 0, egui::Button::new("|◀ First"))
                        .clicked()
                    {
                        self.go_to(0);
                    }
                    if ui
                        .add_enabled(self.current > 0, egui::Button::new("◀ Previous"))
                        .clicked()
                    {
                        self.go_to(self.current.saturating_sub(1));
                    }
                    ui.label("Slide");
                    let mut page = self.current + 1;
                    if ui
                        .add(egui::DragValue::new(&mut page).range(1..=self.page_count()))
                        .changed()
                    {
                        self.go_to(page - 1);
                    }
                    ui.label(format!("of {}", self.page_count()));
                    if ui
                        .add_enabled(
                            self.current + 1 < self.page_count(),
                            egui::Button::new("Next ▶"),
                        )
                        .clicked()
                    {
                        self.go_to(self.current + 1);
                    }
                    if ui
                        .add_enabled(
                            self.current + 1 < self.page_count(),
                            egui::Button::new("Last ▶|"),
                        )
                        .clicked()
                    {
                        self.go_to(self.page_count().saturating_sub(1));
                    }
                });
            });
        }
    }

    fn save_notes(&mut self) -> bool {
        if let Some(notes) = &mut self.notes {
            if let Err(error) = notes.save() {
                self.notes_error = Some(format!(
                    "Could not save notes: {error}. Your edits are still in this window."
                ));
                self.notes_edited_at = None;
                self.show_presenter = true;
                return false;
            }
            self.notes_error = None;
        }
        self.notes_edited_at = None;
        true
    }

    fn slide_region(&self) -> egui::Rect {
        Self::page_regions(self.notes.as_ref().map_or("none", |n| n.beamer_position())).0
    }

    fn page_regions(position: &str) -> (egui::Rect, Option<egui::Rect>) {
        let rect =
            |x0, y0, x1, y1| egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1));
        match position {
            "right" => (rect(0.0, 0.0, 0.5, 1.0), Some(rect(0.5, 0.0, 1.0, 1.0))),
            "left" => (rect(0.5, 0.0, 1.0, 1.0), Some(rect(0.0, 0.0, 0.5, 1.0))),
            "top" => (rect(0.0, 0.5, 1.0, 1.0), Some(rect(0.0, 0.0, 1.0, 0.5))),
            "bottom" => (rect(0.0, 0.0, 1.0, 0.5), Some(rect(0.0, 0.5, 1.0, 1.0))),
            _ => (rect(0.0, 0.0, 1.0, 1.0), None),
        }
    }

    fn preview_slide(&self, ui: &mut egui::Ui, page: usize, size: egui::Vec2, uv: egui::Rect) {
        ui.allocate_ui_with_layout(
            size,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                if let Some(texture) = self.textures.get(&page) {
                    let pixels = texture.size_vec2() * uv.size();
                    let scale = (size.x / pixels.x).min(size.y / pixels.y).max(0.0001);
                    ui.add(egui::Image::new((texture.id(), pixels * scale)).uv(uv));
                } else if page >= self.page_count() {
                    ui.weak("End of presentation");
                } else if let Some(error) = self.cache.errors.get(&page) {
                    ui.colored_label(Color32::LIGHT_RED, error);
                } else if !self.cache.wants_page(page) {
                    ui.weak("Preview exceeds the slide-cache budget. Increase it in Settings.");
                } else {
                    ui.spinner();
                }
            },
        );
    }

    fn presenter_content(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(format!(
                "Slide {} of {}",
                self.current + 1,
                self.page_count()
            ));
            if ui
                .add_enabled(self.current > 0, egui::Button::new("Previous"))
                .clicked()
            {
                self.go_to(self.current.saturating_sub(1));
            }
            if ui
                .add_enabled(
                    self.current + 1 < self.page_count(),
                    egui::Button::new("Next"),
                )
                .clicked()
            {
                self.go_to(self.current + 1);
            }
            let mut page = self.current + 1;
            if ui
                .add(egui::DragValue::new(&mut page).range(1..=self.page_count()))
                .changed()
            {
                self.go_to(page.saturating_sub(1));
            }
            if ui
                .button(if self.fullscreen {
                    "Leave presentation"
                } else {
                    "Present audience window"
                })
                .clicked()
            {
                self.set_fullscreen(ui.ctx(), !self.fullscreen);
            }
        });
        let width = ((ui.available_width() - 16.0) / 2.0).max(100.0);
        let height = (ui.available_height() * 0.32).clamp(80.0, 230.0);
        let region = self.slide_region();
        ui.columns(2, |columns| {
            columns[0].label("CURRENT · AUDIENCE");
            self.preview_slide(
                &mut columns[0],
                self.current,
                egui::vec2(width, height),
                region,
            );
            columns[1].label("NEXT");
            self.preview_slide(
                &mut columns[1],
                self.current + 1,
                egui::vec2(width, height),
                region,
            );
        });
        ui.separator();
        if let Some(error) = &self.notes_error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        if self.notes.is_none() {
            ui.label("Fix the sidecar file, then reopen the PDF to load notes. The existing notes file is untouched.");
            return;
        }
        let mut position = self.notes.as_ref().unwrap().beamer_position().to_owned();
        ui.horizontal_wrapped(|ui| {
            ui.heading("Speaker notes");
            if ui.button("Save notes  Ctrl/Cmd+S").clicked() {
                self.save_notes();
            }
            let notes = self.notes.as_ref().unwrap();
            ui.weak(if notes.dirty {
                "Unsaved edits"
            } else {
                "Saved"
            });
            egui::ComboBox::from_id_source("beamer_note_position")
                .selected_text(format!("Beamer notes: {position}"))
                .show_ui(ui, |ui| {
                    for p in ["none", "right", "left", "top", "bottom"] {
                        ui.selectable_value(&mut position, p.to_owned(), p);
                    }
                });
        });
        let notes = self.notes.as_mut().unwrap();
        if position != notes.beamer_position() {
            notes.set_beamer_position(&position);
            self.notes_edited_at = Some(Instant::now());
            self.notes_error = None;
        }
        ui.label(
            egui::RichText::new(notes.path.display().to_string())
                .small()
                .weak(),
        );
        if let Some(region) = Self::page_regions(&position).1 {
            let size = egui::vec2(
                ui.available_width(),
                (ui.available_height() * 0.5).max(80.0),
            );
            self.preview_slide(ui, self.current, size, region);
        }
        let notes = self.notes.as_mut().unwrap();
        egui::ScrollArea::vertical()
            .id_source(("notes_editor", self.current))
            .show(ui, |ui| {
                if ui
                    .add_sized(
                        [ui.available_width(), ui.available_height().max(100.0)],
                        egui::TextEdit::multiline(&mut notes.text[self.current])
                            .id(egui::Id::new(("slide_note", self.current)))
                            .font(egui::FontId::proportional(20.0))
                            .hint_text("Add notes for this slide…")
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    notes.dirty = true;
                    self.notes_edited_at = Some(Instant::now());
                    self.notes_error = None;
                }
            });
    }

    fn presenter_window(&mut self, ctx: &egui::Context) {
        if !self.show_presenter || self.worker.is_none() {
            return;
        }
        let previous_page = self.current;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("speaker_notes"),
            egui::ViewportBuilder::default()
                .with_title("Keyframe — Speaker notes")
                .with_inner_size([960.0, 760.0])
                .with_min_inner_size([680.0, 480.0]),
            |ctx, class| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    if self.save_notes() {
                        self.show_presenter = false;
                    } else {
                        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                    }
                    return;
                }
                // An embedded fallback shares the parent's input; dispatch it once.
                if class != egui::ViewportClass::Embedded {
                    self.handle_presenter_input(ctx);
                }
                self.refresh_cache_and_textures(ctx);
                if class == egui::ViewportClass::Embedded {
                    egui::Window::new("Speaker notes").show(ctx, |ui| self.presenter_content(ui));
                } else {
                    egui::CentralPanel::default().show(ctx, |ui| self.presenter_content(ui));
                }
            },
        );
        if previous_page != self.current {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            ctx.request_repaint_of(egui::ViewportId::from_hash_of("speaker_notes"));
        }
    }

    fn handle_presenter_input(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save_notes();
        }
        if !ctx.wants_keyboard_input() {
            if let Some(action) = ctx.input(|i| triggered_action(&self.keybinds, i)) {
                self.handle_action(action, ctx);
            }
        }
    }

    fn settings(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Presentation");
                ui.add_space(8.0);
                ui.add(
                    egui::Slider::new(&mut self.settings_draft.render_dpi, 72.0..=300.0)
                        .text("Render DPI"),
                );
                ui.add(
                    egui::Slider::new(&mut self.settings_draft.cache_window, 0..=5)
                        .text("Prefetch slides"),
                );
                ui.add(
                    egui::Slider::new(&mut self.settings_draft.cache_memory_mb, 4..=128)
                        .text("Slide cache (MiB)"),
                );
                ui.weak(
                    "The current slide keeps its full quality even if it exceeds the cache budget.",
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label("Background");
                    ui.color_edit_button_srgb(&mut self.settings_draft.background_color);
                });
                ui.checkbox(
                    &mut self.settings_draft.start_fullscreen,
                    "Start in fullscreen",
                );
                ui.checkbox(
                    &mut self.settings_draft.vsync,
                    "Vsync (applies after restart)",
                );
                ui.add_space(12.0);
                ui.weak("Keyboard bindings can be customized in config.toml.");
                if let Some(path) = Config::config_path() {
                    ui.label(egui::RichText::new(path.display().to_string()).small());
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    save = ui.button("Save settings").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        self.show_settings = open && !cancel;
        if save {
            let rerender = self.config.render_dpi != self.settings_draft.render_dpi;
            self.settings_draft.recent_documents = self.config.recent_documents.clone();
            // Validate the new renderer before replacing the working document.
            if rerender {
                if let Some(path) = &self.document_path {
                    match PdfWorker::open(path, self.settings_draft.render_dpi) {
                        Ok(worker) => {
                            self.worker = Some(worker);
                            self.cache = PageCache::new();
                            self.textures.clear();
                        }
                        Err(e) => {
                            self.status = Some(e.to_string());
                            return;
                        }
                    }
                }
            }
            self.config = self.settings_draft.clone();
            self.save_config();
            self.show_settings = false;
        }
    }
}

impl eframe::App for PresenterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.save_notes() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
        if let Some(edited) = self.notes_edited_at {
            if edited.elapsed() >= Duration::from_millis(800) {
                self.save_notes();
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }
        self.handle_input(ctx);
        self.refresh_cache_and_textures(ctx);
        self.presenter_window(ctx);

        let title = self
            .document_path
            .as_ref()
            .map(|p| {
                format!(
                    "{} — Keyframe",
                    p.file_name().unwrap_or_default().to_string_lossy()
                )
            })
            .unwrap_or_else(|| "Keyframe".into());
        if title != self.window_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        if !self.fullscreen {
            self.desktop_chrome(ctx);
        }

        if self.load_error.is_some() || self.status.is_some() {
            egui::TopBottomPanel::top("error_notice").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        self.load_error
                            .as_deref()
                            .or(self.status.as_deref())
                            .unwrap_or_default(),
                    );
                    if ui.button("Dismiss").clicked() {
                        self.load_error = None;
                        self.status = None;
                    }
                    if ui.button("Open another PDF…").clicked() {
                        self.pick_document();
                    }
                });
            });
        }

        let bg = self.config.background_color;
        let frame = egui::Frame::none().fill(Color32::from_rgb(bg[0], bg[1], bg[2]));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            self.render_page_area(ui);
        });

        if self.show_help {
            show_help_overlay(ctx, &self.keybinds, &mut self.show_help);
        }
        if self.show_settings {
            self.settings(ctx);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_notes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut PresenterApp, key: egui::Key) -> egui::FullOutput {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run(input, |ctx| app.handle_input(ctx))
    }

    #[test]
    fn escape_exits_presentation_without_closing_the_app() {
        let mut app = PresenterApp::new(None, Config::default(), true);
        let output = press(&mut app, egui::Key::Escape);
        assert!(!app.fullscreen);
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&ViewportCommand::Fullscreen(false)));
        assert!(!commands.contains(&ViewportCommand::Close));
    }

    #[test]
    fn escape_dismisses_settings_before_leaving_presentation() {
        let mut app = PresenterApp::new(None, Config::default(), true);
        app.show_settings = true;
        press(&mut app, egui::Key::Escape);
        assert!(!app.show_settings);
        assert!(app.fullscreen);
    }

    #[test]
    fn settings_prevents_quit_shortcut() {
        let mut app = PresenterApp::new(None, Config::default(), false);
        app.show_settings = true;
        let output = press(&mut app, egui::Key::Q);
        assert!(!output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::Close));
    }

    #[test]
    fn both_windows_navigate_the_same_slide_and_note() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PresenterApp::new(None, Config::default(), false);
        let worker = PdfWorker::test_stub(3);
        app.notes =
            Some(SpeakerNotes::load(&dir.path().join("deck.pdf"), &worker.labels, &[]).unwrap());
        app.worker = Some(worker);
        app.notes.as_mut().unwrap().text = vec!["First".into(), "Second".into(), "Third".into()];
        press(&mut app, egui::Key::ArrowRight);
        assert_eq!(app.current, 1);
        assert_eq!(app.notes.as_ref().unwrap().text[app.current], "Second");
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input, |ctx| app.handle_presenter_input(ctx));
        assert_eq!(app.current, 2);
        assert_eq!(app.notes.as_ref().unwrap().text[app.current], "Third");
        press(&mut app, egui::Key::ArrowLeft);
        assert_eq!(app.current, 1);
    }

    #[test]
    fn beamer_regions_separate_slides_and_notes() {
        for position in ["right", "left", "top", "bottom"] {
            let (slide, notes) = PresenterApp::page_regions(position);
            let notes = notes.unwrap();
            assert_eq!(slide.area(), 0.5);
            assert_eq!(notes.area(), 0.5);
            assert_eq!(slide.intersect(notes).area(), 0.0);
        }
    }

    #[test]
    fn presenter_ui_paints_the_note_matching_the_current_slide() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = PresenterApp::new(None, Config::default(), false);
        let worker = PdfWorker::test_stub(2);
        app.notes =
            Some(SpeakerNotes::load(&dir.path().join("deck.pdf"), &worker.labels, &[]).unwrap());
        app.worker = Some(worker);
        app.notes.as_mut().unwrap().text = vec!["Opening remarks".into(), "Closing remarks".into()];
        let ctx = egui::Context::default();
        let render = |app: &mut PresenterApp| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 760.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.presenter_content(ui));
            });
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let first = render(&mut app);
        assert!(first.contains("Opening remarks"));
        app.go_to(1);
        let second = render(&mut app);
        assert!(second.contains("Closing remarks"));
        assert!(!second.contains("Opening remarks"));
    }
}
