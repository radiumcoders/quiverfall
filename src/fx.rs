//! Particles, one-shot effect sprites (flashes), lights, screen shake, ambient motes.

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

/// One of the generated effect sprites (`assets/art/fx_*.png`), same order as `art::FX_NAMES`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FxKind {
    Slash,
    Impact,
    Smoke,
    Flame,
    Frost,
    Void,
    Ring,
    Spark,
}

/// A one-shot effect sprite that grows/shrinks, spins and fades over its life.
#[derive(Message, Clone, Copy)]
pub struct Flash {
    pub pos: Vec2,
    pub kind: FxKind,
    pub size: (f32, f32),
    pub life: f32,
    pub color: Color,
    pub angle: f32,
    pub spin: f32,
    pub vel: Vec2,
    pub flip: bool,
    /// stays glued to this entity (offset = pos relative to it)
    pub follow: Option<Entity>,
}

impl Flash {
    pub fn new(pos: Vec2, kind: FxKind, size: (f32, f32), life: f32, color: Color) -> Self {
        Self { pos, kind, size, life, color, angle: 0.0, spin: 0.0, vel: Vec2::ZERO, flip: false, follow: None }
    }
    pub fn angle(mut self, a: f32) -> Self {
        self.angle = a;
        self
    }
    pub fn spin(mut self, s: f32) -> Self {
        self.spin = s;
        self
    }
    pub fn vel(mut self, v: Vec2) -> Self {
        self.vel = v;
        self
    }
    pub fn flip(mut self, f: bool) -> Self {
        self.flip = f;
        self
    }
    pub fn follow(mut self, e: Entity) -> Self {
        self.follow = Some(e);
        self
    }
}

