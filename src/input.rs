//! Collects keyboard, mouse and gamepad state into one `Controls` resource each frame.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::MainCamera;

#[derive(Resource, Default, Debug)]
pub struct Controls {
    pub move_x: f32,
    pub move_y: f32,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub dash_pressed: bool,
    pub fire_held: bool,
    pub fire_released: bool,
    pub melee_pressed: bool,
    pub cycle: i32,
    pub select: Option<usize>,
    pub pause_pressed: bool,
    pub confirm_pressed: bool,
    pub restart_pressed: bool,
    /// mouse position in the world, when the mouse is the active aim device
    pub mouse_world: Option<Vec2>,
    /// explicit aim direction from a stick / keys
    pub aim_dir: Option<Vec2>,
    pub using_mouse: bool,
    last_cursor: Option<Vec2>,
}

pub fn gather_input(
    mut c: ResMut<Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    pads: Query<&Gamepad>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut script: Option<ResMut<Script>>,
    time: Res<Time>,
) {
    let k = |codes: &[KeyCode]| codes.iter().any(|k| keys.pressed(*k));
    let kp = |codes: &[KeyCode]| codes.iter().any(|k| keys.just_pressed(*k));
    let kr = |codes: &[KeyCode]| codes.iter().any(|k| keys.just_released(*k));

    let mut mx = 0.0;
    if k(&[KeyCode::KeyA, KeyCode::ArrowLeft]) {
        mx -= 1.0;
    }
    if k(&[KeyCode::KeyD, KeyCode::ArrowRight]) {
        mx += 1.0;
    }
    let mut my = 0.0;
    if k(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
        my -= 1.0;
    }
    if k(&[KeyCode::KeyW, KeyCode::ArrowUp]) {
        my += 1.0;
    }

    let jump_keys = [KeyCode::Space, KeyCode::KeyZ];
    let fire_keys = [KeyCode::KeyJ, KeyCode::KeyX];
    c.jump_pressed = kp(&jump_keys);
    c.jump_held = k(&jump_keys);
    c.dash_pressed = kp(&[KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::KeyK, KeyCode::KeyC]);
    c.fire_held = k(&fire_keys) || mouse.pressed(MouseButton::Left);
    c.fire_released = kr(&fire_keys) || mouse.just_released(MouseButton::Left);
    c.melee_pressed = kp(&[KeyCode::KeyF, KeyCode::KeyL]) || mouse.just_pressed(MouseButton::Right);
    c.pause_pressed = kp(&[KeyCode::Escape, KeyCode::KeyP]);
    c.confirm_pressed = kp(&[KeyCode::Enter, KeyCode::Space, KeyCode::NumpadEnter]);
    c.restart_pressed = kp(&[KeyCode::KeyR]);
    c.cycle = 0;
    if kp(&[KeyCode::KeyE, KeyCode::Tab]) {
        c.cycle += 1;
    }
    if kp(&[KeyCode::KeyQ]) {
        c.cycle -= 1;
    }
    if scroll.delta.y > 0.0 {
        c.cycle -= 1;
    } else if scroll.delta.y < 0.0 {
        c.cycle += 1;
    }
    c.select = None;
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6]
        .iter()
        .enumerate()
    {
        if keys.just_pressed(*key) {
            c.select = Some(i);
        }
    }

    // keyboard aim: held direction keys while drawing
    let key_aim = Vec2::new(mx, my);
    c.aim_dir = None;
    if key_aim != Vec2::ZERO && k(&fire_keys) {
        c.aim_dir = Some(key_aim.normalize());
        c.using_mouse = false;
    }

    // mouse
    let cursor = window.cursor_position();
    if cursor.is_some() && cursor != c.last_cursor && c.last_cursor.is_some() {
        c.using_mouse = true;
    }
    if mouse.just_pressed(MouseButton::Left) {
        c.using_mouse = true;
    }
    if keys.just_pressed(KeyCode::KeyJ) || keys.just_pressed(KeyCode::KeyX) {
        c.using_mouse = false;
    }
    c.last_cursor = cursor;
    let (cam, cam_t) = *camera;
    c.mouse_world = cursor.and_then(|p| cam.viewport_to_world_2d(cam_t, p).ok());

    // gamepad
    for pad in &pads {
        let ls = pad.left_stick();
        let dp = pad.dpad();
        let pmx = if ls.x.abs() > 0.3 { ls.x.signum() } else { dp.x };
        let pmy = if ls.y.abs() > 0.5 { ls.y.signum() } else { dp.y };
        if pmx != 0.0 {
            mx = pmx;
        }
        if pmy != 0.0 {
            my = pmy;
        }
        c.jump_pressed |= pad.just_pressed(GamepadButton::South);
        c.jump_held |= pad.pressed(GamepadButton::South);
        c.dash_pressed |= pad.just_pressed(GamepadButton::RightTrigger) || pad.just_pressed(GamepadButton::East);
        let fire_held = pad.pressed(GamepadButton::RightTrigger2) || pad.pressed(GamepadButton::West);
        c.fire_held |= fire_held;
        c.fire_released |=
            pad.just_released(GamepadButton::RightTrigger2) || pad.just_released(GamepadButton::West);
        c.melee_pressed |= pad.just_pressed(GamepadButton::North);
        c.pause_pressed |= pad.just_pressed(GamepadButton::Start);
        c.confirm_pressed |= pad.just_pressed(GamepadButton::South) || pad.just_pressed(GamepadButton::Start);
        if pad.just_pressed(GamepadButton::LeftTrigger) || pad.just_pressed(GamepadButton::DPadUp) {
            c.cycle += 1;
        }
        if pad.just_pressed(GamepadButton::LeftTrigger2) {
            c.cycle -= 1;
        }
        let rs = pad.right_stick();
        if rs.length() > 0.35 {
            c.aim_dir = Some(rs.normalize());
            c.using_mouse = false;
        } else if fire_held && ls.length() > 0.35 {
            c.aim_dir = Some(ls.normalize());
            c.using_mouse = false;
        }
    }
    c.move_x = mx;
    c.move_y = my;
    if let Some(script) = script.as_mut() {
        script.apply(&mut c, time.elapsed_secs());
    }
}

