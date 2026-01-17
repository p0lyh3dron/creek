use crate::api::govern::GoverningBackend;


pub struct WinitGovernor {

}

pub fn boxed() -> Box<dyn GoverningBackend> {
    Box::new(WinitGovernor::new())
}

impl WinitGovernor {
    pub fn new() -> Self {
        Self {}
    }
}

impl GoverningBackend for WinitGovernor {
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
