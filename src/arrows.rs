//! Arrows: flight, impacts and the six arrow powers.

use bevy::prelude::*;

use crate::art::{self, Art};
use crate::audio::{PlaySfx, Sfx};
use crate::config::*;
use crate::enemies::{Enemy, EnemyKind};
use crate::fx::{Burst, Light, Shake};
use crate::game::LevelEntity;
use crate::level::{Level, Tile, freeze_water};
use crate::physics::{Body, Platform};
use crate::player::{Player, trim_queue};
use crate::props::{Lift, Prop};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ArrowKind {
    Steel,
    Lift,
    Tether,
    Ember,
    Frost,
    Blink,
}

impl ArrowKind {
    pub const ALL: [ArrowKind; 6] =
        [ArrowKind::Steel, ArrowKind::Lift, ArrowKind::Tether, ArrowKind::Ember, ArrowKind::Frost, ArrowKind::Blink];

    pub fn from_name(s: &str) -> Self {
        match s {
            "lift" => Self::Lift,
            "tether" => Self::Tether,
            "ember" => Self::Ember,
            "frost" => Self::Frost,
            "blink" => Self::Blink,
            _ => Self::Steel,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Steel => "Steel Arrow",
            Self::Lift => "Lift Arrow",
            Self::Tether => "Tether Arrow",
            Self::Ember => "Ember Arrow",
            Self::Frost => "Frost Arrow",
            Self::Blink => "Blink Arrow",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Steel => "Sticks in wood - stand on it.",
            Self::Lift => "Shoot the floor: a platform rises and carries you up. Works on walls too.",
            Self::Tether => "Hit an anchor ring or wood to zip to it. Jump to let go early.",
            Self::Ember => "Burns thorns and lights braziers. Sets foes ablaze.",
            Self::Frost => "Freezes water into ice and foes into blocks you can stand on.",
            Self::Blink => "Teleports you where it lands. Flies through grates. Swaps places with foes.",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Steel => Color::srgb(1.2, 1.2, 1.2),
            Self::Lift => Color::srgb(0.4, 2.2, 1.9),
            Self::Tether => Color::srgb(2.0, 1.5, 0.7),
            Self::Ember => Color::srgb(3.0, 1.2, 0.3),
            Self::Frost => Color::srgb(1.2, 2.0, 3.0),
            Self::Blink => Color::srgb(1.8, 0.6, 3.0),
        }
    }
}

#[derive(Component)]
pub struct Arrow {
    pub kind: ArrowKind,
    pub vel: Vec2,
    pub grav: f32,
    pub power: f32,
    pub life: f32,
    pub stuck: bool,
    pub fade: f32,
    pub hit: Vec<Entity>,
}

/// A steel arrow lodged in wood: a tiny platform.
#[derive(Component)]
pub struct Peg;

/// Arrows fly dead straight this long before gravity takes hold.
pub const ARROW_FLAT_TIME: f32 = 0.03;

/// Launch speed, gravity and power of a shot drawn for `draw` seconds.
pub fn shot_params(kind: ArrowKind, draw: f32) -> (f32, f32, f32) {
    let power = (draw / DRAW_TIME).clamp(0.0, 1.0);
    // utility arrows always fly fast and flat - only steel rewards a full draw
    let power = if kind == ArrowKind::Steel { power.max(0.15) } else { 1.0 };
    let speed = ARROW_SPEED_MIN + (ARROW_SPEED_MAX - ARROW_SPEED_MIN) * power;
    let grav = ARROW_GRAV_MAX + (ARROW_GRAV_MIN - ARROW_GRAV_MAX) * power;
    (speed, grav, power)
}

pub fn spawn_arrow(commands: &mut Commands, art: &Art, kind: ArrowKind, pos: Vec2, vel: Vec2, grav: f32, power: f32) {
    let mut e = commands.spawn((
        LevelEntity,
        Arrow { kind, vel, grav, power, life: 0.0, stuck: false, fade: 0.0, hit: vec![] },
        Sprite {
            image: art.arrows[kind.index()].clone(),
            custom_size: Some(Vec2::new(34.0, 6.0)),
            color: if kind == ArrowKind::Steel { Color::WHITE } else { Color::srgb(1.5, 1.5, 1.5) },
            ..default()
        },
        Transform::from_translation(pos.extend(7.0)).with_rotation(Quat::from_rotation_z(vel.y.atan2(vel.x))),
    ));
    if kind != ArrowKind::Steel {
        e.insert(Light { color: kind.color(), radius: 46.0, flicker: 0.1 });
    }
}

