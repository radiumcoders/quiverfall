//! Synthesised sound effects and an ambient drone, rendered to in-memory WAVs at startup.

use bevy::audio::{AudioSource, PlaybackMode, Volume};
use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::raster::hash2;

const RATE: u32 = 44100;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    Jump,
    Land,
    Dash,
    Draw,
    Release,
    HitWood,
    HitStone,
    HitEnemy,
    EnemyDie,
    Hurt,
    Swipe,
    Pickup,
    Gate,
    Burn,
    Freeze,
    Blink,
    Lift,
    Tether,
    Splash,
    Block,
    Spit,
    Slam,
    Checkpoint,
    Fizzle,
    Select,
}

#[derive(Message)]
pub struct PlaySfx(pub Sfx, pub f32);

#[derive(Resource)]
pub struct Sounds {
    map: Vec<(Sfx, Handle<AudioSource>)>,
}

fn wav(samples: &[f32]) -> AudioSource {
    let mut b = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    AudioSource { bytes: b.into() }
}

struct Synth {
    out: Vec<f32>,
}

impl Synth {
    fn new(secs: f32) -> Self {
        Self { out: vec![0.0; (secs * RATE as f32) as usize] }
    }
    fn len(&self) -> f32 {
        self.out.len() as f32 / RATE as f32
    }
    /// sine/triangle sweep with exponential decay
    fn tone(&mut self, f0: f32, f1: f32, start: f32, dur: f32, amp: f32, decay: f32, tri: bool) {
        let mut ph = 0.0f32;
        let s0 = (start * RATE as f32) as usize;
        let n = (dur * RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let f = f0 * (f1 / f0).powf(t);
            ph += f / RATE as f32;
            let w = if tri { 4.0 * (ph.fract() - 0.5).abs() - 1.0 } else { (ph * TAU).sin() };
            let env = (-t * decay).exp() * (t * 200.0).min(1.0);
            if let Some(o) = self.out.get_mut(s0 + i) {
                *o += w * env * amp;
            }
        }
    }
    /// filtered noise burst (one-pole lowpass with sweeping cutoff)
    fn noise(&mut self, c0: f32, c1: f32, start: f32, dur: f32, amp: f32, decay: f32, seed: u32) {
        let s0 = (start * RATE as f32) as usize;
        let n = (dur * RATE as f32) as usize;
        let mut lp = 0.0;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let c = c0 * (c1 / c0).powf(t);
            let k = (TAU * c / RATE as f32).min(1.0);
            let r = hash2(i as i32, 0, seed) * 2.0 - 1.0;
            lp += (r - lp) * k;
            let env = (-t * decay).exp() * (t * 400.0).min(1.0);
            if let Some(o) = self.out.get_mut(s0 + i) {
                *o += lp * env * amp;
            }
        }
    }
}

