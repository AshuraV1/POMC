use egui::{Color32, Pos2, Sense, Vec2};

use super::hud::{mc_button, HudTextures, BUTTON_GAP};

pub enum PauseAction {
    None,
    Resume,
    Disconnect,
    Quit,
}

pub fn draw_pause_menu(ctx: &egui::Context, textures: &HudTextures) -> PauseAction {
    let screen = ctx.screen_rect();
    let mut action = PauseAction::None;

    egui::Area::new(egui::Id::new("pause_overlay"))
        .fixed_pos(Pos2::ZERO)
        .interactable(false)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(120));
            ui.allocate_rect(screen, Sense::hover());
        });

    egui::Area::new(egui::Id::new("pause_menu"))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Game Menu")
                        .size(20.0)
                        .color(Color32::WHITE),
                );
                ui.add_space(16.0);

                if mc_button(ui, textures, "Back to Game") {
                    action = PauseAction::Resume;
                }
                ui.add_space(BUTTON_GAP);
                if mc_button(ui, textures, "Disconnect") {
                    action = PauseAction::Disconnect;
                }
                ui.add_space(BUTTON_GAP);
                if mc_button(ui, textures, "Quit Game") {
                    action = PauseAction::Quit;
                }
            });
        });

    action
}
