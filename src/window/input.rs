use std::collections::HashSet;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct InputState {
    pressed: HashSet<KeyCode>,
    mouse_delta: (f64, f64),
    cursor_captured: bool,
    selected_slot: u8,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            mouse_delta: (0.0, 0.0),
            cursor_captured: true,
            selected_slot: 0,
        }
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn on_key_event(&mut self, event: &winit::event::KeyEvent) {
        if let PhysicalKey::Code(code) = event.physical_key {
            match event.state {
                ElementState::Pressed => {
                    self.pressed.insert(code);
                    if let Some(slot) = hotbar_slot(code) {
                        self.selected_slot = slot;
                    }
                }
                ElementState::Released => {
                    self.pressed.remove(&code);
                }
            }
        }
    }

    pub fn selected_slot(&self) -> u8 {
        self.selected_slot
    }

    pub fn on_scroll(&mut self, delta: f32) {
        if delta > 0.0 {
            self.selected_slot = (self.selected_slot + 1) % 9;
        } else if delta < 0.0 {
            self.selected_slot = (self.selected_slot + 8) % 9;
        }
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.mouse_delta.0 += delta.0;
        self.mouse_delta.1 += delta.1;
    }

    pub fn consume_mouse_delta(&mut self) -> (f64, f64) {
        let delta = self.mouse_delta;
        self.mouse_delta = (0.0, 0.0);
        delta
    }

    pub fn is_cursor_captured(&self) -> bool {
        self.cursor_captured
    }
}

fn hotbar_slot(code: KeyCode) -> Option<u8> {
    match code {
        KeyCode::Digit1 => Some(0),
        KeyCode::Digit2 => Some(1),
        KeyCode::Digit3 => Some(2),
        KeyCode::Digit4 => Some(3),
        KeyCode::Digit5 => Some(4),
        KeyCode::Digit6 => Some(5),
        KeyCode::Digit7 => Some(6),
        KeyCode::Digit8 => Some(7),
        KeyCode::Digit9 => Some(8),
        _ => None,
    }
}
