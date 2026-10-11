//! Wren the archer: movement, bow, melee, damage and visuals.

use bevy::prelude::*;
use std::collections::VecDeque;
use std::f32::consts::FRAC_PI_2;

use crate::arrows::{ArrowKind, spawn_arrow};
use crate::art::Art;
use crate::audio::{PlaySfx, Sfx};
use crate::config::*;
use crate::fx::{Burst, Flash, FxKind, Shake};
use crate::game::{LevelEntity, Progress};
use crate::input::Controls;
use crate::level::{CrumbleState, Level, Tile};
use crate::physics::{Body, PlatRef, Platform, move_body};

#[derive(Component)]
pub struct Player {
    pub hp: i32,
    pub charges: f32,
    pub selected: usize,
    pub facing: f32,
    pub coyote: f32,
    pub jump_buf: f32,
    pub jump_was_held: bool,
    pub dash_t: f32,
    pub dash_cd: f32,
    pub dash_dir: f32,
    pub can_dash: bool,
    pub drawing: bool,
    pub draw: f32,
    pub aim: Vec2,
    pub air_hang: f32,
    pub invuln: f32,
    pub melee_t: f32,
    pub melee_cd: f32,
    pub melee_down: bool,
    pub melee_hits: Vec<Entity>,
    pub tether: Option<Vec2>,
    pub tether_t: f32,
    /// hanging from the tether point (after the zip arrives)
    pub hanging: bool,
    pub last_safe: Vec2,
    pub hazard_t: f32,
    pub dead_t: f32,
    pub squash: f32,
    pub was_ground: bool,
    pub pegs: VecDeque<Entity>,
    pub lifts: VecDeque<Entity>,
    pub knock: f32,
}

#[derive(Component)]
pub struct PlayerSprite;

#[derive(Component)]
pub struct BowSprite;

#[derive(Component)]
pub struct NockedArrow;

pub fn spawn_player(commands: &mut Commands, art: &Art, pos: Vec2) {
    commands
        .spawn((
            LevelEntity,
            Player {
                hp: MAX_HP,
                charges: MAX_CHARGES,
                selected: 0,
                facing: 1.0,
                coyote: 0.0,
                jump_buf: 0.0,
                jump_was_held: false,
                dash_t: 0.0,
                dash_cd: 0.0,
                dash_dir: 1.0,
                can_dash: true,
                drawing: false,
                draw: 0.0,
                aim: Vec2::X,
                air_hang: AIR_HANG_TIME,
                invuln: 0.0,
                melee_t: 0.0,
                melee_cd: 0.0,
                melee_down: false,
                melee_hits: vec![],
                tether: None,
                tether_t: 0.0,
                hanging: false,
                last_safe: pos,
                hazard_t: 0.0,
                dead_t: 0.0,
                squash: 0.0,
                was_ground: false,
                pegs: VecDeque::new(),
                lifts: VecDeque::new(),
                knock: 0.0,
            },
            Body::new(Vec2::new(PLAYER_HALF.0, PLAYER_HALF.1)),
            Transform::from_translation(pos.extend(6.0)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                PlayerSprite,
                Sprite { image: art.player[0].clone(), custom_size: Some(Vec2::new(57.0, 54.0)), ..default() },
                bevy::sprite::Anchor::BOTTOM_CENTER,
                Transform::from_xyz(0.0, -PLAYER_HALF.1 - 1.0, 0.0),
            ));
            c.spawn((
                BowSprite,
                Sprite { image: art.bow.clone(), custom_size: Some(Vec2::new(10.0, 48.0)), ..default() },
                Transform::from_xyz(0.0, 0.0, 0.2),
            ));
            c.spawn((
                NockedArrow,
                Sprite { image: art.arrows[0].clone(), custom_size: Some(Vec2::new(34.0, 6.0)), ..default() },
                Transform::from_xyz(0.0, 0.0, 0.3),
                Visibility::Hidden,
            ));
        });
}

