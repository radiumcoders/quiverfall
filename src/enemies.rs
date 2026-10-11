//! Enemies, their projectiles, the Warden boss, and player<->enemy contact/melee.

use bevy::prelude::*;

use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::config::*;
use crate::fx::{Burst, Light, Shake};
use crate::game::LevelEntity;
use crate::level::{Level, Tile};
use crate::physics::{Body, PlatRef, Platform, aabb_overlap, ledge_ahead, move_body};
use crate::player::Player;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Crawler,
    Hopper,
    Moth,
    Spitter,
    Shieldbearer,
    Warden,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub hp: i32,
    pub max_hp: i32,
    pub dir: f32,
    pub t: f32,
    pub state: u8,
    pub frozen: f32,
    pub burn: f32,
    pub burn_tick: f32,
    pub flash: f32,
    pub stun: f32,
    pub home: Vec2,
    pub anim: f32,
    pub turn_t: f32,
    pub awake: bool,
}

impl Enemy {
    pub fn damage(&mut self, n: i32) {
        self.hp -= n;
        self.flash = 0.12;
    }
}

#[derive(Component)]
pub struct EnemySprite {
    pub height: f32,
}

#[derive(Component)]
pub struct Glob {
    pub vel: Vec2,
}

#[derive(Component)]
pub struct Wave {
    pub dir: f32,
    pub life: f32,
}

/// The boss's health bar reads this.
#[derive(Resource, Default)]
pub struct BossBar {
    pub active: bool,
    pub frac: f32,
}

fn stats(kind: EnemyKind) -> (Vec2, i32, f32) {
    // body half-size, hp, visual height
    match kind {
        EnemyKind::Crawler => (Vec2::new(14.0, 9.0), 2, 26.0),
        EnemyKind::Hopper => (Vec2::new(12.0, 12.0), 3, 36.0),
        EnemyKind::Moth => (Vec2::new(11.0, 11.0), 2, 38.0),
        EnemyKind::Spitter => (Vec2::new(13.0, 16.0), 3, 42.0),
        EnemyKind::Shieldbearer => (Vec2::new(11.0, 21.0), 5, 52.0),
        EnemyKind::Warden => (Vec2::new(34.0, 40.0), 30, 118.0),
    }
}

pub fn frames(art: &Art, kind: EnemyKind) -> &Vec<Handle<Image>> {
    match kind {
        EnemyKind::Crawler => &art.crawler,
        EnemyKind::Hopper => &art.hopper,
        EnemyKind::Moth => &art.moth,
        EnemyKind::Spitter => &art.spitter,
        EnemyKind::Shieldbearer => &art.shieldbearer,
        EnemyKind::Warden => &art.warden,
    }
}

