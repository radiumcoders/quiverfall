//! Particles, lights, screen shake, ambient motes.

use bevy::prelude::*;

use crate::art::{Art, palette};
use crate::game::LevelEntity;
use crate::level::Level;

#[derive(Clone, Copy)]
pub enum BurstKind {
    Dust,
    Hit,
    Embers,
    Frost,
    Splash,
    Sparks,
    Blink,
    Death,
    Trail,
    Splat,
}

#[derive(Message, Clone, Copy)]
pub struct Burst {
    pub pos: Vec2,
    pub n: u32,
    pub kind: BurstKind,
    pub color: Color,
}

impl Burst {
    fn new(pos: Vec2, n: u32, kind: BurstKind, color: Color) -> Self {
        Self { pos, n, kind, color }
    }
    pub fn dust(pos: Vec2, n: u32) -> Self {
        Self::new(pos, n, BurstKind::Dust, Color::srgba(0.7, 0.72, 0.75, 0.5))
    }
    pub fn hit(pos: Vec2, n: u32, c: Color) -> Self {
        Self::new(pos, n, BurstKind::Hit, c)
    }
    pub fn embers(pos: Vec2, n: u32) -> Self {
        Self::new(pos, n, BurstKind::Embers, Color::srgb(3.0, 1.3, 0.3))
    }
    pub fn frost(pos: Vec2, n: u32) -> Self {
        Self::new(pos, n, BurstKind::Frost, Color::srgb(1.4, 2.0, 2.8))
    }
    pub fn splash(pos: Vec2) -> Self {
        Self::new(pos, 10, BurstKind::Splash, Color::srgba(0.6, 0.9, 1.0, 0.8))
    }
    pub fn sparks(pos: Vec2, n: u32) -> Self {
        Self::new(pos, n, BurstKind::Sparks, Color::srgb(3.0, 2.4, 1.2))
    }
    pub fn blink(pos: Vec2) -> Self {
        Self::new(pos, 22, BurstKind::Blink, Color::srgb(1.8, 0.6, 3.0))
    }
    pub fn death(pos: Vec2, n: u32) -> Self {
        Self::new(pos, n, BurstKind::Death, Color::srgb(2.0, 1.4, 0.8))
    }
    pub fn trail(pos: Vec2, c: Color) -> Self {
        Self::new(pos, 1, BurstKind::Trail, c)
    }
    pub fn trail_n(pos: Vec2, c: Color, n: u32) -> Self {
        Self::new(pos, n, BurstKind::Blink, c)
    }
    pub fn splat(pos: Vec2) -> Self {
        Self::new(pos, 8, BurstKind::Splat, Color::srgb(1.2, 1.8, 0.3))
    }
}

#[derive(Component)]
pub struct Particle {
    vel: Vec2,
    life: f32,
    max: f32,
    grav: f32,
    drag: f32,
    s0: f32,
    s1: f32,
    color: Color,
}

#[derive(Component)]
pub struct Mote {
    vel: Vec2,
    life: f32,
    max: f32,
}

/// Point light: a soft additive-looking glow behind the entity (bloom does the rest).
#[derive(Component, Clone, Copy)]
pub struct Light {
    pub color: Color,
    pub radius: f32,
    pub flicker: f32,
}

#[derive(Component)]
pub struct LightGlow;

#[derive(Resource, Default)]
pub struct Shake {
    pub trauma: f32,
}

impl Shake {
    pub fn add(&mut self, t: f32) {
        self.trauma = (self.trauma + t).min(1.0);
    }
}

fn rnd() -> f32 {
    rand::random::<f32>()
}

fn rnd_dir() -> Vec2 {
    Vec2::from_angle(rnd() * std::f32::consts::TAU)
}

