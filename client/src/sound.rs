//! A tiny synthesized ding. No audio files, so the same beep works on Ubuntu, macOS, and Windows.
//!
//! Playback stays on its own thread. Opening the sound device can block, and the window
//! thread should keep drawing.

use std::sync::mpsc::{self, SyncSender};
use std::sync::OnceLock;
use std::time::Duration;

use rodio::Source;

pub fn play(kind: &str) {
    let Some(sender) = sender() else {
        return;
    };
    // One beep at a time. Extra dings while a note is playing are dropped.
    let _ = sender.try_send(kind.to_string());
}

fn sender() -> Option<&'static SyncSender<String>> {
    AUDIO.get_or_init(start_audio).as_ref()
}

fn start_audio() -> Option<SyncSender<String>> {
    let (tx, rx) = mpsc::sync_channel::<String>(1);
    std::thread::Builder::new()
        .name("ding-sound".into())
        .spawn(move || {
            let Ok(stream) = rodio::OutputStreamBuilder::open_default_stream() else {
                return;
            };
            while let Ok(kind) = rx.recv() {
                let player = rodio::Sink::connect_new(stream.mixer());
                player.append(Beep::new(&kind));
                player.sleep_until_end();
            }
        })
        .ok()?;
    Some(tx)
}

static AUDIO: OnceLock<Option<SyncSender<String>>> = OnceLock::new();

struct Beep {
    samples: Vec<f32>,
    pos: usize,
}

impl Beep {
    fn new(kind: &str) -> Self {
        const RATE: f32 = 44_100.0;
        let seconds = if kind == "airhorn" { 0.45 } else { 0.32 };
        let count = (RATE * seconds) as usize;
        let samples = (0..count)
            .map(|index| sample_at(kind, index as f32 / RATE))
            .collect();
        Self { samples, pos: 0 }
    }
}

fn sample_at(kind: &str, t: f32) -> f32 {
    let tone = match kind {
        "knock" => {
            let hit = if t < 0.09 || (0.16..0.25).contains(&t) {
                (t * 140.0 * std::f32::consts::TAU).sin()
            } else {
                0.0
            };
            hit * (-t * 18.0).exp()
        }
        "airhorn" => {
            let harsh = (t * 440.0 * std::f32::consts::TAU).sin()
                + 0.5 * (t * 880.0 * std::f32::consts::TAU).sin()
                + 0.25 * (t * 1320.0 * std::f32::consts::TAU).sin();
            harsh * 0.35 * (1.0 - t / 0.45).max(0.0)
        }
        _ => {
            let freq = if t < 0.12 { 880.0 } else { 1174.0 };
            (t * freq * std::f32::consts::TAU).sin() * (-t * 7.0).exp()
        }
    };
    (tone * 0.22).clamp(-1.0, 1.0)
}

impl Iterator for Beep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.samples.get(self.pos).copied()?;
        self.pos += 1;
        Some(sample)
    }
}

impl Source for Beep {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        44_100
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.samples.len() as f32 / 44_100.0,
        ))
    }
}
