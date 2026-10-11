//! Camera follow, screen shake, parallax backgrounds, vignette.

use bevy::prelude::*;

use crate::config::*;
use crate::fx::Shake;
use crate::game::Parallax;
use crate::level::Level;
use crate::physics::Body;
use crate::player::Player;
use crate::{MainCamera, Vignette};

#[derive(Resource, Default)]
pub struct CamState {
    pub pos: Vec2,
    pub initialized: bool,
}

#[allow(clippy::type_complexity)]
pub fn camera_follow(
    time: Res<Time>,
    level: Option<Res<Level>>,
    players: Query<(&Player, &Body, &Transform), Without<MainCamera>>,
    mut cam: Query<(&mut Transform, &Projection), With<MainCamera>>,
    mut state: ResMut<CamState>,
    mut shake: ResMut<Shake>,
    windows: Query<&Window>,
) {
    let Some(level) = level else { return };
    let Ok((mut ct, _)) = cam.single_mut() else { return };
    let Ok((p, body, pt)) = players.single() else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    let aspect = windows.iter().next().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    let hh = VIEW_H / 2.0;
    let hw = hh * aspect;
    let ppos = pt.translation.truncate();
    let look_down = if body.vel.y < -500.0 { -90.0 } else { 0.0 };
    let target = ppos + Vec2::new(p.facing * 60.0, 50.0 + look_down);
    if !state.initialized || state.pos.distance(target) > 900.0 {
        state.pos = target;
        state.initialized = true;
    }
    let k = 1.0 - (-dt * 5.0).exp();
    let ky = 1.0 - (-dt * if body.vel.y < -500.0 { 9.0 } else { 4.0 }).exp();
    state.pos.x += (target.x - state.pos.x) * k;
    state.pos.y += (target.y - state.pos.y) * ky;
    let size = level.size_px();
    let clamp = |v: f32, lo: f32, hi: f32| if lo > hi { (lo + hi) / 2.0 } else { v.clamp(lo, hi) };
    state.pos.x = clamp(state.pos.x, hw, size.x - hw);
    state.pos.y = clamp(state.pos.y, hh, size.y - hh + TILE);
    shake.trauma = (shake.trauma - dt * 1.8).max(0.0);
    let s = shake.trauma * shake.trauma * 12.0;
    let t = time.elapsed_secs();
    let off = Vec2::new((t * 61.0).sin() + (t * 37.0).cos(), (t * 53.0).cos() + (t * 29.0).sin()) * 0.5 * s;
    ct.translation.x = state.pos.x + off.x;
    ct.translation.y = state.pos.y + off.y;
}

pub fn parallax(
    level: Option<Res<Level>>,
    cam: Query<&Transform, With<MainCamera>>,
    mut q: Query<(&Parallax, &mut Transform, &Sprite), Without<MainCamera>>,
    windows: Query<&Window>,
) {
    let Some(level) = level else { return };
    let Ok(ct) = cam.single() else { return };
    let c = ct.translation.truncate();
    let size = level.size_px();
    let aspect = windows.iter().next().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    let view = Vec2::new(VIEW_H * aspect, VIEW_H);
    for (p, mut t, s) in &mut q {
        if p.factor.x > 0.9 {
            // far painting: pinned to the screen, drifts within its margin
            let sz = s.custom_size.unwrap_or(view);
            let scale = (view.x / sz.x).max(1.0) * 1.02;
            t.scale = Vec3::new(scale, scale, 1.0);
            let margin = (sz * scale - view).max(Vec2::ZERO) / 2.0;
            let rel = ((c - size / 2.0) / (size / 2.0).max(Vec2::splat(1.0))).clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
            t.translation.x = c.x - rel.x * margin.x;
            t.translation.y = c.y - rel.y * margin.y;
        } else {
            t.translation.x = c.x * p.factor.x + p.base.x;
            t.translation.y = c.y * p.factor.y + p.base.y + VIEW_H * 0.15;
        }
    }
}

pub fn fit_vignette(windows: Query<&Window>, mut q: Query<&mut Sprite, With<Vignette>>) {
    let aspect = windows.iter().next().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    for mut s in &mut q {
        s.custom_size = Some(Vec2::new(VIEW_H * aspect + 40.0, VIEW_H + 40.0));
    }
}
