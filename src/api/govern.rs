fn load_backend(name: &str) -> Box<dyn GoverningBackend> {
    match name {
        "dummy" => crate::backends::govern_dummy::boxed(),
        #[cfg(feature = "winit")]
        "winit" => crate::backends::govern_winit::boxed(),
        _ => panic!("unknown governing backend: {}", name),
    }
}
pub trait GoverningBackend {
    fn name(&self) -> &str;
    fn init(&mut self);
    fn submit(&mut self);
}

pub struct Governor {
    backend: Box<dyn GoverningBackend>,
}

impl Governor {
    pub fn new(name: &str) -> Self {
        Self { backend: load_backend(name) }
    }

    pub fn swap_backend(&mut self, name: &str) {
        self.backend = load_backend(name);
    }

    pub fn init(&mut self) {
        self.backend.init();
    }

    pub fn submit(&mut self) {
        self.backend.submit();
    }
}