struct Hit {
    pos: Vec2,
    normal: Vec2,
    tile: Option<IVec2>,
    enemy: Option<Entity>,
    prop: Option<Entity>,
    water: Option<IVec2>,
}

pub fn arrows_update(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    level: Option<ResMut<Level>>,
    mut arrows: Query<(Entity, &mut Arrow, &mut Transform, &mut Sprite), (Without<Enemy>, Without<Player>, Without<Prop>)>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Body, &mut Transform), (Without<Player>, Without<Arrow>)>,
    mut players: Query<(&mut Player, &mut Body, &mut Transform), (Without<Enemy>, Without<Arrow>)>,
    mut props: Query<(Entity, &mut Prop, &Transform), (Without<Arrow>, Without<Enemy>, Without<Player>)>,
    mut sfx: MessageWriter<PlaySfx>,
    mut fx: MessageWriter<Burst>,
    mut shake: ResMut<Shake>,
) {
    let Some(mut level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    for (ae, mut a, mut tf, mut sprite) in &mut arrows {
        a.life += dt;
        if a.stuck {
            a.fade -= dt;
            if a.fade < 1.0 {
                sprite.color = sprite.color.with_alpha(a.fade.max(0.0));
            }
            if a.fade <= 0.0 {
                commands.entity(ae).despawn();
            }
            continue;
        }
        if a.life > 3.0 {
            commands.entity(ae).despawn();
            continue;
        }
        // trail
        if a.kind != ArrowKind::Steel && rand::random::<f32>() < 0.7 {
            fx.write(Burst::trail(tf.translation.truncate(), a.kind.color()));
        }
        // arrows fly flat for a moment before they start to drop
        if a.life > ARROW_FLAT_TIME {
            a.vel.y -= a.grav * dt;
        }
        let mut pos = tf.translation.truncate();
        let step_len = (a.vel.length() * dt).max(0.001);
        let steps = (step_len / 4.0).ceil() as i32;
        let d = a.vel * dt / steps as f32;
        let mut hit: Option<Hit> = None;
        'outer: for _ in 0..steps {
            let prev = pos;
            pos += d;
            let tip = pos + a.vel.normalize_or_zero() * 14.0;
            let t = Level::tile_at(tip);
            // water (frost freezes, others splash)
            if level.get(t.x, t.y) == Tile::Water {
                hit = Some(Hit { pos: tip, normal: Vec2::Y, tile: None, enemy: None, prop: None, water: Some(t) });
                break;
            }
            if level.arrow_solid(t.x, t.y) {
                let pt = Level::tile_at(prev + a.vel.normalize_or_zero() * 14.0);
                let normal = if pt.x != t.x && pt.y == t.y {
                    Vec2::new(-(t.x - pt.x).signum() as f32, 0.0)
                } else if pt.y != t.y && pt.x == t.x {
                    Vec2::new(0.0, -(t.y - pt.y).signum() as f32)
                } else if a.vel.x.abs() > a.vel.y.abs() {
                    Vec2::new(-a.vel.x.signum(), 0.0)
                } else {
                    Vec2::new(0.0, -a.vel.y.signum())
                };
                hit = Some(Hit { pos: tip, normal, tile: Some(t), enemy: None, prop: None, water: None });
                break;
            }
            for (ee, e, b, et) in &enemies {
                if e.hp <= 0 || a.hit.contains(&ee) {
                    continue;
                }
                let ep = et.translation.truncate();
                if (tip.x - ep.x).abs() < b.half.x + 3.0 && (tip.y - ep.y).abs() < b.half.y + 3.0 {
                    hit = Some(Hit { pos: tip, normal: -a.vel.normalize_or_zero(), tile: None, enemy: Some(ee), prop: None, water: None });
                    break 'outer;
                }
            }
            for (pe, p, pt) in &props {
                let r = match *p {
                    Prop::Target { .. } => 14.0,
                    Prop::Brazier { .. } => 14.0,
                    Prop::Anchor => 24.0,
                    _ => continue,
                };
                let c = pt.translation.truncate();
                if tip.distance(c) < r {
                    hit = Some(Hit { pos: tip, normal: -a.vel.normalize_or_zero(), tile: None, enemy: None, prop: Some(pe), water: None });
                    break 'outer;
                }
            }
        }
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.rotation = Quat::from_rotation_z(a.vel.y.atan2(a.vel.x));

        // blink arrows that fly too long blink you there anyway
        if hit.is_none() && a.kind == ArrowKind::Blink && a.life > 0.9 {
            hit = Some(Hit { pos, normal: Vec2::ZERO, tile: None, enemy: None, prop: None, water: None });
        }
        let Some(h) = hit else { continue };

        let mut stick = false;
        let mut refund = false;
        let kind = a.kind;
        let dir = a.vel.normalize_or_zero();

        // ---- water ----
        if let Some(w) = h.water {
            if kind == ArrowKind::Frost {
                let frozen = freeze_water(&mut level, w);
                for q in &frozen {
                    fx.write(Burst::frost(Level::tile_center(*q), 3));
                }
                sfx.write(PlaySfx(Sfx::Freeze, 0.6));
            } else {
                fx.write(Burst::splash(h.pos));
                sfx.write(PlaySfx(Sfx::Splash, 0.4));
                if kind == ArrowKind::Blink {
                    refund = true;
                }
            }
            if refund {
                refund_charge(&mut players);
            }
            commands.entity(ae).despawn();
            continue;
        }

        // ---- enemies ----
        if let Some(ee) = h.enemy {
            a.hit.push(ee);
            if let Ok((_, mut e, mut eb, mut etf)) = enemies.get_mut(ee) {
                let epos = etf.translation.truncate();
                // shieldbearers block from the front
                let blocked = e.kind == EnemyKind::Shieldbearer
                    && e.frozen <= 0.0
                    && dir.x * e.dir < 0.0
                    && !matches!(kind, ArrowKind::Ember | ArrowKind::Blink);
                if blocked {
                    sfx.write(PlaySfx(Sfx::Block, 0.6));
                    fx.write(Burst::sparks(h.pos, 8));
                    a.vel = Vec2::new(-a.vel.x * 0.3, 260.0);
                    a.grav = 1500.0;
                    a.kind = ArrowKind::Steel;
                    continue;
                }
                let dmg = if a.power >= 0.99 && kind == ArrowKind::Steel { 2 } else { 1 };
                match kind {
                    ArrowKind::Ember => {
                        e.burn = 2.0;
                        e.damage(dmg);
                        fx.write(Burst::embers(epos, 10));
                        sfx.write(PlaySfx(Sfx::Burn, 0.5));
                    }
                    ArrowKind::Frost if e.kind != EnemyKind::Warden => {
                        e.frozen = 6.0;
                        eb.vel = Vec2::ZERO;
                        fx.write(Burst::frost(epos, 12));
                        sfx.write(PlaySfx(Sfx::Freeze, 0.6));
                    }
                    ArrowKind::Blink => {
                        if let Ok((mut p, mut pb, mut ptf)) = players.single_mut() {
                            let ppos = ptf.translation.truncate();
                            // swap places (keep feet on the floor for both)
                            let pfeet = ppos.y - pb.half.y;
                            let efeet = epos.y - eb.half.y;
                            ptf.translation.x = epos.x;
                            ptf.translation.y = efeet + pb.half.y + 1.0;
                            etf.translation.x = ppos.x;
                            etf.translation.y = pfeet + eb.half.y + 1.0;
                            pb.vel = Vec2::ZERO;
                            p.can_dash = true;
                            p.tether = None;
                            p.hanging = false;
                            fx.write(Burst::blink(ppos));
                            fx.write(Burst::blink(epos));
                        }
                        e.stun = 0.8;
                        sfx.write(PlaySfx(Sfx::Blink, 0.6));
                    }
                    _ => {
                        e.damage(dmg);
                        eb.vel += dir * if e.kind == EnemyKind::Warden { 30.0 } else { 160.0 } + Vec2::new(0.0, 60.0);
                        fx.write(Burst::hit(h.pos, 8, Color::srgb(1.0, 0.95, 0.8)));
                        sfx.write(PlaySfx(Sfx::HitEnemy, 0.6));
                        shake.add(0.12);
                        if kind == ArrowKind::Steel
                            && let Ok((mut p, _, _)) = players.single_mut()
                        {
                            p.charges = (p.charges + 0.34).min(MAX_CHARGES);
                        }
                    }
                }
            }
            // full-draw steel arrows pierce
            if kind == ArrowKind::Steel && a.power >= 0.99 && a.hit.len() < 3 {
                continue;
            }
            commands.entity(ae).despawn();
            continue;
        }

        // ---- props ----
        if let Some(pe) = h.prop {
            if let Ok((_, mut prop, ptf)) = props.get_mut(pe) {
                let c = ptf.translation.truncate();
                match &mut *prop {
                    Prop::Target { hit, .. } => {
                        if !*hit {
                            *hit = true;
                            sfx.write(PlaySfx(Sfx::HitWood, 0.7));
                            sfx.write(PlaySfx(Sfx::Checkpoint, 0.4));
                            fx.write(Burst::sparks(c, 10));
                        }
                        stick = true;
                    }
                    Prop::Brazier { lit, .. } => {
                        if kind == ArrowKind::Ember && !*lit {
                            *lit = true;
                            sfx.write(PlaySfx(Sfx::Burn, 0.7));
                            fx.write(Burst::embers(c + Vec2::new(0.0, 10.0), 16));
                        } else if kind != ArrowKind::Ember {
                            sfx.write(PlaySfx(Sfx::HitStone, 0.5));
                        }
                    }
                    Prop::Anchor => {
                        if kind == ArrowKind::Tether {
                            start_tether(&mut players, c, &mut sfx);
                        } else {
                            sfx.write(PlaySfx(Sfx::HitStone, 0.5));
                        }
                    }
                    _ => {}
                }
            }
            if stick {
                a.stuck = true;
                a.fade = 4.0;
                tf.translation.x = h.pos.x - dir.x * 10.0;
                tf.translation.y = h.pos.y - dir.y * 10.0;
                tf.translation.z = 2.5;
            } else {
                commands.entity(ae).despawn();
            }
            continue;
        }

        // ---- tiles / free blink ----
        let tile = h.tile.map(|t| level.get(t.x, t.y)).unwrap_or(Tile::Empty);
        let surface = h.pos - dir * 14.0; // a bit back from the tip, outside the tile
        match kind {
            ArrowKind::Steel => {
                if tile == Tile::Wood {
                    sfx.write(PlaySfx(Sfx::HitWood, 0.6));
                    stick = true;
                    if h.normal.x != 0.0 {
                        // a peg you can stand on
                        let t = h.tile.unwrap();
                        let face_x = if h.normal.x > 0.0 { (t.x + 1) as f32 * TILE } else { t.x as f32 * TILE };
                        let peg_c = Vec2::new(face_x + h.normal.x * 11.0, h.pos.y.min((t.y + 1) as f32 * TILE - 3.0));
                        tf.translation.x = peg_c.x;
                        tf.translation.y = peg_c.y;
                        tf.rotation = Quat::from_rotation_z(if h.normal.x > 0.0 { std::f32::consts::PI } else { 0.0 });
                        tf.translation.z = 4.0;
                        commands.entity(ae).insert((Peg, Platform { half: Vec2::new(12.0, 3.0), delta: Vec2::ZERO }));
                        a.stuck = true;
                        a.fade = f32::INFINITY;
                        if let Ok((mut p, _, _)) = players.single_mut() {
                            p.pegs.push_back(ae);
                            let mut q = std::mem::take(&mut p.pegs);
                            trim_queue(&mut commands, &mut q, MAX_PEGS);
                            p.pegs = q;
                        }
                        fx.write(Burst::dust(peg_c, 4));
                        continue;
                    }
                } else if tile == Tile::Thorns {
                    sfx.write(PlaySfx(Sfx::HitWood, 0.4));
                    stick = true;
                } else {
                    sfx.write(PlaySfx(Sfx::HitStone, 0.5));
                    fx.write(Burst::sparks(h.pos, 5));
                    stick = rand::random::<f32>() < 0.5;
                }
            }
            ArrowKind::Lift => {
                if h.normal.y > 0.5 || h.normal.x != 0.0 {
                    let t = h.tile.unwrap();
                    let center = if h.normal.y > 0.5 {
                        let mut c = Vec2::new(h.pos.x, (t.y + 1) as f32 * TILE - 6.0);
                        // keep the platform out of neighbouring walls
                        let probe = |c: Vec2| level.box_solid(c + Vec2::new(0.0, 22.0), Vec2::new(LIFT_WIDTH / 2.0, 14.0));
                        for _ in 0..12 {
                            if !probe(c) {
                                break;
                            }
                            let left = level.box_solid(c + Vec2::new(-LIFT_WIDTH / 2.0, 22.0), Vec2::new(1.0, 14.0));
                            c.x += if left { 4.0 } else { -4.0 };
                        }
                        c
                    } else {
                        let face_x = if h.normal.x > 0.0 { (t.x + 1) as f32 * TILE } else { t.x as f32 * TILE };
                        Vec2::new(face_x + h.normal.x * (LIFT_WIDTH / 2.0 - 8.0), h.pos.y)
                    };
                    spawn_lift(&mut commands, &art, center, &mut players);
                    sfx.write(PlaySfx(Sfx::Lift, 0.7));
                    fx.write(Burst::trail_n(center, kind.color(), 14));
                } else {
                    refund = true;
                    sfx.write(PlaySfx(Sfx::Fizzle, 0.5));
                }
            }
            ArrowKind::Tether => {
                if tile == Tile::Wood {
                    start_tether(&mut players, surface, &mut sfx);
                    stick = true;
                } else {
                    refund = true;
                    sfx.write(PlaySfx(Sfx::Fizzle, 0.5));
                    fx.write(Burst::sparks(h.pos, 4));
                }
            }
            ArrowKind::Ember => {
                if tile == Tile::Thorns {
                    let t = h.tile.unwrap();
                    level.burning.entry(t).or_insert(0.05);
                    sfx.write(PlaySfx(Sfx::Burn, 0.7));
                } else if tile == Tile::Wood {
                    stick = true;
                }
                fx.write(Burst::embers(h.pos, 10));
            }
            ArrowKind::Frost => {
                fx.write(Burst::frost(h.pos, 8));
                sfx.write(PlaySfx(Sfx::Freeze, 0.3));
            }
            ArrowKind::Blink => {
                let target = if h.normal == Vec2::ZERO { h.pos } else { surface };
                if let Ok((mut p, mut pb, mut ptf)) = players.single_mut() {
                    if let Some(spot) = free_spot(&level, target, pb.half, h.normal) {
                        let from = ptf.translation.truncate();
                        ptf.translation.x = spot.x;
                        ptf.translation.y = spot.y;
                        pb.vel = Vec2::new(pb.vel.x * 0.3, 0.0);
                        p.can_dash = true;
                        p.air_hang = AIR_HANG_TIME;
                        p.tether = None;
                        p.hanging = false;
                        fx.write(Burst::blink(from));
                        fx.write(Burst::blink(spot));
                        sfx.write(PlaySfx(Sfx::Blink, 0.7));
                    } else {
                        refund = true;
                        sfx.write(PlaySfx(Sfx::Fizzle, 0.5));
                    }
                }
            }
        }
        if refund {
            refund_charge(&mut players);
        }
        if stick {
            a.stuck = true;
            a.fade = 3.0;
            tf.translation.x = h.pos.x - dir.x * 12.0;
            tf.translation.y = h.pos.y - dir.y * 12.0;
            tf.translation.z = 2.5;
            commands.entity(ae).remove::<Light>();
        } else {
            commands.entity(ae).despawn();
        }
    }
}

