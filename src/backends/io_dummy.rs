use crate::api::io::IOBackend;

pub struct DummyGraphicsBackend {

}

pub fn boxed() -> Box<dyn IOBackend> {
    Box::new(DummyGraphicsBackend::new())
}

impl DummyGraphicsBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl IOBackend for DummyGraphicsBackend {
    fn name(&self) -> &str {
        "dummy"
    }

    fn init(&mut self) {
        println!("initialized dummy graphics backend");
    }
}