fn render(s: Sfx) -> Vec<f32> {
    use Sfx::*;
    let mut y = Synth::new(0.6);
    match s {
        Jump => {
            y.noise(800.0, 3000.0, 0.0, 0.12, 0.5, 5.0, 1);
            y.tone(220.0, 330.0, 0.0, 0.1, 0.15, 4.0, true);
        }
        Land => y.noise(900.0, 200.0, 0.0, 0.1, 0.6, 6.0, 2),
        Dash => y.noise(400.0, 4000.0, 0.0, 0.22, 0.6, 3.0, 3),
        Draw => {
            y.noise(300.0, 1400.0, 0.0, 0.35, 0.25, 1.0, 4);
            y.tone(90.0, 140.0, 0.0, 0.35, 0.12, 1.5, true);
        }
        Release => {
            y.tone(160.0, 120.0, 0.0, 0.18, 0.5, 6.0, true);
            y.noise(2500.0, 6000.0, 0.0, 0.1, 0.35, 6.0, 5);
        }
        HitWood => {
            y.tone(140.0, 90.0, 0.0, 0.15, 0.6, 7.0, false);
            y.noise(1200.0, 400.0, 0.0, 0.06, 0.5, 8.0, 6);
        }
        HitStone => {
            y.tone(1800.0, 1500.0, 0.0, 0.12, 0.2, 8.0, false);
            y.noise(5000.0, 2000.0, 0.0, 0.05, 0.5, 9.0, 7);
        }
        HitEnemy => {
            y.noise(1500.0, 300.0, 0.0, 0.14, 0.8, 5.0, 8);
            y.tone(200.0, 80.0, 0.0, 0.14, 0.4, 5.0, false);
        }
        EnemyDie => {
            y.noise(2000.0, 100.0, 0.0, 0.5, 0.8, 4.0, 9);
            y.tone(300.0, 40.0, 0.0, 0.45, 0.4, 3.0, true);
        }
        Hurt => {
            y.tone(400.0, 90.0, 0.0, 0.35, 0.5, 3.0, true);
            y.noise(3000.0, 300.0, 0.0, 0.3, 0.6, 4.0, 10);
        }
        Swipe => y.noise(1500.0, 5000.0, 0.0, 0.14, 0.5, 4.0, 11),
        Pickup => {
            for (i, f) in [523.0, 659.0, 784.0, 1046.0].iter().enumerate() {
                y.tone(*f, *f, i as f32 * 0.07, 0.4, 0.22, 4.0, false);
            }
        }
        Gate => {
            y.noise(200.0, 120.0, 0.0, 0.6, 0.6, 2.0, 12);
            y.tone(70.0, 50.0, 0.0, 0.6, 0.4, 2.0, true);
        }
        Burn => y.noise(600.0, 3000.0, 0.0, 0.5, 0.5, 2.5, 13),
        Freeze => {
            y.tone(2400.0, 3200.0, 0.0, 0.4, 0.15, 3.0, false);
            y.tone(1800.0, 2600.0, 0.05, 0.35, 0.12, 3.0, false);
            y.noise(6000.0, 8000.0, 0.0, 0.3, 0.25, 4.0, 14);
        }
        Blink => {
            y.tone(900.0, 200.0, 0.0, 0.25, 0.35, 3.0, false);
            y.tone(300.0, 1200.0, 0.12, 0.25, 0.25, 3.0, false);
        }
        Lift => {
            y.tone(110.0, 220.0, 0.0, 0.5, 0.35, 1.5, true);
            y.tone(440.0, 660.0, 0.0, 0.5, 0.1, 2.0, false);
        }
        Tether => {
            y.noise(2000.0, 600.0, 0.0, 0.3, 0.4, 3.0, 15);
            y.tone(600.0, 300.0, 0.0, 0.25, 0.2, 4.0, true);
        }
        Splash => y.noise(3000.0, 500.0, 0.0, 0.4, 0.6, 3.0, 16),
        Block => {
            y.tone(900.0, 850.0, 0.0, 0.3, 0.35, 5.0, false);
            y.tone(1350.0, 1300.0, 0.0, 0.3, 0.2, 5.0, false);
        }
        Spit => y.noise(800.0, 300.0, 0.0, 0.2, 0.6, 4.0, 17),
        Slam => {
            y.tone(80.0, 30.0, 0.0, 0.55, 0.9, 2.5, false);
            y.noise(800.0, 80.0, 0.0, 0.5, 0.8, 2.5, 18);
        }
        Checkpoint => {
            for (i, f) in [392.0, 523.0, 659.0].iter().enumerate() {
                y.tone(*f, *f, i as f32 * 0.1, 0.45, 0.2, 2.5, false);
            }
        }
        Fizzle => y.noise(4000.0, 1000.0, 0.0, 0.15, 0.3, 5.0, 19),
        Select => y.tone(880.0, 990.0, 0.0, 0.06, 0.2, 5.0, false),
    }
    // trim trailing silence
    let mut out = y.out;
    while out.last().is_some_and(|v| v.abs() < 1e-4) && out.len() > 100 {
        out.pop();
    }
    out
}

fn drone() -> Vec<f32> {
    // 16 s seamless ambient pad: detuned low fifths with slow beating + filtered noise wind
    let secs = 16.0;
    let mut y = Synth::new(secs);
    let n = y.out.len();
    let freqs = [55.0, 82.41, 110.0, 164.81, 220.5];
    for (k, f) in freqs.iter().enumerate() {
        // integer cycles over the loop so it wraps cleanly
        let f = (f * secs).round() / secs;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let lfo = 0.5 + 0.5 * (TAU * t * (k as f32 + 1.0) / secs).sin();
            y.out[i] += (TAU * f * t).sin() * 0.05 * lfo / (1.0 + k as f32 * 0.4);
        }
    }
    let mut lp = 0.0;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let r = hash2(i as i32, 1, 99) * 2.0 - 1.0;
        lp += (r - lp) * 0.01;
        let swell = 0.5 + 0.5 * (TAU * t * 2.0 / secs).sin();
        y.out[i] += lp * 0.25 * swell;
    }
    let _ = y.len();
    y.out
}

pub fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    use Sfx::*;
    let all = [
        Jump, Land, Dash, Draw, Release, HitWood, HitStone, HitEnemy, EnemyDie, Hurt, Swipe, Pickup, Gate, Burn,
        Freeze, Blink, Lift, Tether, Splash, Block, Spit, Slam, Checkpoint, Fizzle, Select,
    ];
    let map = all.iter().map(|s| (*s, sources.add(wav(&render(*s))))).collect();
    commands.insert_resource(Sounds { map });
    let amb = sources.add(wav(&drone()));
    commands.spawn((
        AudioPlayer(amb),
        PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.55), ..default() },
    ));
}

pub fn play_sfx(mut commands: Commands, mut msgs: MessageReader<PlaySfx>, sounds: Res<Sounds>) {
    let mut played: Vec<Sfx> = Vec::new();
    for PlaySfx(s, vol) in msgs.read() {
        if played.contains(s) {
            continue; // one of each per frame is plenty
        }
        played.push(*s);
        if let Some((_, h)) = sounds.map.iter().find(|(k, _)| k == s) {
            commands.spawn((
                AudioPlayer(h.clone()),
                PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(*vol), ..default() },
            ));
        }
    }
}
