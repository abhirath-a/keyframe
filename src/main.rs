#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod config;
mod help;
mod keybinds;
mod notes;
mod pdf_worker;

use std::path::PathBuf;

use app::PresenterApp;
use config::Config;

fn main() {
    #[cfg(windows)]
    {
        let app_id: Vec<u16> = "dev.keyframe.keyframe\0".encode_utf16().collect();
        // Match the installed shortcut so Windows groups and pins the app correctly.
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
        }
    }
    if let Err(error) = run() {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Keyframe could not start")
            .set_description(error.to_string())
            .show();
    }
}

fn run() -> eframe::Result<()> {
    let path = std::env::args_os().nth(1).map(PathBuf::from);

    let config = Config::load();
    // Best-effort: write out a default config file so there's something to
    // edit. Never fatal if this fails (e.g. read-only filesystem).
    if let Err(e) = config.ensure_default_file() {
        eprintln!("warning: could not write default config: {e}");
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title("Keyframe")
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../packaging/keyframe.png"))
                .expect("The bundled Keyframe icon must be a valid PNG"),
        )
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 480.0])
        .with_fullscreen(config.start_fullscreen)
        .with_drag_and_drop(true);

    let native_options = eframe::NativeOptions {
        viewport,
        vsync: config.vsync,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    let start_fullscreen = config.start_fullscreen;

    eframe::run_native(
        "keyframe",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            cc.egui_ctx.style_mut(|style| {
                style.spacing.button_padding = egui::vec2(12.0, 8.0);
                style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            });
            Ok(Box::new(PresenterApp::new(path, config, start_fullscreen)))
        }),
    )
}
