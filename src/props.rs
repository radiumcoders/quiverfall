//! Level props: lanterns (checkpoints), shrines, targets, braziers, anchors, exit, hints,
//! plus lifts and moving platforms.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::arrows::ArrowKind;
use crate::art::{self, Art};
use crate::audio::{PlaySfx, Sfx};
use crate::config::*;
use crate::fx::{Burst, Light, Shake};
use crate::game::{AppState, Checkpoint, LevelEntity, Progress};
use crate::hud::{Banner, HintText};
use crate::level::{LEVEL_SOURCES, Level, Place};
use crate::physics::Platform;
use crate::player::Player;

#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum Prop {
    Target { id: u8, hit: bool },
    Brazier { id: u8, lit: bool },
    Anchor,
    Lantern { lit: bool },
    Shrine { kind: ArrowKind, taken: bool },
    Exit,
    Hint(usize),
}

#[derive(Component)]
pub struct Lift {
    pub start: Vec2,
    pub risen: f32,
    pub done: bool,
    pub wall: f32,
}

#[derive(Component)]
pub struct Mover {
    pub origin: Vec2,
    pub offset: Vec2,
    pub speed: f32,
    pub t: f32,
}

#[derive(Component)]
pub struct ShrineIcon {
    pub base: Vec2,
}

pub fn spawn_prop(commands: &mut Commands, art: &Art, s: Place, floor: Vec2, _level: &Level) {
    let tile_c = floor + Vec2::new(0.0, TILE / 2.0);
    let sprite = |img: &Handle<Image>, h: f32, w_over_h: f32| Sprite {
        image: img.clone(),
        custom_size: Some(Vec2::new(h * w_over_h, h)),
        ..default()
    };
    match s {
        Place::Lantern => {
            commands.spawn((
                LevelEntity,
                Prop::Lantern { lit: false },
                sprite(&art.props[art::PROP_LANTERN], 58.0, 62.0 / 192.0),
                Anchor::BOTTOM_CENTER,
                Transform::from_translation(floor.extend(2.0)),
            ));
        }
        Place::Target(id) => {
            commands.spawn((
                LevelEntity,
                Prop::Target { id, hit: false },
                sprite(&art.props[art::PROP_TARGET], 40.0, 131.0 / 192.0),
                Anchor(Vec2::new(0.08, -0.12)),
                Transform::from_translation(tile_c.extend(2.0)),
            ));
        }
        Place::Brazier(id) => {
            commands.spawn((
                LevelEntity,
                Prop::Brazier { id, lit: false },
                sprite(&art.props[art::PROP_BRAZIER], 34.0, 144.0 / 192.0),
                Anchor(Vec2::new(0.0, -0.5 + 18.0 / 34.0)),
                Transform::from_translation((floor + Vec2::new(0.0, 18.0)).extend(2.0)),
            ));
        }
        Place::Anchor => {
            commands.spawn((
                LevelEntity,
                Prop::Anchor,
                sprite(&art.props[art::PROP_ANCHOR], 40.0, 118.0 / 192.0),
                Anchor(Vec2::new(0.0, 0.18)),
                Transform::from_translation(tile_c.extend(2.0)),
                Light { color: Color::srgb(1.0, 0.8, 0.45), radius: 36.0, flicker: 0.0 },
            ));
        }
        Place::Shrine(kind) => {
            commands.spawn((
                LevelEntity,
                Prop::Shrine { kind, taken: false },
                sprite(&art.props[art::PROP_SHRINE], 66.0, 145.0 / 192.0),
                Anchor::BOTTOM_CENTER,
                Transform::from_translation(floor.extend(2.0)),
                Light { color: kind.color(), radius: 90.0, flicker: 0.05 },
            ));
            let base = floor + Vec2::new(0.0, 84.0);
            commands.spawn((
                LevelEntity,
                ShrineIcon { base },
                Sprite { image: art.arrows[kind.index()].clone(), custom_size: Some(Vec2::new(40.0, 7.0)), color: Color::srgb(1.6, 1.6, 1.6), ..default() },
                Transform::from_translation(base.extend(2.1)).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ));
        }
        Place::Exit => {
            commands.spawn((
                LevelEntity,
                Prop::Exit,
                sprite(&art.exit, 120.0, 246.0 / 400.0),
                Anchor::BOTTOM_CENTER,
                Transform::from_translation(floor.extend(1.0)),
                Light { color: Color::srgb(1.6, 1.4, 0.9), radius: 150.0, flicker: 0.03 },
            ));
        }
        Place::Hint(i) => {
            commands.spawn((
                LevelEntity,
                Prop::Hint(i),
                Sprite { image: art.glow.clone(), custom_size: Some(Vec2::splat(18.0)), color: Color::srgb(0.9, 1.4, 1.3), ..default() },
                Transform::from_translation((floor + Vec2::new(0.0, 10.0)).extend(1.5)),
            ));
        }
        _ => {}
    }
}

