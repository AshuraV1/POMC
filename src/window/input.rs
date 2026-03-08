use std::collections::HashSet;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct InputState {
    pressed: HashSet<KeyCode>,
    mouse_delta: (f64, f64),
    cursor_captured: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            mouse_delta: (0.0, 0.0),
            cursor_captured: true,
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
                }
                ElementState::Released => {
                    self.pressed.remove(&code);
                }
            }
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

    pub fn toggle_cursor_capture(&mut self) {
        self.cursor_captured = !self.cursor_captured;
    }

    pub fn is_cursor_captured(&self) -> bool {
        self.cursor_captured
    }
}
