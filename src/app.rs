use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;
use egui::{Color32, TextureHandle, TextureOptions, ViewportCommand};

use crate::config::Config;
use crate::help::show_help_overlay;
use crate::keybinds::{resolve_keybinds, triggered_action, Action, KeyCombo};
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
}

impl PresenterApp {
    pub fn new(initial_path: Option<PathBuf>, config: Config, start_fullscreen: bool) -> Self {
        let keybinds = resolve_keybinds(&config);
        let mut app = Self {
            config,
            keybinds,
            worker: None,
            cache: PageCache::new(),
            textures: HashMap::new(),
            current: 0,
            show_help: false,
            fullscreen: start_fullscreen,
            load_error: None,
        };
        if let Some(path) = initial_path {
            app.open_document(path);
        }
        app
    }

    fn open_document(&mut self, path: PathBuf) {
        match PdfWorker::open(&path, self.config.render_dpi) {
            Ok(worker) => {
                self.worker = Some(worker);
                self.cache = PageCache::new();
                self.textures.clear();
                self.current = 0;
                self.load_error = None;
            }
            Err(e) => {
                self.load_error = Some(e.to_string());
            }
        }
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
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
            }
            Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Any keypress closes the help screen except the toggle itself,
        // which is handled by the normal action dispatch below.
        let action = ctx.input(|i| triggered_action(&self.keybinds, i));
        if let Some(action) = action {
            self.handle_action(action, ctx);
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

        for (page, image) in self.cache.drain_finished(worker) {
            let tex = ctx.load_texture(
                format!("page-{page}"),
                image,
                TextureOptions::LINEAR,
            );
            self.textures.insert(page, tex);
        }

        self.cache.refresh(worker, self.current, self.config.cache_window);

        // Drop textures for pages that fell out of the cache window.
        let keep: HashSet<usize> = self.cache.images.keys().copied().collect();
        self.textures.retain(|p, _| keep.contains(p));

        if self.cache.is_pending(self.current) {
            ctx.request_repaint_after(Duration::from_millis(8));
        }
    }

    fn render_page_area(&self, ui: &mut egui::Ui) {
        let Some(_worker) = &self.worker else {
            ui.centered_and_justified(|ui| {
                ui.label("Drop a PDF file here to present.");
            });
            return;
        };

        let count = self.page_count();

        ui.centered_and_justified(|ui| {
            if let Some(tex) = self.textures.get(&self.current) {
                let img_size = tex.size_vec2();
                let avail = ui.available_size();
                let scale = (avail.x / img_size.x).min(avail.y / img_size.y).min(1.0e6);
                let draw_size = img_size * scale.max(0.0001);
                ui.add(egui::Image::new((tex.id(), draw_size)));
            } else {
                ui.spinner();
            }
        });

        // Page indicator, bottom-right corner.
        egui::Area::new(egui::Id::new("page_indicator"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -10.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ui.ctx(), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} / {}", self.current + 1, count))
                        .color(Color32::from_gray(160))
                        .small(),
                );
            });
    }
}

impl eframe::App for PresenterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(ctx);
        self.refresh_cache_and_textures(ctx);

        let bg = self.config.background_color;
        let frame = egui::Frame::none().fill(Color32::from_rgb(bg[0], bg[1], bg[2]));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if let Some(err) = &self.load_error {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::LIGHT_RED, err);
                });
            } else {
                self.render_page_area(ui);
            }
        });

        if self.show_help {
            show_help_overlay(ctx, &self.keybinds);
        }
    }
}