/// QF_SCRIPT="t0-t1:TOK,TOK;..." drives the controls for automated play-tests.
/// Tokens: L R U D (move), J (jump), S (dash), F (draw; released at t1), M (melee),
/// a=dx/dy (aim direction), sel=N (pick arrow N, 1-based).
#[derive(Resource)]
pub struct Script {
    segs: Vec<(f32, f32, Vec<String>)>,
    prev_jump: bool,
    prev_fire: bool,
    prev_dash: bool,
    prev_melee: bool,
    t0: Option<f32>,
}

impl Script {
    pub fn from_env() -> Option<Self> {
        let s = std::env::var("QF_SCRIPT").ok()?;
        let mut segs = vec![];
        for seg in s.split(';').filter(|x| !x.trim().is_empty()) {
            let (range, toks) = seg.split_once(':')?;
            let (a, b) = range.split_once('-')?;
            segs.push((a.trim().parse().ok()?, b.trim().parse().ok()?, toks.split(',').map(|t| t.trim().to_string()).collect()));
        }
        Some(Self { segs, prev_jump: false, prev_fire: false, prev_dash: false, prev_melee: false, t0: None })
    }

    fn apply(&mut self, c: &mut Controls, now: f32) {
        let t = now - *self.t0.get_or_insert(now);
        let (mut mx, mut my) = (0.0, 0.0);
        let (mut jump, mut fire, mut dash, mut melee) = (false, false, false, false);
        let mut aim = None;
        for (a, b, toks) in &self.segs {
            if t < *a || t >= *b {
                continue;
            }
            for tok in toks {
                match tok.as_str() {
                    "L" => mx -= 1.0,
                    "R" => mx += 1.0,
                    "U" => my += 1.0,
                    "D" => my -= 1.0,
                    "J" => jump = true,
                    "F" => fire = true,
                    "S" => dash = true,
                    "M" => melee = true,
                    _ => {
                        if let Some(v) = tok.strip_prefix("a=")
                            && let Some((x, y)) = v.split_once('/')
                        {
                            aim = Some(Vec2::new(x.parse().unwrap_or(1.0), y.parse().unwrap_or(0.0)).normalize_or(Vec2::X));
                        }
                        if let Some(v) = tok.strip_prefix("sel=") {
                            c.select = v.parse::<usize>().ok().map(|n| n.saturating_sub(1));
                        }
                    }
                }
            }
        }
        c.move_x = mx;
        c.move_y = my;
        c.jump_pressed = jump && !self.prev_jump;
        c.jump_held = jump;
        c.fire_held = fire;
        c.fire_released = !fire && self.prev_fire;
        c.dash_pressed = dash && !self.prev_dash;
        c.melee_pressed = melee && !self.prev_melee;
        c.using_mouse = false;
        c.aim_dir = aim;
        self.prev_jump = jump;
        self.prev_fire = fire;
        self.prev_dash = dash;
        self.prev_melee = melee;
    }
}