pub fn spawn_bursts(mut commands: Commands, art: Res<Art>, mut msgs: MessageReader<Burst>) {
    for b in msgs.read() {
        for _ in 0..b.n {
            let (vel, life, grav, drag, s0, s1) = match b.kind {
                BurstKind::Dust => (Vec2::new((rnd() - 0.5) * 90.0, rnd() * 50.0), 0.5, -20.0, 2.0, 7.0, 14.0),
                BurstKind::Hit => (rnd_dir() * (120.0 + rnd() * 200.0), 0.3, 0.0, 4.0, 5.0, 0.0),
                BurstKind::Embers => (Vec2::new((rnd() - 0.5) * 60.0, 30.0 + rnd() * 90.0), 0.8 + rnd() * 0.5, -60.0, 1.0, 4.0, 0.0),
                BurstKind::Frost => (rnd_dir() * (40.0 + rnd() * 120.0), 0.7, 120.0, 2.0, 5.0, 0.0),
                BurstKind::Splash => (Vec2::new((rnd() - 0.5) * 160.0, 120.0 + rnd() * 160.0), 0.6, 900.0, 0.5, 5.0, 2.0),
                BurstKind::Sparks => (rnd_dir() * (150.0 + rnd() * 250.0), 0.25, 600.0, 2.0, 3.0, 0.0),
                BurstKind::Blink => (rnd_dir() * (60.0 + rnd() * 160.0), 0.5, 0.0, 3.0, 6.0, 0.0),
                BurstKind::Death => (rnd_dir() * (80.0 + rnd() * 260.0), 0.7 + rnd() * 0.4, 100.0, 2.0, 7.0, 0.0),
                BurstKind::Trail => (rnd_dir() * 15.0, 0.35, 0.0, 1.0, 5.0, 0.0),
                BurstKind::Splat => (Vec2::new((rnd() - 0.5) * 160.0, rnd() * 160.0), 0.45, 800.0, 0.5, 5.0, 1.0),
            };
            commands.spawn((
                LevelEntity,
                Particle { vel, life, max: life, grav, drag, s0, s1, color: b.color },
                Sprite { image: art.dot.clone(), custom_size: Some(Vec2::splat(s0)), color: b.color, ..default() },
                Transform::from_translation((b.pos + rnd_dir() * rnd() * 6.0).extend(8.0)),
            ));
        }
    }
}

pub fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut p, mut t, mut s) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = p.life / p.max;
        let drag = p.drag;
        p.vel *= (1.0 - drag * dt).max(0.0);
        p.vel.y -= p.grav * dt;
        t.translation += (p.vel * dt).extend(0.0);
        let size = p.s1 + (p.s0 - p.s1) * k;
        s.custom_size = Some(Vec2::splat(size.max(0.1)));
        let a = p.color.alpha();
        s.color = p.color.with_alpha(a * k.min(1.0).powf(0.7));
    }
}

/// Floating dust motes around the camera for depth.
pub fn ambient_motes(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    level: Option<Res<Level>>,
    cam: Query<&Transform, With<crate::MainCamera>>,
    mut q: Query<(Entity, &mut Mote, &mut Transform, &mut Sprite), Without<crate::MainCamera>>,
) {
    let Some(level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    let Ok(cam) = cam.single() else { return };
    let c = cam.translation.truncate();
    let pal = palette(level.biome);
    let count = q.iter().count();
    if count < 70 && rnd() < 0.6 {
        let pos = c + Vec2::new((rnd() - 0.5) * 1100.0, (rnd() - 0.5) * 650.0);
        let near = rnd() < 0.25;
        let a = pal.accent.to_srgba();
        commands.spawn((
            LevelEntity,
            Mote { vel: Vec2::new((rnd() - 0.5) * 12.0, 4.0 + rnd() * 12.0), life: 0.0, max: 4.0 + rnd() * 5.0 },
            Sprite {
                image: art.dot.clone(),
                custom_size: Some(Vec2::splat(if near { 4.0 } else { 2.2 })),
                color: Color::srgba(a.red, a.green, a.blue, 0.0),
                ..default()
            },
            Transform::from_translation(pos.extend(if near { 9.0 } else { -30.0 })),
        ));
    }
    for (e, mut m, mut t, mut s) in &mut q {
        m.life += dt;
        if m.life > m.max {
            commands.entity(e).despawn();
            continue;
        }
        let k = (m.life / m.max * std::f32::consts::PI).sin();
        t.translation.x += (m.vel.x + (time.elapsed_secs() + t.translation.y * 0.01).sin() * 6.0) * dt;
        t.translation.y += m.vel.y * dt;
        s.color = s.color.with_alpha(0.55 * k);
    }
}

pub fn attach_lights(mut commands: Commands, art: Res<Art>, added: Query<(Entity, &Light), Added<Light>>) {
    for (e, l) in &added {
        commands.entity(e).with_children(|c| {
            c.spawn((
                LightGlow,
                Sprite { image: art.glow.clone(), custom_size: Some(Vec2::splat(l.radius * 2.0)), color: l.color.with_alpha(0.22), ..default() },
                Transform::from_xyz(0.0, 0.0, -0.6),
            ));
        });
    }
}

pub fn flicker_lights(
    time: Res<Time>,
    lights: Query<(&Light, &Children)>,
    mut glows: Query<(&mut Sprite, &mut Transform), With<LightGlow>>,
) {
    let t = time.elapsed_secs();
    for (l, children) in &lights {
        for c in children.iter() {
            if let Ok((mut s, mut tr)) = glows.get_mut(c) {
                let f = 1.0 + l.flicker * ((t * 13.0).sin() * 0.6 + (t * 7.3).sin() * 0.4);
                s.color = l.color.with_alpha(0.22 * f);
                s.custom_size = Some(Vec2::splat(l.radius * 2.0 * f));
                // keep the glow upright even if the parent rotates (arrows)
                tr.rotation = Quat::IDENTITY;
            }
        }
    }
}
