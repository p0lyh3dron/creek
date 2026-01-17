use crate::api::govern::GoverningBackend;

pub struct DummyGovernor {

}

pub fn boxed() -> Box<dyn GoverningBackend> {
    Box::new(DummyGovernor::new())
}

impl DummyGovernor {
    pub fn new() -> Self {
        Self {}
    }
}

impl GoverningBackend for DummyGovernor {
    fn name(&self) -> &str {
        "dummy"
    }

    fn init(&mut self) {
        println!("initialized dummy graphics backend");
    }

    fn submit(&mut self) {
        // do nothing
    }
}