impl Player {
    /// Returns true if damage was dealt.
    pub fn hurt(&mut self, amount: i32) -> bool {
        if self.invuln > 0.0 || self.hp <= 0 {
            return false;
        }
        self.hp -= amount;
        self.invuln = INVULN;
        self.drawing = false;
        self.tether = None;
        self.hanging = false;
        true
    }
}

pub fn player_update(
    mut commands: Commands,
    time: Res<Time>,
    c: Res<Controls>,
    level: Option<ResMut<Level>>,
    progress: Res<Progress>,
    art: Res<Art>,
    mut players: Query<(Entity, &mut Player, &mut Body, &mut Transform)>,
    plats: Query<(Entity, &Transform, &Platform), Without<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut fx: MessageWriter<Burst>,
    mut flashes: MessageWriter<Flash>,
    mut shake: ResMut<Shake>,
) {
    let Some(mut level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    let plat_refs: Vec<PlatRef> = plats
        .iter()
        .map(|(e, t, p)| PlatRef { e, center: t.translation.truncate(), half: p.half, delta: p.delta })
        .collect();
    for (me, mut p, mut body, mut tf) in &mut players {
        let mut pos = tf.translation.truncate();
        p.invuln = (p.invuln - dt).max(0.0);
        p.squash *= (1.0 - dt * 10.0).max(0.0);

        if p.hp <= 0 {
            p.dead_t += dt;
            body.vel = Vec2::ZERO;
            continue;
        }
        if p.hazard_t > 0.0 {
            let before = p.hazard_t;
            p.hazard_t -= dt;
            if before > 0.3 && p.hazard_t <= 0.3 {
                pos = p.last_safe;
                tf.translation.x = pos.x;
                tf.translation.y = pos.y;
            }
            body.vel = Vec2::ZERO;
            continue;
        }

        // ---- arrow selection ----
        if let Some(i) = c.select
            && progress.unlocked[i]
            && p.selected != i
        {
            p.selected = i;
            sfx.write(PlaySfx(Sfx::Select, 0.5));
        }
        if c.cycle != 0 {
            let mut i = p.selected as i32;
            for _ in 0..6 {
                i = (i + c.cycle.signum()).rem_euclid(6);
                if progress.unlocked[i as usize] {
                    break;
                }
            }
            if i as usize != p.selected {
                p.selected = i as usize;
                sfx.write(PlaySfx(Sfx::Select, 0.5));
            }
        }

        // ---- timers ----
        if body.on_ground {
            p.coyote = COYOTE;
            p.can_dash = true;
            p.air_hang = AIR_HANG_TIME;
        } else {
            p.coyote -= dt;
        }
        p.jump_buf = if c.jump_pressed { JUMP_BUFFER } else { p.jump_buf - dt };
        p.dash_cd -= dt;
        p.melee_cd -= dt;
        p.melee_t -= dt;
        p.knock -= dt;
        p.charges = (p.charges + CHARGE_REGEN * dt).min(MAX_CHARGES);

        // ---- aim ----
        let shoulder = pos + Vec2::new(0.0, 6.0);
        let aim = if c.using_mouse {
            c.mouse_world.map(|m| (m - shoulder).normalize_or(Vec2::X * p.facing))
        } else {
            c.aim_dir
        };
        // keep the last aim while drawing (keys/sticks are often released together with fire)
        p.aim = aim.unwrap_or(if p.drawing { p.aim } else { Vec2::new(p.facing, 0.0) });

        // ---- tether zip / hang ----
        if let Some(target) = p.tether {
            p.tether_t += dt;
            let d = target - pos;
            if p.hanging {
                body.vel = Vec2::ZERO;
                pos = target;
                p.can_dash = true;
                p.air_hang = AIR_HANG_TIME;
                if c.jump_pressed {
                    p.tether = None;
                    p.hanging = false;
                    p.hanging = false;
                    body.vel = Vec2::new(c.move_x * RUN_SPEED, JUMP_VEL * 0.9);
                    sfx.write(PlaySfx(Sfx::Jump, 0.5));
                } else if c.move_y < 0.0 || p.tether_t > 2.5 {
                    p.tether = None;
                    p.hanging = false;
                    p.hanging = false;
                }
            } else if d.length() < 14.0 {
                p.hanging = true;
                p.tether_t = 0.0;
                body.vel = Vec2::ZERO;
            } else if p.tether_t > 0.75 || (p.tether_t > 0.05 && (body.wall != 0 || body.ceiling)) {
                // couldn't reach it: let go
                p.tether = None;
                p.hanging = false;
                body.vel = d.normalize_or_zero() * 300.0;
            } else if c.jump_pressed {
                p.tether = None;
                p.hanging = false;
                body.vel = d.normalize_or_zero() * 420.0 + Vec2::new(0.0, JUMP_VEL * 0.8);
                p.can_dash = true;
                sfx.write(PlaySfx(Sfx::Jump, 0.5));
            } else {
                body.vel = d.normalize() * TETHER_SPEED.min(d.length() / dt);
            }
        } else if p.dash_t > 0.0 {
            // ---- dash ----
            p.dash_t -= dt;
            body.vel = Vec2::new(p.dash_dir * DASH_SPEED, 0.0);
            if p.dash_t <= 0.0 {
                body.vel.x *= 0.4;
            }
            if rand::random::<f32>() < 0.6 {
                fx.write(Burst::dust(pos + Vec2::new(-p.dash_dir * 10.0, -6.0), 1));
            }
        } else {
            if c.dash_pressed && p.can_dash && p.dash_cd <= 0.0 {
                p.dash_t = DASH_TIME;
                p.dash_cd = DASH_COOLDOWN;
                p.dash_dir = if c.move_x != 0.0 { c.move_x.signum() } else { p.facing };
                p.facing = p.dash_dir;
                if !body.on_ground {
                    p.can_dash = false;
                }
                sfx.write(PlaySfx(Sfx::Dash, 0.6));
                fx.write(Burst::dust(pos, 6));
            }
            // horizontal
            let slow = if p.drawing && body.on_ground { 0.4 } else if p.drawing { 0.7 } else { 1.0 };
            let target = c.move_x * RUN_SPEED * slow;
            let accel = if c.move_x == 0.0 { FRICTION } else if body.on_ground { RUN_ACCEL } else { AIR_ACCEL };
            if p.knock <= 0.0 {
                let dv = target - body.vel.x;
                body.vel.x += dv.clamp(-accel * dt, accel * dt);
            }
            if c.move_x != 0.0 && !p.drawing {
                p.facing = c.move_x.signum();
            }
            // drop through one-way floors
            let on_oneway = body.ground_platform.is_some() || {
                let t = Level::tile_at(pos - Vec2::new(0.0, body.half.y + 2.0));
                level.one_way(t.x, t.y)
            };
            if c.move_y < 0.0 && c.jump_pressed && body.on_ground && on_oneway {
                body.drop_through = 0.25;
                p.jump_buf = 0.0;
            } else if p.jump_buf > 0.0 && p.coyote > 0.0 {
                body.vel.y = JUMP_VEL;
                p.coyote = 0.0;
                p.jump_buf = 0.0;
                p.squash = -0.25;
                sfx.write(PlaySfx(Sfx::Jump, 0.45));
                fx.write(Burst::dust(pos - Vec2::new(0.0, body.half.y), 4));
            }
            // variable jump height
            if body.vel.y > 0.0 && p.jump_was_held && !c.jump_held {
                body.vel.y *= JUMP_CUT;
            }
            let g = if body.vel.y > 0.0 { GRAVITY_UP } else { GRAVITY_DOWN };
            body.vel.y -= g * dt;
            // archer hang: drawing in the air slows the fall
            if p.drawing && !body.on_ground && body.vel.y < 0.0 && p.air_hang > 0.0 {
                p.air_hang -= dt;
                body.vel.y = body.vel.y.max(-AIR_HANG_FALL);
            }
            body.vel.y = body.vel.y.max(-MAX_FALL);
        }
        p.jump_was_held = c.jump_held;

        // ---- bow ----
        if c.fire_held && p.melee_t <= 0.0 && (p.tether.is_none() || p.hanging) {
            if !p.drawing {
                p.drawing = true;
                p.draw = 0.0;
                sfx.write(PlaySfx(Sfx::Draw, 0.35));
            }
            let before = p.draw;
            p.draw += dt;
            // glint on the arrowhead when a Steel shot reaches full draw
            if p.selected == 0 && before < DRAW_TIME && p.draw >= DRAW_TIME {
                let tip = shoulder + p.aim * 22.0;
                flashes.write(Flash::new(tip, FxKind::Spark, (8.0, 40.0), 0.3, Color::srgb(2.4, 2.6, 2.8)).follow(me));
            }
            if p.aim.x.abs() > 0.15 {
                p.facing = p.aim.x.signum();
            }
        }
        if p.drawing && (!c.fire_held || c.fire_released) {
            p.drawing = false;
            let kind = ArrowKind::ALL[p.selected];
            let power = (p.draw / DRAW_TIME).clamp(0.0, 1.0);
            // utility arrows always fly fast and flat - only steel rewards a full draw
            let power = if kind == ArrowKind::Steel { power.max(0.15) } else { 1.0 };
            if kind != ArrowKind::Steel && p.charges < 1.0 {
                sfx.write(PlaySfx(Sfx::Fizzle, 0.5));
            } else {
                if kind != ArrowKind::Steel {
                    p.charges -= 1.0;
                }
                let speed = ARROW_SPEED_MIN + (ARROW_SPEED_MAX - ARROW_SPEED_MIN) * power;
                let grav = ARROW_GRAV_MAX + (ARROW_GRAV_MIN - ARROW_GRAV_MAX) * power;
                let start = shoulder + p.aim * 14.0;
                spawn_arrow(&mut commands, &art, kind, start, p.aim * speed, grav, power);
                if power >= 1.0 {
                    let c = if kind == ArrowKind::Steel { Color::srgb(2.2, 2.3, 2.4) } else { kind.color() };
                    flashes.write(Flash::new(start, FxKind::Spark, (40.0, 10.0), 0.14, c).angle(p.aim.to_angle()));
                }
                sfx.write(PlaySfx(Sfx::Release, 0.5));
                // small recoil in the air
                if !body.on_ground {
                    body.vel -= p.aim * 60.0;
                }
            }
        }

        // ---- melee ----
        if c.melee_pressed && p.melee_cd <= 0.0 && !p.drawing {
            p.melee_t = 0.16;
            p.melee_cd = 0.32;
            p.melee_down = !body.on_ground && c.move_y < 0.0;
            p.melee_hits.clear();
            sfx.write(PlaySfx(Sfx::Swipe, 0.5));
            let (off, ang) = if p.melee_down { (Vec2::new(0.0, -24.0), -FRAC_PI_2) } else { (Vec2::new(p.facing * 26.0, 4.0), 0.0) };
            let flip = p.facing < 0.0 && !p.melee_down;
            flashes.write(
                Flash::new(pos + off, FxKind::Slash, (48.0, 64.0), 0.18, Color::srgb(1.9, 2.0, 2.1)).angle(ang).flip(flip).follow(me),
            );
        }

        // ---- move ----
        if !p.hanging {
            move_body(&mut pos, &mut body, dt, &level, &plat_refs, me);
        }

        if body.on_ground && !p.was_ground {
            p.squash = 0.3;
            p.can_dash = true;
            sfx.write(PlaySfx(Sfx::Land, 0.35));
            fx.write(Burst::dust(pos - Vec2::new(0.0, body.half.y), 5));
        }
        p.was_ground = body.on_ground;

        // crumbling floors start shaking when stood on
        if body.on_ground && body.ground_platform.is_none() {
            let feet = pos - Vec2::new(0.0, body.half.y + 2.0);
            for dx in [-body.half.x + 1.0, body.half.x - 1.0] {
                let t = Level::tile_at(feet + Vec2::new(dx, 0.0));
                if level.get(t.x, t.y) == Tile::Crumble && !level.crumble.contains_key(&t) {
                    level.crumble.insert(t, CrumbleState::Shaking(0.45));
                }
            }
        }

        // remember safe ground for hazard respawns
        if body.on_ground
            && body.ground_platform.is_none()
            && level.hazard_at(pos, body.half + Vec2::new(TILE * 1.2, 4.0)).is_none()
        {
            let feet = pos - Vec2::new(0.0, body.half.y + 2.0);
            let l = Level::tile_at(feet - Vec2::new(body.half.x, 0.0));
            let r = Level::tile_at(feet + Vec2::new(body.half.x, 0.0));
            let firm = |t: IVec2| matches!(level.get(t.x, t.y), Tile::Stone | Tile::Wood | Tile::OneWay);
            if firm(l) && firm(r) {
                p.last_safe = pos;
            }
        }

        // hazards: spikes, water, thorns, pits
        let pit = pos.y < -TILE * 2.0;
        if pit || level.hazard_at(pos, body.half - Vec2::new(2.0, 2.0)).is_some() {
            p.invuln = 0.0;
            if p.hurt(1) {
                p.invuln = 0.6;
                sfx.write(PlaySfx(Sfx::Hurt, 0.7));
                shake.add(0.5);
                fx.write(Burst::hit(pos, 10, Color::srgb(1.0, 0.95, 0.9)));
            }
            if p.hp > 0 {
                p.hazard_t = 0.6;
                p.tether = None;
                p.hanging = false;
                p.dash_t = 0.0;
            }
            body.vel = Vec2::ZERO;
        }

        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
    }
}

pub fn player_visuals(
    time: Res<Time>,
    art: Res<Art>,
    players: Query<(&Player, &Body, &Children, &Transform)>,
    mut sprites: Query<&mut Sprite, (With<PlayerSprite>, Without<BowSprite>, Without<NockedArrow>)>,
    mut bows: Query<(&mut Transform, &mut Sprite), (With<BowSprite>, Without<PlayerSprite>, Without<Player>, Without<NockedArrow>)>,
    mut nocked: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<NockedArrow>, Without<BowSprite>, Without<PlayerSprite>, Without<Player>)>,
    mut ps_t: Query<&mut Transform, (With<PlayerSprite>, Without<Player>, Without<BowSprite>, Without<NockedArrow>)>,
    mut gizmos: Gizmos,
    mut anim: Local<f32>,
) {
    let dt = time.delta_secs();
    for (p, body, children, ptf) in &players {
        let pos = ptf.translation.truncate();
        let speed = body.vel.x.abs();
        *anim += speed * dt / 30.0;
        let frame = if p.hp <= 0 {
            6
        } else if p.dash_t > 0.0 || p.tether.is_some() {
            7
        } else if !body.on_ground {
            if body.vel.y > 0.0 { 5 } else { 6 }
        } else if speed > 25.0 {
            1 + (*anim as usize) % 4
        } else {
            0
        };
        let flicker = p.invuln > 0.0 && ((time.elapsed_secs() * 20.0) as i32) % 2 == 0;
        let fade = if p.hazard_t > 0.0 { ((p.hazard_t - 0.3).abs() / 0.3).clamp(0.0, 1.0) } else { 1.0 };
        let dead_fade = if p.hp <= 0 { (1.0 - p.dead_t).max(0.0) } else { 1.0 };
        for child in children.iter() {
            if let Ok(mut s) = sprites.get_mut(child) {
                s.image = art.player[frame].clone();
                s.flip_x = p.facing < 0.0;
                let a = if flicker { 0.35 } else { 1.0 } * fade * dead_fade;
                s.color = Color::srgba(1.0, 1.0, 1.0, a);
            }
            if let Ok(mut t) = ps_t.get_mut(child) {
                let breathe = if frame == 0 { (time.elapsed_secs() * 2.5).sin() * 0.02 } else { 0.0 };
                let sq = p.squash;
                t.scale = Vec3::new(1.0 + sq * 0.5 - breathe, 1.0 - sq * 0.6 + breathe, 1.0);
                let tilt = if (1..=4).contains(&frame) { -p.facing * 0.06 } else { 0.0 };
                t.rotation = Quat::from_rotation_z(tilt);
            }
            if let Ok((mut t, mut s)) = bows.get_mut(child) {
                s.color = Color::srgba(1.0, 1.0, 1.0, fade * dead_fade);
                if p.drawing {
                    let ang = p.aim.y.atan2(p.aim.x);
                    t.translation = (Vec2::new(0.0, 6.0) + p.aim * 11.0).extend(0.4);
                    t.rotation = Quat::from_rotation_z(ang);
                    s.flip_y = false;
                } else {
                    // slung across the back
                    t.translation = Vec3::new(-p.facing * 5.0, 2.0, -0.2);
                    t.rotation = Quat::from_rotation_z(p.facing * 0.5);
                    s.flip_x = p.facing < 0.0;
                }
                if p.drawing {
                    s.flip_x = false;
                }
            }
            if let Ok((mut t, mut s, mut vis)) = nocked.get_mut(child) {
                if p.drawing && p.hp > 0 {
                    *vis = Visibility::Inherited;
                    let pull = (p.draw / DRAW_TIME).clamp(0.0, 1.0);
                    let ang = p.aim.y.atan2(p.aim.x);
                    let c = Vec2::new(0.0, 6.0) + p.aim * (11.0 - 10.0 * pull + 13.0);
                    t.translation = c.extend(0.5);
                    t.rotation = Quat::from_rotation_z(ang);
                    s.image = art.arrows[p.selected].clone();
                    let glow = 1.0 + pull * if p.selected == 0 { 0.3 } else { 1.5 };
                    s.color = Color::srgb(glow, glow, glow);
                } else {
                    *vis = Visibility::Hidden;
                }
            }
        }
        // bowstring
        if p.drawing && p.hp > 0 {
            let pull = (p.draw / DRAW_TIME).clamp(0.0, 1.0);
            let ang = p.aim.y.atan2(p.aim.x);
            let rot = Vec2::from_angle(ang);
            let center = pos + Vec2::new(0.0, 6.0) + p.aim * 11.0;
            let tip_a = center + rot.rotate(Vec2::new(-3.0, 22.0));
            let tip_b = center + rot.rotate(Vec2::new(-3.0, -22.0));
            let nock = center + rot.rotate(Vec2::new(-3.0 - 12.0 * pull, 0.0));
            let col = if pull >= 1.0 { Color::srgb(2.0, 2.2, 2.4) } else { Color::srgba(0.85, 0.85, 0.8, 0.9) };
            gizmos.line_2d(tip_a, nock, col);
            gizmos.line_2d(nock, tip_b, col);
        }
        // tether rope
        if let Some(t) = p.tether {
            gizmos.line_2d(pos + Vec2::new(0.0, 6.0), t, Color::srgb(1.6, 1.3, 0.8));
        }
    }
}

/// Peg/lift bookkeeping: keep only the newest N.
pub fn trim_queue(commands: &mut Commands, q: &mut VecDeque<Entity>, max: usize) {
    while q.len() > max {
        if let Some(e) = q.pop_front() {
            commands.entity(e).despawn();
        }
    }
}
