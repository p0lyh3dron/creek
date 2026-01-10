use crate::api::scripts::ScriptBackend;

pub struct DummyBackend {

}

pub fn boxed() -> Box<dyn ScriptBackend> {
    Box::new(DummyBackend::new())
}

impl DummyBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBackend for DummyBackend {
    fn name(&self) -> &str {
        "dummy"
    }

    fn exec(&mut self, code: &str) -> Result<(), String> {
        println!("dummy executing code: {}", code);
        Ok(())
    }
}
