//! Kinematic AABB-vs-tile movement with one-way tiles and moving one-way platforms.

use bevy::prelude::*;

use crate::config::TILE;
use crate::level::Level;

#[derive(Component, Default, Clone)]
pub struct Body {
    pub half: Vec2,
    pub vel: Vec2,
    pub on_ground: bool,
    pub ground_platform: Option<Entity>,
    /// -1 / +1 when the last horizontal move was blocked
    pub wall: i8,
    pub ceiling: bool,
    /// > 0 while dropping through one-way floors
    pub drop_through: f32,
    /// ghosts (moths) ignore everything
    pub no_collide: bool,
}

impl Body {
    pub fn new(half: Vec2) -> Self {
        Self { half, ..default() }
    }
}

/// A one-way platform entity (pegs, lifts, movers, frozen enemies).
#[derive(Component, Default, Clone)]
pub struct Platform {
    pub half: Vec2,
    /// how far it moved this frame
    pub delta: Vec2,
}

#[derive(Clone, Copy)]
pub struct PlatRef {
    pub e: Entity,
    pub center: Vec2,
    pub half: Vec2,
    pub delta: Vec2,
}
pub fn move_body(pos: &mut Vec2, body: &mut Body, dt: f32, level: &Level, plats: &[PlatRef], self_e: Entity) {
    body.drop_through = (body.drop_through - dt).max(0.0);
    if body.no_collide {
        *pos += body.vel * dt;
        return;
    }
    // ride the platform we stood on
    if let Some(pe) = body.ground_platform
        && let Some(p) = plats.iter().find(|p| p.e == pe)
    {
        let before = *pos;
        pos.y += p.delta.y;
        if level.box_solid(*pos, body.half) {
            pos.y = before.y;
        }
        pos.x += p.delta.x;
        if level.box_solid(*pos, body.half) {
            pos.x = before.x;
        }
    }

    let half = body.half;
    body.wall = 0;
    body.ceiling = false;

    // ---- X ----
    let mut dx = body.vel.x * dt;
    while dx.abs() > 1e-4 {
        let step = dx.clamp(-6.0, 6.0);
        dx -= step;
        let cand = Vec2::new(pos.x + step, pos.y);
        if level.box_solid(cand, half) {
            if step > 0.0 {
                let tx = ((cand.x + half.x) / TILE).floor();
                pos.x = tx * TILE - half.x - 0.01;
                body.wall = 1;
            } else {
                let tx = ((cand.x - half.x) / TILE).floor();
                pos.x = (tx + 1.0) * TILE + half.x + 0.01;
                body.wall = -1;
            }
            body.vel.x = 0.0;
            break;
        }
        pos.x = cand.x;
    }

    // ---- Y ----
    let was_ground = body.on_ground;
    body.on_ground = false;
    body.ground_platform = None;
    let mut dy = body.vel.y * dt;
    if was_ground && dy > -0.5 && body.vel.y <= 0.0 {
        dy = -0.5; // keep probing the floor
    }
    // a platform rising through our feet picks us up (lift arrows fired at the floor we stand on)
    if body.vel.y <= 0.0 && body.drop_through <= 0.0 {
        let bottom = pos.y - half.y;
        for p in plats {
            if p.e == self_e || p.delta.y <= 0.0 {
                continue;
            }
            let ptop = p.center.y + p.half.y;
            let prev_top = ptop - p.delta.y;
            if (pos.x - p.center.x).abs() < half.x + p.half.x && prev_top <= bottom + 1.5 && ptop >= bottom - 0.5 {
                pos.y = ptop + half.y + 0.001;
                body.vel.y = 0.0;
                body.on_ground = true;
                body.ground_platform = Some(p.e);
                dy = 0.0;
                break;
            }
        }
    }
    while dy.abs() > 1e-4 {
        let step = dy.clamp(-6.0, 6.0);
        dy -= step;
        let prev_bottom = pos.y - half.y;
        let cand = Vec2::new(pos.x, pos.y + step);
        if level.box_solid(cand, half) {
            if step > 0.0 {
                let ty = ((cand.y + half.y) / TILE).floor();
                pos.y = ty * TILE - half.y - 0.01;
                body.ceiling = true;
            } else {
                let ty = ((cand.y - half.y) / TILE).floor();
                pos.y = (ty + 1.0) * TILE + half.y + 0.001;
                body.on_ground = true;
            }
            body.vel.y = 0.0;
            break;
        }
        if step < 0.0 && body.drop_through <= 0.0 {
            let new_bottom = cand.y - half.y;
            // one-way tiles
            let row = (prev_bottom / TILE).floor() as i32 - 1;
            let top = (row + 1) as f32 * TILE;
            if prev_bottom >= top - 0.01 && new_bottom < top {
                let x0 = ((pos.x - half.x) / TILE).floor() as i32;
                let x1 = ((pos.x + half.x - 0.01) / TILE).floor() as i32;
                if (x0..=x1).any(|x| level.one_way(x, row)) {
                    pos.y = top + half.y + 0.001;
                    body.vel.y = 0.0;
                    body.on_ground = true;
                    break;
                }
            }
            // platforms
            let mut landed = None;
            for p in plats {
                if p.e == self_e {
                    continue;
                }
                let ptop = p.center.y + p.half.y;
                let prev_top = ptop - p.delta.y.max(0.0);
                if (pos.x - p.center.x).abs() < half.x + p.half.x
                    && prev_bottom >= prev_top - 1.5
                    && new_bottom < ptop
                {
                    landed = Some((p.e, ptop));
                    break;
                }
            }
            if let Some((e, ptop)) = landed {
                pos.y = ptop + half.y + 0.001;
                body.vel.y = 0.0;
                body.on_ground = true;
                body.ground_platform = Some(e);
                break;
            }
        }
        pos.y = cand.y;
    }
}

/// Is there floor right under the box edge in direction `dir`? (used by walkers to turn at ledges)
pub fn ledge_ahead(level: &Level, pos: Vec2, half: Vec2, dir: f32) -> bool {
    let probe = Vec2::new(pos.x + dir * (half.x + 4.0), pos.y - half.y - 4.0);
    let t = Level::tile_at(probe);
    !(level.solid(t.x, t.y) || level.one_way(t.x, t.y))
}

pub fn aabb_overlap(a: Vec2, ah: Vec2, b: Vec2, bh: Vec2) -> bool {
    (a.x - b.x).abs() < ah.x + bh.x && (a.y - b.y).abs() < ah.y + bh.y
}
