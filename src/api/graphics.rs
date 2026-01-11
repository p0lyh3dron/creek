fn load_backend(name: &str) -> Box<dyn GraphicsBackend> {
    match name {
        "dummy" => crate::backends::graphics_dummy::boxed(),
        _ => panic!("unknown graphics backend: {}", name),
    }
}
pub trait GraphicsBackend {
    fn name(&self) -> &str;
}

pub struct Graphics {
    backend: Box<dyn GraphicsBackend>,
}

impl Graphics {
    pub fn new(name: &str) -> Self {
        Self { backend: load_backend(name) }
    }

    pub fn swap_backend(&mut self, name: &str) {
        self.backend = load_backend(name);
    }
}
