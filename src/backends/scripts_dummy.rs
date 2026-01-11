use crate::api::scripts::ScriptBackend;

pub struct DummyScriptsBackend {

}

pub fn boxed() -> Box<dyn ScriptBackend> {
    Box::new(DummyScriptsBackend::new())
}

impl DummyScriptsBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBackend for DummyScriptsBackend {
    fn name(&self) -> &str {
        "dummy"
    }

    fn exec(&mut self, code: &str) -> Result<(), String> {
        println!("dummy executing code: {}", code);
        Ok(())
    }
}
