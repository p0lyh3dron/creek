fn load_backend(name: &str) -> Box<dyn ScriptBackend> {
    match name {
        "dummy" => crate::backends::scripts_dummy::boxed(),
        _ => panic!("unknown script backend: {}", name),
    }
}
pub trait ScriptBackend: Send {
    fn name(&self) -> &str;
    fn exec(&mut self, code: &str) -> Result<(), String>;
}

pub struct Scripts {
    backend: Box<dyn ScriptBackend>,
}

impl Scripts {
    pub fn new(name: &str) -> Self {
        Self { backend: load_backend(name) }
    }

    pub fn swap_backend(&mut self, name: &str) {
        self.backend = load_backend(name);
    }

    pub fn exec(&mut self, code: &str) -> Result<(), String> {
        self.backend.exec(code)
    }
}
