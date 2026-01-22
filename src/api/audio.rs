fn load_backend(name: &str) -> Box<dyn AudioBackend> {
    match name {
        "dummy" => crate::backends::audio_dummy::boxed(),
        #[cfg(feature = "use_rodio")]
        "rodio" => crate::backends::audio_rodio::boxed(),
        _ => panic!("unknown script backend: {}", name),
    }
}
pub trait AudioBackend: Send + Sync {
    fn name(&self) -> &str;
    fn load_sound_file(&mut self, path: &str) -> Result<u32, String>;
    fn play_sound(&mut self, sound_id: u32);
}

pub struct Audio {
    backend: Box<dyn AudioBackend>,
}

impl Audio {
    pub fn new(name: &str) -> Self {
        Self { backend: load_backend(name) }
    }

    pub fn swap_backend(&mut self, name: &str) {
        self.backend = load_backend(name);
    }

    pub fn load_sound_file(&mut self, path: &str) -> Result<u32, String> {
        self.backend.load_sound_file(path)
    }

    pub fn play_sound(&mut self, sound_id: u32) {
        self.backend.play_sound(sound_id);
    }    
}
