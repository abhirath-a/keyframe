mod app;
mod config;
mod help;
mod keybinds;
mod pdf_worker;

use std::path::PathBuf;

use app::PresenterApp;
use config::Config;

fn main() -> eframe::Result<()> {
    let path = std::env::args().nth(1).map(PathBuf::from);

    let config = Config::load();
    // Best-effort: write out a default config file so there's something to
    // edit. Never fatal if this fails (e.g. read-only filesystem).
    if let Err(e) = config.ensure_default_file() {
        eprintln!("warning: could not write default config: {e}");
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title("Keyframe")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([320.0, 240.0])
        .with_fullscreen(config.start_fullscreen)
        .with_drag_and_drop(true);

    let native_options = eframe::NativeOptions {
        viewport,
        vsync: config.vsync,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    let start_fullscreen = config.start_fullscreen;

    eframe::run_native(
        "keyframe",
        native_options,
        Box::new(move |_cc| Ok(Box::new(PresenterApp::new(path, config, start_fullscreen)))),
    )
}
