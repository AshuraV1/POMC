use egui::Color32;

use super::hud::{mc_button, HudTextures, BUTTON_GAP};

pub enum MenuAction {
    None,
    Connect { server: String, username: String },
    Quit,
}

const LABEL_COLOR: Color32 = Color32::from_rgb(200, 200, 200);

pub struct MainMenu {
    server_address: String,
    username: String,
    show_connect: bool,
}

impl MainMenu {
    pub fn new() -> Self {
        Self {
            server_address: "localhost:25565".into(),
            username: "Steve".into(),
            show_connect: false,
        }
    }

    pub fn draw(&mut self, ctx: &egui::Context, textures: &HudTextures) -> MenuAction {
        let mut action = MenuAction::None;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(30, 30, 30)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);

                    ui.label(
                        egui::RichText::new("Ferrite")
                            .size(64.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );

                    ui.add_space(12.0);

                    ui.label(
                        egui::RichText::new("A Minecraft client written in Rust")
                            .size(16.0)
                            .color(Color32::from_rgb(180, 180, 180)),
                    );

                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("Heavy early development - just getting started!")
                            .size(14.0)
                            .color(Color32::from_rgb(255, 200, 100))
                            .italics(),
                    );

                    ui.add_space(40.0);

                    if self.show_connect {
                        self.draw_connect_form(ui, textures, &mut action);
                    } else {
                        if mc_button(ui, textures, "Direct Connect") {
                            self.show_connect = true;
                        }

                        ui.add_space(BUTTON_GAP);

                        if mc_button(ui, textures, "Quit Game") {
                            action = MenuAction::Quit;
                        }
                    }
                });
            });

        action
    }

    fn draw_connect_form(
        &mut self,
        ui: &mut egui::Ui,
        textures: &HudTextures,
        action: &mut MenuAction,
    ) {
        ui.set_max_width(300.0);

        ui.label(
            egui::RichText::new("Username")
                .size(14.0)
                .color(LABEL_COLOR),
        );
        ui.add_sized(
            [300.0, 30.0],
            egui::TextEdit::singleline(&mut self.username),
        );

        ui.add_space(8.0);

        ui.label(
            egui::RichText::new("Server Address")
                .size(14.0)
                .color(LABEL_COLOR),
        );
        let response = ui.add_sized(
            [300.0, 30.0],
            egui::TextEdit::singleline(&mut self.server_address),
        );

        ui.add_space(16.0);

        let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        ui.horizontal(|ui| {
            ui.add_space(40.0);

            if mc_button(ui, textures, "Back") {
                self.show_connect = false;
            }

            ui.add_space(16.0);

            if mc_button(ui, textures, "Connect") || enter_pressed {
                *action = MenuAction::Connect {
                    server: self.server_address.clone(),
                    username: self.username.clone(),
                };
            }
        });
    }
}
