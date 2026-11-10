//! pygame.mixer mit rodio: Sound (Effekte, ganz im Speicher), Channel, music (gestreamt).
//! Ohne Audiogerät wird alles still übersprungen.

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub struct Mixer {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    music_sink: RefCell<Option<Sink>>,
    music_path: RefCell<Option<String>>,
    music_volume: Cell<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone)]
pub struct Sound {
    data: Arc<Vec<f32>>,
    channels: u16,
    rate: u32,
    volume: Rc<Cell<f32>>,
}

pub struct Channel {
    sink: Sink,
}

impl Mixer {
    pub fn new() -> Mixer {
        let muted = std::env::var("JND_MUTE").map(|v| v == "1").unwrap_or(false);
        let (stream, handle) = if muted {
            (None, None)
        } else {
            match OutputStream::try_default() {
                Ok((s, h)) => (Some(s), Some(h)),
                Err(_) => (None, None),
            }
        };
        Mixer {
            _stream: stream, handle, music_sink: RefCell::new(None), music_path: RefCell::new(None),
            music_volume: Cell::new(1.0), sample_rate: 44100, channels: 2,
        }
    }

    pub fn available(&self) -> bool { self.handle.is_some() }

    /// pygame.mixer.Sound(path)
    pub fn load_sound(&self, path: &str) -> Option<Sound> {
        let f = File::open(path).ok()?;
        let dec = Decoder::new(BufReader::new(f)).ok()?;
        let channels = dec.channels();
        let rate = dec.sample_rate();
        let data: Vec<f32> = dec.convert_samples().collect();
        Some(Sound { data: Arc::new(data), channels, rate, volume: Rc::new(Cell::new(1.0)) })
    }

    /// pygame.mixer.Sound(buffer=...) aus 16-bit-Samples
    pub fn sound_from_i16(&self, samples: &[i16], channels: u16, rate: u32) -> Sound {
        let data: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
        Sound { data: Arc::new(data), channels, rate, volume: Rc::new(Cell::new(1.0)) }
    }

    /// sound.play(loops=0, maxtime=0) -> Channel
    pub fn play(&self, s: &Sound, maxtime_ms: i64) -> Option<Channel> {
        let h = self.handle.as_ref()?;
        let sink = Sink::try_new(h).ok()?;
        sink.set_volume(s.volume.get());
        let src = SamplesBuffer::new(s.channels, s.rate, (*s.data).clone());
        if maxtime_ms > 0 {
            sink.append(src.take_duration(Duration::from_millis(maxtime_ms as u64)));
        } else {
            sink.append(src);
        }
        Some(Channel { sink })
    }

    /// Spielt ab und vergisst den Kanal (läuft bis zum Ende weiter)
    pub fn play_detached(&self, s: &Sound) {
        if let Some(c) = self.play(s, 0) { c.sink.detach(); }
    }

    // ---- pygame.mixer.music ----
    pub fn music_load(&self, path: &str) {
        self.music_stop();
        *self.music_path.borrow_mut() = Some(path.to_string());
    }
    pub fn music_play(&self, loops: i32) -> bool {
        let Some(h) = self.handle.as_ref() else { return false; };
        let Some(path) = self.music_path.borrow().clone() else { return false; };
        self.music_stop();
        let Ok(sink) = Sink::try_new(h) else { return false; };
        sink.set_volume(self.music_volume.get());
        let Ok(f) = File::open(&path) else { return false; };
        if loops < 0 {
            match Decoder::new_looped(BufReader::new(f)) {
                Ok(d) => sink.append(d),
                Err(_) => return false,
            }
        } else {
            match Decoder::new(BufReader::new(f)) {
                Ok(d) => sink.append(d),
                Err(_) => return false,
            }
        }
        *self.music_sink.borrow_mut() = Some(sink);
        true
    }
    pub fn music_stop(&self) {
        if let Some(s) = self.music_sink.borrow_mut().take() { s.stop(); }
    }
    pub fn music_pause(&self) { if let Some(s) = self.music_sink.borrow().as_ref() { s.pause(); } }
    pub fn music_unpause(&self) { if let Some(s) = self.music_sink.borrow().as_ref() { s.play(); } }
    pub fn music_set_volume(&self, v: f64) {
        let v = v.clamp(0.0, 1.0) as f32;
        self.music_volume.set(v);
        if let Some(s) = self.music_sink.borrow().as_ref() { s.set_volume(v); }
    }
    pub fn music_get_volume(&self) -> f64 { self.music_volume.get() as f64 }
    pub fn music_get_busy(&self) -> bool {
        match self.music_sink.borrow().as_ref() { Some(s) => !s.empty() && !s.is_paused(), None => false }
    }
}

impl Default for Mixer { fn default() -> Self { Mixer::new() } }

impl Sound {
    pub fn set_volume(&self, v: f64) { self.volume.set(v.clamp(0.0, 1.0) as f32); }
    pub fn get_volume(&self) -> f64 { self.volume.get() as f64 }
    pub fn get_length(&self) -> f64 {
        if self.rate == 0 || self.channels == 0 { return 0.0; }
        self.data.len() as f64 / self.channels as f64 / self.rate as f64
    }
}

impl Channel {
    pub fn stop(&self) { self.sink.stop(); }
    pub fn get_busy(&self) -> bool { !self.sink.empty() }
    pub fn set_volume(&self, v: f64) { self.sink.set_volume(v.clamp(0.0, 1.0) as f32); }
}
