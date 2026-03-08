use std::collections::VecDeque;
use std::time::Instant;

use egui::{Color32, Pos2, Rect};

const MAX_MESSAGES: usize = 100;
const VISIBLE_MESSAGES: usize = 10;
const MESSAGE_LIFETIME_SECS: f32 = 10.0;
const CHAT_X: f32 = 4.0;
const CHAT_BOTTOM_OFFSET: f32 = 52.0;
const LINE_HEIGHT: f32 = 16.0;
const FONT_SIZE: f32 = 13.0;
const INPUT_HEIGHT: f32 = 24.0;

struct ChatLine {
    text: String,
    received: Instant,
}

pub struct ChatState {
    messages: VecDeque<ChatLine>,
    input: String,
    open: bool,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            input: String::new(),
            open: false,
        }
    }

    pub fn push_message(&mut self, text: String) {
        self.messages.push_back(ChatLine {
            text,
            received: Instant::now(),
        });
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.input.clear();
    }

    pub fn open_with_slash(&mut self) {
        self.open = true;
        self.input = "/".into();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.input.clear();
    }

    pub fn take_pending_message(&mut self) -> Option<String> {
        if self.open && !self.input.is_empty() {
            let msg = self.input.clone();
            self.input.clear();
            self.open = false;
            Some(msg)
        } else {
            self.open = false;
            None
        }
    }

    pub fn draw(&mut self, ctx: &egui::Context, screen: Rect) -> Option<String> {
        let now = Instant::now();
        let chat_bottom = screen.max.y - CHAT_BOTTOM_OFFSET;

        egui::Area::new(egui::Id::new("chat_messages"))
            .fixed_pos(Pos2::ZERO)
            .interactable(false)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_clip_rect(screen);
                let painter = ui.painter();

                let visible: Vec<&ChatLine> = if self.open {
                    self.messages.iter().rev().take(VISIBLE_MESSAGES).collect()
                } else {
                    self.messages
                        .iter()
                        .rev()
                        .filter(|m| now.duration_since(m.received).as_secs_f32() < MESSAGE_LIFETIME_SECS)
                        .take(VISIBLE_MESSAGES)
                        .collect()
                };

                for (i, line) in visible.iter().enumerate() {
                    let y = chat_bottom - (i as f32 + 1.0) * LINE_HEIGHT;
                    let bg_rect = Rect::from_min_size(
                        Pos2::new(CHAT_X, y),
                        egui::Vec2::new(320.0, LINE_HEIGHT),
                    );
                    painter.rect_filled(bg_rect, 0.0, Color32::from_black_alpha(100));
                    painter.text(
                        Pos2::new(CHAT_X + 4.0, y + LINE_HEIGHT / 2.0),
                        egui::Align2::LEFT_CENTER,
                        &line.text,
                        egui::FontId::proportional(FONT_SIZE),
                        Color32::WHITE,
                    );
                }

                ui.allocate_rect(screen, egui::Sense::hover());
            });

        let mut pending = None;

        if self.open {
            egui::Area::new(egui::Id::new("chat_input"))
                .fixed_pos(Pos2::new(CHAT_X, chat_bottom))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let response = ui.add_sized(
                        [320.0, INPUT_HEIGHT],
                        egui::TextEdit::singleline(&mut self.input)
                            .desired_width(312.0)
                            .font(egui::FontId::proportional(FONT_SIZE)),
                    );
                    if response.lost_focus() {
                        pending = self.take_pending_message();
                    } else {
                        response.request_focus();
                    }
                });
        }

        pending
    }
}