fn refund_charge(players: &mut Query<(&mut Player, &mut Body, &mut Transform), (Without<Enemy>, Without<Arrow>)>) {
    if let Ok((mut p, _, _)) = players.single_mut() {
        p.charges = (p.charges + 1.0).min(MAX_CHARGES);
    }
}

fn start_tether(
    players: &mut Query<(&mut Player, &mut Body, &mut Transform), (Without<Enemy>, Without<Arrow>)>,
    target: Vec2,
    sfx: &mut MessageWriter<PlaySfx>,
) {
    if let Ok((mut p, mut b, _)) = players.single_mut() {
        p.tether = Some(target - Vec2::new(0.0, 26.0));
        p.tether_t = 0.0;
        p.hanging = false;
        p.dash_t = 0.0;
        b.drop_through = 0.0;
        sfx.write(PlaySfx(Sfx::Tether, 0.7));
    }
}

fn spawn_lift(
    commands: &mut Commands,
    art: &Art,
    center: Vec2,
    players: &mut Query<(&mut Player, &mut Body, &mut Transform), (Without<Enemy>, Without<Arrow>)>,
) {
    let half = Vec2::new(LIFT_WIDTH / 2.0, 6.0);
    let e = commands
        .spawn((
            LevelEntity,
            Platform { half, delta: Vec2::ZERO },
            Lift { risen: 0.0, done: false },
            Sprite {
                image: art.platforms[art::PLAT_LIFT].clone(),
                custom_size: Some(Vec2::new(LIFT_WIDTH + 8.0, 16.0)),
                color: Color::srgb(1.3, 1.3, 1.3),
                ..default()
            },
            Transform::from_translation(center.extend(3.0)),
            Light { color: ArrowKind::Lift.color(), radius: 70.0, flicker: 0.05 },
        ))
        .id();
    if let Ok((mut p, _, _)) = players.single_mut() {
        p.lifts.push_back(e);
        let mut q = std::mem::take(&mut p.lifts);
        trim_queue(commands, &mut q, MAX_LIFTS);
        p.lifts = q;
    }
}

