fn load_backend(name: &str) -> Box<dyn GraphicsBackend> {
    match name {
        "dummy" => crate::backends::graphics_dummy::boxed(),
        #[cfg(feature = "vulkan")]
        "vulkan" => crate::backends::graphics_vulkan::boxed(),
        _ => panic!("unknown graphics backend: {}", name),
    }
}
pub trait GraphicsBackend {
    fn name(&self) -> &str;
    fn init(&mut self);
    fn update(&mut self);
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

    pub fn init(&mut self) {
        self.backend.init();
    }

    pub fn update(&mut self) {
        self.backend.update();
    }
}