pub fn spawn_enemy(commands: &mut Commands, art: &Art, kind: EnemyKind, floor: Vec2) {
    let (half, hp, vis_h) = stats(kind);
    let pos = floor + Vec2::new(0.0, half.y + 0.5);
    let mut body = Body::new(half);
    body.no_collide = kind == EnemyKind::Moth;
    let flying = kind == EnemyKind::Moth;
    commands
        .spawn((
            LevelEntity,
            Enemy {
                kind,
                hp,
                max_hp: hp,
                dir: -1.0,
                t: 1.0 + rand::random::<f32>(),
                state: 0,
                frozen: 0.0,
                burn: 0.0,
                burn_tick: 0.0,
                flash: 0.0,
                stun: 0.0,
                home: pos,
                anim: rand::random::<f32>() * 4.0,
                turn_t: 0.0,
                awake: false,
            },
            body,
            Transform::from_translation(pos.extend(5.0)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                EnemySprite { height: vis_h },
                Sprite { image: frames(art, kind)[0].clone(), custom_size: Some(Vec2::new(vis_h, vis_h)), ..default() },
                if flying { bevy::sprite::Anchor::CENTER } else { bevy::sprite::Anchor::BOTTOM_CENTER },
                Transform::from_xyz(0.0, if flying { 0.0 } else { -half.y - 1.0 }, 0.0),
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn enemies_update(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    level: Option<ResMut<Level>>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Body, &mut Transform, Has<Platform>), Without<Player>>,
    mut players: Query<(&mut Player, &mut Body, &Transform), Without<Enemy>>,
    plats: Query<(Entity, &Transform, &Platform), (Without<Enemy>, Without<Player>)>,
    mut sfx: MessageWriter<PlaySfx>,
    mut fx: MessageWriter<Burst>,
    mut shake: ResMut<Shake>,
    mut bar: ResMut<BossBar>,
) {
    let Some(mut level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    let Ok((mut player, mut pbody, ptf)) = players.single_mut() else { return };
    let ppos = ptf.translation.truncate();
    let mut plat_refs: Vec<PlatRef> = plats
        .iter()
        .map(|(e, t, p)| PlatRef { e, center: t.translation.truncate(), half: p.half, delta: p.delta })
        .collect();
    // frozen enemies are platforms too (for each other)
    for (e, en, b, t, _) in &enemies {
        if en.frozen > 0.0 {
            plat_refs.push(PlatRef { e, center: t.translation.truncate(), half: b.half, delta: Vec2::ZERO });
        }
    }
    let mut spawn_moths: Vec<Vec2> = vec![];
    bar.active = false;

    for (me, mut e, mut body, mut tf, is_plat) in &mut enemies {
        let mut pos = tf.translation.truncate();
        e.flash -= dt;
        e.anim += dt;
        let to_p = ppos - pos;

        // ---- death ----
        if e.hp <= 0 {
            fx.write(Burst::death(pos, if e.kind == EnemyKind::Warden { 60 } else { 16 }));
            sfx.write(PlaySfx(Sfx::EnemyDie, 0.7));
            shake.add(if e.kind == EnemyKind::Warden { 1.0 } else { 0.2 });
            if e.kind == EnemyKind::Warden {
                level.boss_active = false;
                level.gate_open[8] = true;
                sfx.write(PlaySfx(Sfx::Gate, 0.8));
            }
            player.charges = (player.charges + 0.5).min(MAX_CHARGES);
            commands.entity(me).despawn();
            continue;
        }
        if e.kind == EnemyKind::Warden && e.awake {
            bar.active = true;
            bar.frac = e.hp as f32 / e.max_hp as f32;
        }

        // ---- status ----
        if e.burn > 0.0 {
            e.burn -= dt;
            e.burn_tick -= dt;
            if e.burn_tick <= 0.0 {
                e.burn_tick = 0.6;
                e.damage(1);
            }
            if rand::random::<f32>() < 0.4 {
                fx.write(Burst::embers(pos, 1));
            }
        }
        if e.frozen > 0.0 {
            e.frozen -= dt;
            body.vel.x = 0.0;
            if !is_plat {
                commands.entity(me).insert(Platform { half: body.half, delta: Vec2::ZERO });
            }
            if e.kind == EnemyKind::Moth {
                body.vel.y = (body.vel.y - GRAVITY_DOWN * dt).max(-MAX_FALL);
                body.no_collide = false;
            } else {
                body.vel.y = (body.vel.y - GRAVITY_DOWN * dt).max(-MAX_FALL);
            }
            if e.frozen <= 0.0 {
                commands.entity(me).remove::<Platform>();
                fx.write(Burst::frost(pos, 10));
                if e.kind == EnemyKind::Moth {
                    body.no_collide = true;
                    e.home = pos;
                    e.state = 0;
                }
            }
            move_body(&mut pos, &mut body, dt, &level, &plat_refs, me);
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
            continue;
        }
        if e.stun > 0.0 {
            e.stun -= dt;
            body.vel.x *= 0.9;
            body.vel.y -= GRAVITY_DOWN * dt;
            if e.kind == EnemyKind::Moth {
                body.vel *= 0.9;
            }
            move_body(&mut pos, &mut body, dt, &level, &plat_refs, me);
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
            continue;
        }

        // ---- AI ----
        let grav = |b: &mut Body| b.vel.y = (b.vel.y - GRAVITY_DOWN * dt).max(-MAX_FALL);
        match e.kind {
            EnemyKind::Crawler => {
                body.vel.x = e.dir * 60.0;
                grav(&mut body);
                if body.on_ground && (body.wall != 0 || ledge_ahead(&level, pos, body.half, e.dir)) {
                    e.dir = -e.dir;
                }
                if body.wall != 0 {
                    e.dir = -body.wall as f32;
                }
            }
            EnemyKind::Hopper => {
                grav(&mut body);
                match e.state {
                    0 => {
                        body.vel.x *= 0.8;
                        e.dir = to_p.x.signum();
                        e.t -= dt;
                        if e.t <= 0.0 && to_p.length() < 300.0 && body.on_ground {
                            e.state = 1;
                            e.t = 0.35;
                        }
                    }
                    1 => {
                        body.vel.x = 0.0;
                        e.t -= dt;
                        if e.t <= 0.0 {
                            body.vel = Vec2::new(to_p.x.clamp(-260.0, 260.0), 640.0);
                            e.state = 2;
                            e.t = 0.1;
                        }
                    }
                    _ => {
                        e.t -= dt;
                        if body.on_ground && e.t <= 0.0 {
                            e.state = 0;
                            e.t = 0.7 + rand::random::<f32>() * 0.6;
                            fx.write(Burst::dust(pos - Vec2::new(0.0, body.half.y), 4));
                        }
                    }
                }
            }
            EnemyKind::Moth => {
                e.t -= dt;
                match e.state {
                    0 => {
                        let target = e.home + Vec2::new((e.anim * 1.3).sin() * 40.0, (e.anim * 2.1).sin() * 14.0);
                        body.vel = (target - pos) * 3.0;
                        e.dir = to_p.x.signum();
                        if e.t <= 0.0 && to_p.length() < 230.0 {
                            e.state = 1;
                            e.t = 0.5;
                        }
                    }
                    1 => {
                        body.vel = Vec2::ZERO;
                        pos.x += (rand::random::<f32>() - 0.5) * 2.0;
                        if e.t <= 0.0 {
                            e.state = 2;
                            e.t = 0.75;
                            body.vel = to_p.normalize_or_zero() * 340.0;
                        }
                    }
                    2 => {
                        if e.t <= 0.0 {
                            e.state = 3;
                        }
                    }
                    _ => {
                        let d = e.home - pos;
                        body.vel = d.normalize_or_zero() * 130.0;
                        if d.length() < 8.0 {
                            e.state = 0;
                            e.t = 1.5;
                        }
                    }
                }
            }
            EnemyKind::Spitter => {
                grav(&mut body);
                body.vel.x = 0.0;
                e.dir = to_p.x.signum();
                e.t -= dt;
                if e.state == 1 && e.t <= 0.25 {
                    e.state = 2;
                    // lob toward the player
                    let tf_ = (to_p.x.abs() / 260.0).clamp(0.6, 1.4);
                    let g = 900.0;
                    let mouth = pos + Vec2::new(e.dir * 12.0, 6.0);
                    let d = ppos - mouth;
                    let v = Vec2::new(d.x / tf_, (d.y + 0.5 * g * tf_ * tf_) / tf_);
                    commands.spawn((
                        LevelEntity,
                        Glob { vel: v },
                        Sprite { image: art.dot.clone(), custom_size: Some(Vec2::splat(12.0)), color: Color::srgb(1.4, 2.0, 0.4), ..default() },
                        Transform::from_translation(mouth.extend(7.0)),
                        Light { color: Color::srgb(0.8, 1.6, 0.2), radius: 30.0, flicker: 0.1 },
                    ));
                    sfx.write(PlaySfx(Sfx::Spit, 0.5));
                }
                if e.t <= 0.0 {
                    if e.state == 0 && to_p.length() < 460.0 {
                        e.state = 1;
                        e.t = 0.7;
                    } else {
                        e.state = 0;
                        e.t = 1.8;
                    }
                }
            }
            EnemyKind::Shieldbearer => {
                grav(&mut body);
                e.t -= dt;
                let near = to_p.length() < 320.0;
                // slow to turn around: you can get behind it
                if near && to_p.x.signum() != e.dir {
                    e.turn_t += dt;
                    if e.turn_t > 0.7 {
                        e.dir = -e.dir;
                        e.turn_t = 0.0;
                    }
                } else {
                    e.turn_t = 0.0;
                }
                match e.state {
                    0 => {
                        let speed = if near { 55.0 } else { 35.0 };
                        body.vel.x = e.dir * speed;
                        if body.on_ground && (body.wall != 0 || ledge_ahead(&level, pos, body.half, e.dir)) {
                            body.vel.x = 0.0;
                            if !near {
                                e.dir = -e.dir;
                            }
                        }
                        if near && to_p.x.abs() < 70.0 && to_p.y.abs() < 40.0 && to_p.x.signum() == e.dir && e.t <= 0.0 {
                            e.state = 1;
                            e.t = 0.4;
                        }
                    }
                    1 => {
                        body.vel.x = 0.0;
                        if e.t <= 0.0 {
                            e.state = 2;
                            e.t = 0.3;
                            body.vel.x = e.dir * 300.0;
                        }
                    }
                    _ => {
                        body.vel.x *= 0.92;
                        if e.t <= 0.0 {
                            e.state = 0;
                            e.t = 1.2;
                        }
                    }
                }
            }
            EnemyKind::Warden => {
                grav(&mut body);
                if !e.awake {
                    if to_p.x.abs() < 11.0 * TILE && to_p.y.abs() < 6.0 * TILE {
                        e.awake = true;
                        level.boss_active = true;
                        shake.add(0.8);
                        sfx.write(PlaySfx(Sfx::Slam, 0.9));
                        e.state = 0;
                        e.t = 1.4;
                    }
                } else {
                    e.t -= dt;
                    match e.state {
                        // idle
                        0 => {
                            body.vel.x *= 0.85;
                            e.dir = to_p.x.signum();
                            if e.t <= 0.0 {
                                let r = rand::random::<f32>();
                                if e.hp < e.max_hp / 2 && r < 0.25 {
                                    spawn_moths.push(pos + Vec2::new(-60.0, 90.0));
                                    spawn_moths.push(pos + Vec2::new(60.0, 90.0));
                                    sfx.write(PlaySfx(Sfx::Spit, 0.8));
                                    e.t = 1.5;
                                } else if r < 0.65 {
                                    e.state = 1;
                                    e.t = 0.55;
                                } else {
                                    e.state = 4;
                                    e.t = 0.6;
                                }
                            }
                        }
                        // leap windup
                        1 => {
                            body.vel.x = 0.0;
                            if e.t <= 0.0 {
                                e.state = 2;
                                e.t = 0.2;
                                body.vel = Vec2::new((to_p.x / 0.95).clamp(-520.0, 520.0), 900.0);
                            }
                        }
                        // airborne
                        2 => {
                            if body.on_ground && e.t <= 0.0 {
                                e.state = 3;
                                e.t = 0.7;
                                body.vel.x = 0.0;
                                shake.add(0.9);
                                sfx.write(PlaySfx(Sfx::Slam, 1.0));
                                fx.write(Burst::dust(pos - Vec2::new(0.0, body.half.y), 30));
                                for d in [-1.0, 1.0] {
                                    commands.spawn((
                                        LevelEntity,
                                        Wave { dir: d, life: 1.6 },
                                        Sprite {
                                            image: art.glow.clone(),
                                            custom_size: Some(Vec2::new(40.0, 30.0)),
                                            color: Color::srgb(2.4, 1.6, 0.7),
                                            ..default()
                                        },
                                        Transform::from_translation((pos + Vec2::new(d * 40.0, -body.half.y + 12.0)).extend(7.0)),
                                    ));
                                }
                            }
                        }
                        // slam recovery
                        3 => {
                            if e.t <= 0.0 {
                                e.state = 0;
                                e.t = 0.9;
                            }
                        }
                        // charge windup
                        4 => {
                            body.vel.x = 0.0;
                            pos.x += (rand::random::<f32>() - 0.5) * 3.0;
                            if e.t <= 0.0 {
                                e.state = 5;
                                e.t = 1.3;
                            }
                        }
                        // charging
                        _ => {
                            body.vel.x = e.dir * 430.0;
                            if rand::random::<f32>() < 0.5 {
                                fx.write(Burst::dust(pos - Vec2::new(-e.dir * 20.0, body.half.y), 1));
                            }
                            if body.wall != 0 || e.t <= 0.0 {
                                if body.wall != 0 {
                                    shake.add(0.6);
                                    sfx.write(PlaySfx(Sfx::Slam, 0.7));
                                    e.stun = 1.0;
                                    body.vel.x = -e.dir * 120.0;
                                }
                                e.state = 0;
                                e.t = 1.0;
                            }
                        }
                    }
                }
            }
        }

        move_body(&mut pos, &mut body, dt, &level, &plat_refs, me);
        // drowned / impaled enemies die too
        if e.kind != EnemyKind::Moth && e.kind != EnemyKind::Warden && level.hazard_at(pos, body.half * 0.6).is_some() {
            e.hp = 0;
        }
        if pos.y < -100.0 {
            e.hp = 0;
        }
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;

        // ---- contact damage ----
        if player.hp > 0
            && player.hazard_t <= 0.0
            && aabb_overlap(pos, body.half * 0.85, ppos, pbody.half)
        {
            let dmg = if e.kind == EnemyKind::Warden && e.state == 5 { 2 } else { 1 };
            if player.hurt(dmg) {
                let away = (ppos.x - pos.x).signum();
                pbody.vel = Vec2::new(away * 320.0, 380.0);
                player.knock = 0.22;
                player.dash_t = 0.0;
                sfx.write(PlaySfx(Sfx::Hurt, 0.7));
                shake.add(0.45);
                fx.write(Burst::hit(ppos, 12, Color::WHITE));
            }
        }
    }
    for p in spawn_moths {
        spawn_enemy(&mut commands, &art, EnemyKind::Moth, p);
    }
}

/// Melee swipes and the down-slash pogo.
#[allow(clippy::type_complexity)]
pub fn melee(
    level: Option<Res<Level>>,
    mut players: Query<(&mut Player, &mut Body, &Transform), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Body, &Transform), Without<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut fx: MessageWriter<Burst>,
    mut shake: ResMut<Shake>,
) {
    let Some(level) = level else { return };
    let Ok((mut p, mut pb, ptf)) = players.single_mut() else { return };
    if p.melee_t <= 0.0 || p.hp <= 0 {
        return;
    }
    let pos = ptf.translation.truncate();
    let (c, half) = if p.melee_down {
        (pos + Vec2::new(0.0, -30.0), Vec2::new(18.0, 16.0))
    } else {
        (pos + Vec2::new(p.facing * 26.0, 4.0), Vec2::new(22.0, 18.0))
    };
    let mut pogo = false;
    for (ee, mut e, mut eb, etf) in &mut enemies {
        if p.melee_hits.contains(&ee) || e.hp <= 0 {
            continue;
        }
        let ep = etf.translation.truncate();
        if aabb_overlap(c, half, ep, eb.half) {
            p.melee_hits.push(ee);
            if e.frozen > 0.0 {
                // shatter frozen foes
                e.damage(2);
                fx.write(Burst::frost(ep, 14));
            } else {
                e.damage(1);
                eb.vel += Vec2::new(p.facing * if e.kind == EnemyKind::Warden { 20.0 } else { 220.0 }, 120.0);
            }
            fx.write(Burst::hit(ep, 8, Color::WHITE));
            sfx.write(PlaySfx(Sfx::HitEnemy, 0.6));
            shake.add(0.15);
            p.charges = (p.charges + 0.25).min(MAX_CHARGES);
            if p.melee_down {
                pogo = true;
            }
        }
    }
    // pogo off spikes
    if p.melee_down
        && level
            .box_touches(c, half, |t, _| matches!(t, Tile::Spikes | Tile::Thorns))
            .is_some()
    {
        pogo = true;
        fx.write(Burst::sparks(c, 6));
        sfx.write(PlaySfx(Sfx::Block, 0.4));
    }
    if pogo {
        pb.vel.y = 600.0;
        p.can_dash = true;
        p.air_hang = AIR_HANG_TIME;
        p.melee_t = 0.0;
    }
}

#[allow(clippy::type_complexity)]
pub fn projectiles(
    mut commands: Commands,
    time: Res<Time>,
    level: Option<Res<Level>>,
    mut globs: Query<(Entity, &mut Glob, &mut Transform), (Without<Player>, Without<Wave>)>,
    mut waves: Query<(Entity, &mut Wave, &mut Transform, &mut Sprite), (Without<Player>, Without<Glob>)>,
    mut players: Query<(&mut Player, &mut Body, &Transform), (Without<Glob>, Without<Wave>)>,
    mut sfx: MessageWriter<PlaySfx>,
    mut fx: MessageWriter<Burst>,
    mut shake: ResMut<Shake>,
) {
    let Some(level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    let Ok((mut p, mut pb, ptf)) = players.single_mut() else { return };
    let ppos = ptf.translation.truncate();
    let mut hurt = |p: &mut Player, pb: &mut Body, from: Vec2| {
        if p.hazard_t <= 0.0 && p.hurt(1) {
            pb.vel = Vec2::new((ppos.x - from.x).signum() * 280.0, 340.0);
            p.knock = 0.2;
            sfx.write(PlaySfx(Sfx::Hurt, 0.7));
            shake.add(0.4);
        }
    };
    for (e, mut g, mut tf) in &mut globs {
        g.vel.y -= 900.0 * dt;
        let pos = tf.translation.truncate() + g.vel * dt;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        let t = Level::tile_at(pos);
        if level.solid(t.x, t.y) || pos.y < -50.0 {
            fx.write(Burst::splat(pos));
            commands.entity(e).despawn();
            continue;
        }
        if aabb_overlap(pos, Vec2::splat(6.0), ppos, pb.half) {
            hurt(&mut p, &mut pb, pos);
            fx.write(Burst::splat(pos));
            commands.entity(e).despawn();
        }
    }
    for (e, mut w, mut tf, mut s) in &mut waves {
        w.life -= dt;
        let pos = tf.translation.truncate() + Vec2::new(w.dir * 330.0 * dt, 0.0);
        tf.translation.x = pos.x;
        let ahead = Level::tile_at(pos + Vec2::new(w.dir * 16.0, 0.0));
        s.color = s.color.with_alpha((w.life / 0.4).min(1.0));
        if w.life <= 0.0 || level.solid(ahead.x, ahead.y) {
            commands.entity(e).despawn();
            continue;
        }
        if rand::random::<f32>() < 0.5 {
            fx.write(Burst::dust(pos - Vec2::new(0.0, 10.0), 1));
        }
        if aabb_overlap(pos, Vec2::new(14.0, 14.0), ppos, pb.half) {
            hurt(&mut p, &mut pb, pos);
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn enemy_visuals(
    time: Res<Time>,
    art: Res<Art>,
    images: Res<Assets<Image>>,
    enemies: Query<(&Enemy, &Body, &Children)>,
    mut sprites: Query<(&EnemySprite, &mut Sprite, &mut Transform)>,
) {
    let now = time.elapsed_secs();
    for (e, body, children) in &enemies {
        for c in children.iter() {
            let Ok((es, mut s, mut t)) = sprites.get_mut(c) else { continue };
            let f = frames(&art, e.kind);
            let idx = match e.kind {
                EnemyKind::Crawler => ((e.anim * 8.0) as usize) % 4,
                EnemyKind::Moth => ((e.anim * 14.0) as usize) % 4,
                EnemyKind::Hopper => match e.state {
                    1 => 1,
                    2 if body.vel.y > 0.0 => 2,
                    2 => 3,
                    _ => 0,
                },
                EnemyKind::Spitter => match e.state {
                    1 => 1,
                    2 => 2,
                    _ if e.t > 1.5 => 3,
                    _ => 0,
                },
                EnemyKind::Shieldbearer => match e.state {
                    1 => 2,
                    2 => 3,
                    _ if body.vel.x.abs() > 5.0 => ((e.anim * 4.0) as usize) % 2,
                    _ => 2,
                },
                EnemyKind::Warden => match e.state {
                    1 | 4 => 1,
                    2 => 2,
                    3 => 3,
                    5 => 2,
                    _ => 0,
                },
            };
            let idx = if e.frozen > 0.0 || e.stun > 0.0 { idx.min(f.len() - 1) } else { idx };
            s.image = f[idx].clone();
            let sz = images.get(&s.image).map(|i| i.size_f32()).unwrap_or(Vec2::splat(1.0));
            s.custom_size = Some(Vec2::new(es.height * sz.x / sz.y.max(1.0), es.height));
            s.flip_x = e.dir < 0.0;
            let mut col = if e.flash > 0.0 { Color::srgb(4.0, 4.0, 4.0) } else { Color::WHITE };
            if e.frozen > 0.0 {
                col = Color::srgb(0.55, 0.85, 1.5);
            } else if e.burn > 0.0 {
                let k = 0.7 + 0.3 * (now * 20.0).sin();
                col = Color::srgb(1.6, 0.8 * k, 0.5 * k);
            }
            s.color = col;
            let bob = if e.kind == EnemyKind::Spitter && e.state == 1 { 1.0 + 0.08 * (now * 30.0).sin() } else { 1.0 };
            t.scale = Vec3::new(bob, 1.0 / bob.max(0.5), 1.0);
        }
    }
}