/// Find a non-solid spot for the player near `target`, nudging away from the surface.
fn free_spot(level: &Level, target: Vec2, half: Vec2, normal: Vec2) -> Option<Vec2> {
    let base = target + normal * Vec2::new(half.x + 2.0, half.y + 2.0);
    let candidates = [
        Vec2::ZERO,
        Vec2::new(0.0, 8.0),
        Vec2::new(0.0, -8.0),
        Vec2::new(0.0, 16.0),
        Vec2::new(0.0, -16.0),
        Vec2::new(8.0, 0.0),
        Vec2::new(-8.0, 0.0),
        Vec2::new(0.0, 24.0),
        Vec2::new(0.0, -24.0),
        Vec2::new(12.0, 12.0),
        Vec2::new(-12.0, 12.0),
    ];
    candidates.iter().map(|o| base + *o).find(|p| !level.box_solid(*p, half))
}

// ---------------- aim preview ----------------

const AIM_DOTS: usize = 30;
const AIM_SPACING: f32 = 18.0;
/// how far ahead (seconds of flight) the preview looks
const AIM_LOOKAHEAD: f32 = 1.0;

#[derive(Component)]
pub struct AimDot(usize);

#[derive(Component)]
pub struct AimMarker;

/// Where a shot would go: the flight path (same integration as `arrows_update`) and,
/// if it hits a wall, floor or water within the lookahead, the impact point.
pub fn predict_shot(level: &Level, start: Vec2, vel: Vec2, grav: f32) -> (Vec<Vec2>, Option<Vec2>) {
    let h = 1.0 / 120.0;
    let (mut pos, mut vel, mut life) = (start, vel, 0.0);
    let mut path = vec![pos];
    while life < AIM_LOOKAHEAD {
        life += h;
        if life > ARROW_FLAT_TIME {
            vel.y -= grav * h;
        }
        pos += vel * h;
        let tip = pos + vel.normalize_or_zero() * 14.0;
        let t = Level::tile_at(tip);
        if level.get(t.x, t.y) == Tile::Water || level.arrow_solid(t.x, t.y) {
            path.push(tip);
            return (path, Some(tip));
        }
        path.push(pos);
    }
    (path, None)
}

