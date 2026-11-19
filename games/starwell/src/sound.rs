//! A small software mixer: WAV assets loaded from the cart archive, played
//! as voices and written into the wasmcart audio ring (interleaved f32
//! stereo). Each frame writes as many stereo frames as the host clock says
//! have passed since the first frame, so the stream neither drifts nor gaps.

use crate::{audio, audio_cursor, info};

pub struct Sound {
    /// Mono samples, -1..1.
    data: Vec<f32>,
    rate: f32,
}

/// Parse a 16-bit PCM WAV (mono or stereo, stereo is folded to mono).
fn parse_wav(b: &[u8]) -> Option<Sound> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let mut p = 12;
    let (mut ch, mut rate, mut bits) = (1u16, 32000u32, 16u16);
    while p + 8 <= b.len() {
        let id = &b[p..p + 4];
        let len = u32::from_le_bytes([b[p + 4], b[p + 5], b[p + 6], b[p + 7]]) as usize;
        let body = &b[p + 8..(p + 8 + len).min(b.len())];
        if id == b"fmt " && body.len() >= 16 {
            ch = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            if bits != 16 || ch == 0 {
                return None;
            }
            let frames = body.len() / 2 / ch as usize;
            let mut data = Vec::with_capacity(frames);
            for f in 0..frames {
                let mut s = 0.0;
                for c in 0..ch as usize {
                    let i = (f * ch as usize + c) * 2;
                    s += i16::from_le_bytes([body[i], body[i + 1]]) as f32 / 32768.0;
                }
                data.push(s / ch as f32);
            }
            return Some(Sound { data, rate: rate as f32 });
        }
        p += 8 + len + (len & 1);
    }
    None
}

pub fn load(path: &str) -> Sound {
    let empty = Sound { data: Vec::new(), rate: 32000.0 };
    let Some(n) = wasmcart::wc_asset_size(path) else {
        wasmcart::wc_log(&format!("starwell: missing asset {path}"));
        return empty;
    };
    let mut bytes = vec![0u8; n];
    if wasmcart::wc_load_asset(path, &mut bytes).is_none() {
        return empty;
    }
    parse_wav(&bytes).unwrap_or(empty)
}

struct Voice {
    sound: usize,
    pos: f64,
    vol: f32,
    pan: f32,
    pitch: f32,
    looped: bool,
    /// Channel id for looped voices whose volume the game steers.
    tag: u32,
}

pub struct Mixer {
    pub sounds: Vec<Sound>,
    voices: Vec<Voice>,
    rate: f64,
    start_ms: Option<f64>,
    emitted: u64,
    /// Running sum of squares of what was written, for the debug field.
    pub level: f32,
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        Mixer { sounds: Vec::new(), voices: Vec::new(), rate: rate as f64, start_ms: None, emitted: 0, level: 0.0 }
    }
    pub fn add(&mut self, s: Sound) -> usize {
        self.sounds.push(s);
        self.sounds.len() - 1
    }
    pub fn play(&mut self, sound: usize, vol: f32, pan: f32, pitch: f32) {
        if self.voices.len() > 24 {
            self.voices.remove(self.voices.iter().position(|v| !v.looped).unwrap_or(0));
        }
        self.voices.push(Voice { sound, pos: 0.0, vol, pan, pitch, looped: false, tag: 0 });
    }
    /// Start (or keep) a looped voice with `tag`, at volume `vol`.
    pub fn loop_vol(&mut self, tag: u32, sound: usize, vol: f32, pitch: f32) {
        if let Some(v) = self.voices.iter_mut().find(|v| v.looped && v.tag == tag) {
            if v.sound != sound {
                v.sound = sound;
                v.pos = 0.0;
            }
            v.vol = vol;
            v.pitch = pitch;
            return;
        }
        self.voices.push(Voice { sound, pos: 0.0, vol, pan: 0.0, pitch, looped: true, tag });
    }

    /// Fill the ring with this frame's audio.
    pub fn mix(&mut self, time_ms: f64) {
        let cap = info().audio_cap as usize;
        if cap == 0 {
            return;
        }
        let start = *self.start_ms.get_or_insert(time_ms);
        let target = ((time_ms - start).max(0.0) * self.rate / 1000.0) as u64 + 256;
        let mut n = target.saturating_sub(self.emitted) as usize;
        if n >= cap {
            self.emitted = target - (cap as u64 - 1);
            n = cap - 1;
        }
        if n == 0 {
            return;
        }
        let ring = audio();
        let cursor = audio_cursor();
        let mut w = *cursor as usize % cap;
        let mut sum = 0.0f32;
        for _ in 0..n {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for v in self.voices.iter_mut() {
                let s = &self.sounds[v.sound];
                let len = s.data.len();
                if len == 0 {
                    continue;
                }
                if v.pos >= len as f64 {
                    if v.looped {
                        v.pos -= len as f64;
                    } else {
                        continue;
                    }
                }
                let i = v.pos as usize;
                let fr = (v.pos - i as f64) as f32;
                let j = if i + 1 < len { i + 1 } else if v.looped { 0 } else { i };
                let x = (s.data[i] * (1.0 - fr) + s.data[j] * fr) * v.vol;
                l += x * (1.0 - v.pan.max(0.0));
                r += x * (1.0 + v.pan.min(0.0));
                v.pos += (s.rate as f64 / self.rate) * v.pitch as f64;
            }
            // Gentle limiter so stacked effects never clip hard.
            let (l, r) = (l.tanh() * 0.95, r.tanh() * 0.95);
            sum += l * l + r * r;
            ring[w * 2] = l;
            ring[w * 2 + 1] = r;
            w = (w + 1) % cap;
        }
        *cursor = w as u32;
        self.emitted += n as u64;
        self.level = (sum / (2 * n) as f32).sqrt();
        let sounds = &self.sounds;
        self.voices.retain(|v| v.looped || v.pos < sounds[v.sound].data.len() as f64);
    }
}
