use std::collections::HashMap;

use egui::{Align2, Color32, Context, Id, RichText};

use crate::keybinds::{Action, KeyCombo};

/// Draws a full-screen semi-transparent overlay listing every action and
/// its currently-bound keys. Toggled by `Action::ToggleHelp` (default `?`).
pub fn show_help_overlay(ctx: &Context, binds: &HashMap<Action, Vec<KeyCombo>>) {
    egui::Area::new(Id::new("help_overlay"))
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            // Dim the whole screen behind the panel.
            let screen = ctx.screen_rect();
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(200));

            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(20.0))
                .show(ui, |ui| {
                    ui.set_max_width(420.0);
                    ui.vertical(|ui| {
                        ui.heading("Keyboard Shortcuts");
                        ui.add_space(8.0);

                        egui::Grid::new("help_grid")
                            .num_columns(2)
                            .spacing([24.0, 6.0])
                            .show(ui, |ui| {
                                for action in Action::ALL {
                                    ui.label(action.label());
                                    let combos = binds
                                        .get(&action)
                                        .map(|c| {
                                            c.iter()
                                                .map(KeyCombo::display)
                                                .collect::<Vec<_>>()
                                                .join("  /  ")
                                        })
                                        .unwrap_or_default();
                                    ui.label(RichText::new(combos).monospace());
                                    ui.end_row();
                                }
                            });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Press ? again to close").weak().italics(),
                        );
                    });
                });
        });
}