/// Lifts rise; movers patrol. Runs before the player so riders get carried.
pub fn platforms_update(
    time: Res<Time>,
    level: Option<Res<Level>>,
    mut lifts: Query<(&mut Lift, &mut Platform, &mut Transform), Without<Mover>>,
    mut movers: Query<(&mut Mover, &mut Platform, &mut Transform), Without<Lift>>,
    mut statics: Query<&mut Platform, (Without<Lift>, Without<Mover>)>,
) {
    let Some(level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    for mut p in &mut statics {
        p.delta = Vec2::ZERO;
    }
    for (mut l, mut p, mut t) in &mut lifts {
        p.delta = Vec2::ZERO;
        if l.done {
            continue;
        }
        let step = LIFT_SPEED * dt;
        let c = t.translation.truncate();
        // headroom check: room for the player above the platform after moving
        let top = c.y + p.half.y + step;
        let probe_c = Vec2::new(c.x, top + PLAYER_HALF.1 + 2.0);
        let probe_h = Vec2::new(p.half.x - 12.0, PLAYER_HALF.1 + 1.0);
        let blocked = level.box_solid(probe_c, probe_h);
        if blocked || l.risen >= LIFT_RISE {
            l.done = true;
            continue;
        }
        l.risen += step;
        t.translation.y += step;
        p.delta = Vec2::new(0.0, step);
        let _ = l.start;
    }
    for (mut m, mut p, mut t) in &mut movers {
        let len = m.offset.length().max(1.0);
        let period = len / m.speed.max(1.0);
        m.t += dt;
        let ph = (m.t / period) % 2.0;
        let s = if ph < 1.0 { ph } else { 2.0 - ph };
        let s = s * s * (3.0 - 2.0 * s);
        let new = m.origin + m.offset * s;
        let old = t.translation.truncate();
        p.delta = new - old;
        t.translation.x = new.x;
        t.translation.y = new.y;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn props_update(
    time: Res<Time>,
    art: Res<Art>,
    level: Option<ResMut<Level>>,
    mut props: Query<(&mut Prop, &Transform, &mut Sprite, Entity), Without<ShrineIcon>>,
    mut icons: Query<(&ShrineIcon, &mut Transform, &mut Sprite), Without<Prop>>,
    mut players: Query<(&mut Player, &Transform), (Without<Prop>, Without<ShrineIcon>)>,
    mut progress: ResMut<Progress>,
    mut checkpoint: ResMut<Checkpoint>,
    mut next: ResMut<NextState<AppState>>,
    mut banner: ResMut<Banner>,
    mut hint: ResMut<HintText>,
    mut sfx: MessageWriter<PlaySfx>,
    mut fx: MessageWriter<Burst>,
    mut shake: ResMut<Shake>,
    mut commands: Commands,
) {
    let Some(mut level) = level else { return };
    let Ok((mut player, ptf)) = players.single_mut() else { return };
    let ppos = ptf.translation.truncate();
    let now = time.elapsed_secs();
    hint.0 = None;
    let mut switches: [(u32, u32); 16] = [(0, 0); 16];
    for (mut prop, tf, mut sprite, ent) in &mut props {
        let pos = tf.translation.truncate();
        match &mut *prop {
            Prop::Lantern { lit } => {
                if !*lit && player.hp > 0 && (ppos - (pos + Vec2::new(0.0, 22.0))).length() < 40.0 {
                    *lit = true;
                    checkpoint.pos = Some(pos + Vec2::new(0.0, PLAYER_HALF.1 + 1.0));
                    player.hp = MAX_HP;
                    player.charges = MAX_CHARGES;
                    sprite.image = art.props[art::PROP_LANTERN_LIT].clone();
                    commands.entity(ent).insert(Light { color: Color::srgb(2.0, 1.4, 0.6), radius: 110.0, flicker: 0.12 });
                    sfx.write(PlaySfx(Sfx::Checkpoint, 0.7));
                    fx.write(Burst::embers(pos + Vec2::new(0.0, 40.0), 14));
                }
            }
            Prop::Target { id, hit } => {
                switches[*id as usize].0 += 1;
                if *hit {
                    switches[*id as usize].1 += 1;
                    if sprite.image != art.props[art::PROP_TARGET_HIT] {
                        sprite.image = art.props[art::PROP_TARGET_HIT].clone();
                        sprite.custom_size = Some(Vec2::new(40.0 * 168.0 / 192.0, 40.0));
                    }
                }
            }
            Prop::Brazier { id, lit } => {
                switches[*id as usize].0 += 1;
                if *lit {
                    switches[*id as usize].1 += 1;
                    if sprite.image != art.props[art::PROP_BRAZIER_LIT] {
                        sprite.image = art.props[art::PROP_BRAZIER_LIT].clone();
                        sprite.custom_size = Some(Vec2::new(50.0 * 97.0 / 192.0, 50.0));
                        commands.entity(ent).insert((
                            Light { color: Color::srgb(2.4, 1.2, 0.4), radius: 100.0, flicker: 0.2 },
                            Anchor(Vec2::new(0.0, -0.5 + 18.0 / 50.0)),
                        ));
                    }
                    if rand::random::<f32>() < 0.3 {
                        fx.write(Burst::embers(pos + Vec2::new(0.0, 14.0), 1));
                    }
                }
            }
            Prop::Shrine { kind, taken } => {
                if !*taken && progress.unlocked[kind.index()] && player.hp > 0 {
                    // already owned (e.g. after a respawn)
                    *taken = true;
                }
                if *taken && sprite.color != Color::srgb(0.55, 0.55, 0.6) {
                    sprite.color = Color::srgb(0.55, 0.55, 0.6);
                    commands.entity(ent).remove::<Light>();
                }
                if !*taken && (ppos - (pos + Vec2::new(0.0, 30.0))).length() < 44.0 {
                    *taken = true;
                    progress.unlocked[kind.index()] = true;
                    player.selected = kind.index();
                    player.charges = MAX_CHARGES;
                    banner.unlock(*kind);
                    sfx.write(PlaySfx(Sfx::Pickup, 0.8));
                    shake.add(0.3);
                    fx.write(Burst::trail_n(pos + Vec2::new(0.0, 84.0), kind.color(), 40));
                }
            }
            Prop::Exit => {
                if player.hp > 0 && (ppos.x - pos.x).abs() < 30.0 && (ppos.y - (pos.y + 30.0)).abs() < 40.0 {
                    sfx.write(PlaySfx(Sfx::Pickup, 0.8));
                    if level.index + 1 >= LEVEL_SOURCES.len() {
                        next.set(AppState::Victory);
                    } else {
                        next.set(AppState::LevelDone);
                    }
                }
            }
            Prop::Hint(i) => {
                let d = (ppos - pos).length();
                let k = 0.6 + 0.4 * (now * 3.0 + pos.x).sin();
                sprite.color = Color::srgb(0.9 * k + 0.3, 1.4 * k + 0.3, 1.3 * k + 0.3);
                if d < 90.0 {
                    hint.0 = level.hints.get(*i).cloned();
                }
            }
            Prop::Anchor => {}
        }
    }
    for (icon, mut t, mut s) in &mut icons {
        t.translation.y = icon.base.y + (now * 2.0).sin() * 5.0;
        let taken = props.iter().any(|(p, pt, _, _)| {
            matches!(p, Prop::Shrine { taken: true, .. }) && (pt.translation.x - icon.base.x).abs() < 1.0
        });
        if taken {
            s.color = s.color.with_alpha((s.color.alpha() - 0.03).max(0.0));
        }
    }
    for (id, (n, on)) in switches.iter().enumerate() {
        if *n > 0 && on == n && !level.gate_open[id] {
            level.gate_open[id] = true;
            sfx.write(PlaySfx(Sfx::Gate, 0.8));
            shake.add(0.35);
        }
    }
}
