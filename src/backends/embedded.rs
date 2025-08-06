use crate::api::Window;

pub struct EmbeddedWindow {
    open: bool,
}

impl Window for EmbeddedWindow {
    fn new(_title: &str, _width: u32, _height: u32) -> Self {
        // Simulate headless mode
        Self { open: true }
    }

    fn update(&mut self) {
        // No-op or simulated I/O
    }

    fn is_open(&self) -> bool {
        self.open
    }
}