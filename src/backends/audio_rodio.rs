use std::fs::File;
use rodio::{Decoder, OutputStream, Sink, Source};

use crate::api::audio::AudioBackend;

pub struct RodioAudioBackend {
    stream: OutputStream,
    sink: Sink,

    open_sounds: Vec<(u32, rodio::source::Buffered<rodio::Decoder<std::io::BufReader<File>>>)>,
}

pub fn boxed() -> Box<dyn AudioBackend> {
    Box::new(RodioAudioBackend::new())
}

impl RodioAudioBackend {
    pub fn new() -> Self {
        let stream = rodio::OutputStreamBuilder::open_default_stream().expect("failed to open default audio output stream");
        let sink = rodio::Sink::connect_new(&stream.mixer());
        let open_sounds = Vec::new();
        Self { stream, sink, open_sounds, }
    }
}

impl AudioBackend for RodioAudioBackend {
    fn name(&self) -> &str {
        "rodio"
    }

    fn load_sound_file(&mut self, path: &str) -> Result<u32, String> {
        let file = File::open(path).unwrap();
        let source = Decoder::try_from(file).unwrap().buffered();

        self.open_sounds.push((self.open_sounds.len() as u32, source));
        Ok(self.open_sounds.len() as u32 - 1)
    }

    fn play_sound(&mut self, sound_id: u32) {
        if let Some((_, source)) = self.open_sounds.iter().find(|(id, _)| *id == sound_id) {
            self.stream.mixer().add(source.clone())
        } else {
            println!("rodio: sound id {} not found", sound_id);
        }
    }
}
