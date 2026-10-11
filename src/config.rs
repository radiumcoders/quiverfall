//! Tuning constants. World units are pixels, y up. One tile = 32 px.

pub const TILE: f32 = 32.0;
/// Visible world height (the camera scales the window to this many world px).
pub const VIEW_H: f32 = 576.0;

// player movement
pub const RUN_SPEED: f32 = 270.0;
pub const RUN_ACCEL: f32 = 2600.0;
pub const AIR_ACCEL: f32 = 1900.0;
pub const FRICTION: f32 = 3000.0;
pub const JUMP_VEL: f32 = 680.0;
pub const GRAVITY_UP: f32 = 1900.0;
pub const GRAVITY_DOWN: f32 = 2500.0;
pub const JUMP_CUT: f32 = 0.45;
pub const MAX_FALL: f32 = 900.0;
pub const COYOTE: f32 = 0.1;
pub const JUMP_BUFFER: f32 = 0.12;
pub const DASH_SPEED: f32 = 760.0;
pub const DASH_TIME: f32 = 0.15;
pub const DASH_COOLDOWN: f32 = 0.35;
pub const PLAYER_HALF: (f32, f32) = (10.0, 20.0);

// bow
pub const DRAW_TIME: f32 = 0.38;
pub const ARROW_SPEED_MIN: f32 = 480.0;
pub const ARROW_SPEED_MAX: f32 = 1050.0;
pub const ARROW_GRAV_MIN: f32 = 750.0;
pub const ARROW_GRAV_MAX: f32 = 1300.0;
pub const AIR_HANG_TIME: f32 = 0.7;
pub const AIR_HANG_FALL: f32 = 70.0;
pub const MAX_PEGS: usize = 3;
pub const MAX_LIFTS: usize = 2;
pub const MAX_CHARGES: f32 = 3.0;
pub const CHARGE_REGEN: f32 = 1.0 / 2.5;

// health
pub const MAX_HP: i32 = 5;
pub const INVULN: f32 = 1.1;

// lift arrow
pub const LIFT_RISE: f32 = 7.0 * TILE;
pub const LIFT_SPEED: f32 = 170.0;
pub const LIFT_WIDTH: f32 = 72.0;

// tether
pub const TETHER_SPEED: f32 = 950.0;
