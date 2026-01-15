use crate::api::graphics::GraphicsBackend;

pub struct DummyGraphicsBackend {

}

pub fn boxed() -> Box<dyn GraphicsBackend> {
    Box::new(DummyGraphicsBackend::new())
}

impl DummyGraphicsBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl GraphicsBackend for DummyGraphicsBackend {
    fn name(&self) -> &str {
        "dummy"
    }

    fn init(&mut self) {
        println!("initialized dummy graphics backend");
    }

    fn update(&mut self) {
        // do nothing
    }
}