#[derive(Component)]
pub struct FlashFx {
    f: Flash,
    t: f32,
    offset: Vec2,
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

/// The painted flash that accompanies a particle burst, if any.
fn burst_flashes(b: &Burst, out: &mut Vec<Flash>) {
    let p = b.pos;
    let ang = rnd() * std::f32::consts::TAU;
    match b.kind {
        BurstKind::Hit => {
            let s = 34.0 + b.n as f32 * 3.0;
            out.push(Flash::new(p, FxKind::Impact, (s * 0.6, s * 1.4), 0.16, glow(b.color, 1.8)).angle(ang));
        }
        BurstKind::Embers if b.n >= 6 => {
            out.push(Flash::new(p, FxKind::Flame, (34.0, 58.0), 0.45, Color::srgb(2.4, 1.5, 0.8)).vel(Vec2::new(0.0, 50.0)));
            out.push(Flash::new(p, FxKind::Impact, (20.0, 60.0), 0.18, Color::srgb(2.6, 1.4, 0.5)).angle(ang));
        }
        BurstKind::Frost if b.n >= 8 => {
            out.push(Flash::new(p, FxKind::Frost, (30.0, 76.0), 0.35, Color::srgb(1.5, 2.0, 2.4)).angle(ang).spin(0.8));
        }
        BurstKind::Sparks if b.n >= 4 => {
            out.push(Flash::new(p, FxKind::Spark, (44.0, 8.0), 0.22, Color::srgb(2.6, 2.3, 1.6)).angle(ang * 0.1));
        }
        BurstKind::Blink => {
            out.push(Flash::new(p, FxKind::Void, (24.0, 104.0), 0.42, Color::srgb(1.8, 1.0, 2.6)).angle(ang).spin(-5.0));
        }
        BurstKind::Death => {
            let big = b.n > 30;
            out.push(Flash::new(p, FxKind::Ring, (20.0, if big { 420.0 } else { 150.0 }), if big { 0.8 } else { 0.45 }, Color::srgb(2.2, 1.8, 1.2)).angle(ang));
            out.push(Flash::new(p, FxKind::Impact, (30.0, if big { 220.0 } else { 90.0 }), 0.22, Color::srgb(2.4, 2.0, 1.5)).angle(ang));
        }
        BurstKind::Dust if b.n >= 4 => {
            let big = b.n >= 20;
            let puffs = if big { 6 } else { 2 };
            for i in 0..puffs {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let sp = if big { 90.0 } else { 40.0 };
                out.push(
                    Flash::new(p + Vec2::new(side * 6.0, 4.0), FxKind::Smoke, (14.0, if big { 70.0 } else { 34.0 }), 0.5 + rnd() * 0.2, Color::srgba(0.75, 0.78, 0.82, 0.55))
                        .vel(Vec2::new(side * sp * (0.5 + rnd()), 12.0 + rnd() * 20.0))
                        .angle(ang + i as f32)
                        .spin(side * 0.6)
                        .flip(side > 0.0),
                );
            }
            if big {
                out.push(Flash::new(p, FxKind::Ring, (40.0, 320.0), 0.5, Color::srgba(1.6, 1.4, 1.1, 0.8)));
            }
        }
        _ => {}
    }
}

fn glow(c: Color, k: f32) -> Color {
    let l = c.to_linear();
    Color::linear_rgba(l.red * k, l.green * k, l.blue * k, l.alpha)
}

pub fn spawn_flashes(mut commands: Commands, art: Res<Art>, mut msgs: MessageReader<Flash>, follow: Query<&Transform>) {
    for f in msgs.read() {
        spawn_flash(&mut commands, &art, *f, &follow);
    }
}

fn spawn_flash(commands: &mut Commands, art: &Art, f: Flash, follow: &Query<&Transform>) {
    let offset = match f.follow.and_then(|e| follow.get(e).ok()) {
        Some(t) => f.pos - t.translation.truncate(),
        None => Vec2::ZERO,
    };
    commands.spawn((
        LevelEntity,
        FlashFx { f, t: 0.0, offset },
        Sprite {
            image: art.fx[f.kind as usize].clone(),
            custom_size: Some(Vec2::splat(f.size.0)),
            color: f.color,
            flip_x: f.flip,
            ..default()
        },
        Transform::from_translation(f.pos.extend(8.5)).with_rotation(Quat::from_rotation_z(f.angle)),
    ));
}

pub fn update_flashes(
    mut commands: Commands,
    time: Res<Time>,
    images: Res<Assets<Image>>,
    art: Res<Art>,
    targets: Query<&Transform, Without<FlashFx>>,
    mut q: Query<(Entity, &mut FlashFx, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (e, mut fx, mut t, mut s) in &mut q {
        fx.t += dt;
        let k = fx.t / fx.f.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let ease = 1.0 - (1.0 - k).powi(3);
        let size = fx.f.size.0 + (fx.f.size.1 - fx.f.size.0) * ease;
        // keep the painted aspect ratio, fit the long side to `size`
        let img = crate::art::img_size(&images, &art.fx[fx.f.kind as usize]);
        let aspect = if img.y > 0.0 { img.x / img.y } else { 1.0 };
        s.custom_size = Some(if aspect >= 1.0 { Vec2::new(size, size / aspect) } else { Vec2::new(size * aspect, size) });
        // quick pop in, smooth fade out
        let a = (k / 0.12).min(1.0) * (1.0 - k).powf(1.4);
        s.color = fx.f.color.with_alpha(fx.f.color.alpha() * a);
        let vel = fx.f.vel;
        fx.offset += vel * dt;
        fx.f.vel *= (1.0 - 2.5 * dt).max(0.0);
        let base = fx.f.follow.and_then(|f| targets.get(f).ok()).map(|t| t.translation.truncate());
        let pos = base.unwrap_or(fx.f.pos) + fx.offset;
        t.translation.x = pos.x;
        t.translation.y = pos.y;
        t.rotate_z(fx.f.spin * dt);
    }
}

pub fn spawn_bursts(mut commands: Commands, art: Res<Art>, mut msgs: MessageReader<Burst>, follow: Query<&Transform>) {
    let mut flashes = Vec::new();
    for b in msgs.read() {
        burst_flashes(b, &mut flashes);
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
    for f in flashes {
        spawn_flash(&mut commands, &art, f, &follow);
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

pub fn attach_lights(
    mut commands: Commands,
    art: Res<Art>,
    added: Query<(Entity, &Light, Option<&Sprite>, Option<&bevy::sprite::Anchor>), Added<Light>>,
) {
    for (e, l, sprite, anchor) in &added {
        // centre the glow on the sprite, not on its anchor point
        let size = sprite.and_then(|s| s.custom_size).unwrap_or(Vec2::ZERO);
        let off = -anchor.map(|a| a.0).unwrap_or(Vec2::ZERO) * size;
        commands.entity(e).with_children(|c| {
            c.spawn((
                LightGlow,
                Sprite { image: art.glow.clone(), custom_size: Some(Vec2::splat(l.radius * 2.0)), color: l.color.with_alpha(0.22), ..default() },
                Transform::from_xyz(off.x, off.y, -0.6),
            ));
        });
    }
}

/// Lights that were switched off take their glow with them.
pub fn remove_lights(
    mut commands: Commands,
    mut removed: RemovedComponents<Light>,
    children: Query<&Children>,
    glows: Query<(), With<LightGlow>>,
) {
    for e in removed.read() {
        if let Ok(ch) = children.get(e) {
            for c in ch.iter() {
                if glows.contains(c) {
                    commands.entity(c).despawn();
                }
            }
        }
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
                tr.rotation = Quat::IDENTITY;
            }
        }
    }
}