/// Dotted arc showing the arrow's projectile motion while the bow is drawn.
pub fn aim_preview(
    mut commands: Commands,
    time: Res<Time<Real>>,
    art: Res<Art>,
    level: Option<Res<Level>>,
    players: Query<(&Player, &Transform)>,
    mut dots: Query<(&AimDot, &mut Transform, &mut Sprite, &mut Visibility), (Without<Player>, Without<AimMarker>)>,
    mut marker: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<AimMarker>, Without<Player>, Without<AimDot>)>,
) {
    let Some(level) = level else { return };
    if dots.is_empty() {
        for i in 0..AIM_DOTS {
            commands.spawn((LevelEntity, AimDot(i), Sprite { image: art.dot.clone(), ..default() }, Transform::from_xyz(0.0, 0.0, 6.5), Visibility::Hidden));
        }
        commands.spawn((
            LevelEntity,
            AimMarker,
            Sprite { image: art.fx[crate::fx::FxKind::Ring as usize].clone(), ..default() },
            Transform::from_xyz(0.0, 0.0, 6.6),
            Visibility::Hidden,
        ));
        return;
    }
    let shot = players.single().ok().filter(|(p, _)| p.drawing && p.hp > 0 && p.draw > 0.06);
    let Some((p, tf)) = shot else {
        for (_, _, _, mut v) in &mut dots {
            *v = Visibility::Hidden;
        }
        if let Ok((_, _, mut v)) = marker.single_mut() {
            *v = Visibility::Hidden;
        }
        return;
    };
    let kind = ArrowKind::ALL[p.selected];
    let (speed, grav, power) = shot_params(kind, p.draw);
    let start = tf.translation.truncate() + Vec2::new(0.0, 6.0) + p.aim * 14.0;
    let (path, impact) = predict_shot(&level, start, p.aim * speed, grav);
    let t = time.elapsed_secs();
    let col = if kind == ArrowKind::Steel { Color::srgb(1.3, 1.4, 1.5) } else { kind.color() };
    // fade in with the draw, brighter once the shot is at full power
    let draw_k = (p.draw / 0.2).min(1.0);
    let base_a = draw_k * if power >= 1.0 { 0.95 } else { 0.7 };

    // walk the path, dropping a dot every AIM_SPACING px; the dots march outward
    let mut along: Vec<(Vec2, f32)> = Vec::with_capacity(AIM_DOTS);
    let total: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>().max(1.0);
    let mut next = 22.0 + (t * 70.0) % AIM_SPACING;
    let mut acc = 0.0;
    for w in path.windows(2) {
        let seg = w[0].distance(w[1]);
        while next <= acc + seg && along.len() < AIM_DOTS {
            let k = (next - acc) / seg.max(1e-3);
            along.push((w[0].lerp(w[1], k), next / total));
            next += AIM_SPACING;
        }
        acc += seg;
    }
    for (d, mut dt, mut s, mut v) in &mut dots {
        match along.get(d.0) {
            Some(&(pos, f)) => {
                *v = Visibility::Inherited;
                dt.translation.x = pos.x;
                dt.translation.y = pos.y;
                let fade = (1.0 - f).powf(0.6);
                s.custom_size = Some(Vec2::splat(7.5 - 3.5 * f));
                s.color = col.with_alpha(base_a * fade);
            }
            None => *v = Visibility::Hidden,
        }
    }
    if let Ok((mut mt, mut ms, mut mv)) = marker.single_mut() {
        match impact {
            Some(pos) => {
                *mv = Visibility::Inherited;
                mt.translation.x = pos.x;
                mt.translation.y = pos.y;
                mt.rotation = Quat::from_rotation_z(t * 1.5);
                let pulse = 0.5 + 0.5 * (t * 6.0).sin();
                ms.custom_size = Some(Vec2::splat(26.0 + 5.0 * pulse));
                ms.color = col.with_alpha(draw_k * (0.7 + 0.3 * pulse));
            }
            None => *mv = Visibility::Hidden,
        }
    }
}
