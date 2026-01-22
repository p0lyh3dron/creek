fn load_backend(name: &str) -> Box<dyn IOBackend> {
    match name {
        "dummy" => crate::backends::io_dummy::boxed(),
        #[cfg(feature = "vulkan")]
        "vulkan" => crate::backends::io_vulkanwinit::boxed(),
        _ => panic!("unknown graphics backend: {}", name),
    }
}
pub trait IOBackend {
    fn name(&self) -> &str;
    fn init(&mut self);
}

pub struct IO {
    backend: Box<dyn IOBackend>,
}

impl IO {
    pub fn new(name: &str) -> Self {
        Self { backend: load_backend(name) }
    }

    pub fn swap_backend(&mut self, name: &str) {
        self.backend = load_backend(name);
    }

    pub fn init(&mut self) {
        self.backend.init();
    }
}
