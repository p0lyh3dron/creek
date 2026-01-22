use crate::api::audio::AudioBackend;

pub struct DummyAudioBackend {

}

pub fn boxed() -> Box<dyn AudioBackend> {
    Box::new(DummyAudioBackend::new())
}

impl DummyAudioBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl AudioBackend for DummyAudioBackend {
    fn name(&self) -> &str {
        "dummy"
    }

    fn load_sound_file(&mut self, path: &str) -> Result<u32, String> {
        println!("dummy loading sound file: {}", path);
        Ok(0)
    }

    fn play_sound(&mut self, sound_id: u32) {
        println!("dummy playing sound id: {}", sound_id);
    }
}
