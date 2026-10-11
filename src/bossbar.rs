//! The Warden's health bar: ornate frame, chip-damage trail, hit flash, wake-up sweep.

use bevy::prelude::*;

use crate::art::Art;
use crate::enemies::BossBar;

/// The empty slot inside `boss_frame.png`, as fractions of the image (left, top, right, bottom).
/// (Slightly oversized so the fill tucks under the frame's inner lip.)
const SLOT: (f32, f32, f32, f32) = (0.142, 0.42, 0.143, 0.315);
const FRAME_ASPECT: f32 = 1013.0 / 200.0;

const FILL: Color = Color::srgb(0.78, 0.16, 0.12);
const FILL_LOW: Color = Color::srgb(0.95, 0.38, 0.12);
const LAG: Color = Color::srgb(1.0, 0.86, 0.66);
const NAME: Color = Color::srgb(0.98, 0.86, 0.66);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum BossPart {
    Root,
    Box,
    Name,
    Sub,
    Track,
    Lag,
    Fill,
    Shine,
    Tick,
    Frame,
    Emblem,
}

/// Animation state for the bar (survives level reloads; driven by `BossBar`).
#[derive(Default)]
pub struct BarAnim {
    was_active: bool,
    alpha: f32,
    hold: f32,
    shown: f32,
    lag: f32,
    lag_delay: f32,
    last: f32,
    flash: f32,
    shake: f32,
    rising: bool,
}

pub fn spawn_boss_bar(root: &mut ChildSpawnerCommands, art: &Art) {
    let txt = |font: &Handle<Font>, s: &str, size: f32, c: Color| {
        (Text::new(s), TextFont { font: font.clone().into(), font_size: size.into(), ..default() }, TextColor(c))
    };
    root.spawn((
        BossPart::Root,
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            width: percent(100),
            // keep clear of the masks/quiver column on the left (symmetric so it stays centred)
            padding: UiRect::horizontal(px(285)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            display: Display::None,
            ..default()
        },
    ))
    .with_children(|c| {
        // title row: the Warden's mask, then name and epithet
        c.spawn(Node { align_items: AlignItems::Center, column_gap: px(10), ..default() }).with_children(|row| {
            row.spawn((
                BossPart::Emblem,
                ImageNode::new(art.boss_emblem.clone()),
                Node { width: px(46), height: px(52), ..default() },
            ));
            row.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                col.spawn((
                    BossPart::Name,
                    txt(&art.font_title, "THE WARDEN", 26.0, NAME),
                    TextShadow { offset: Vec2::new(0.0, 3.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) },
                ));
                col.spawn((
                    BossPart::Sub,
                    txt(&art.font_body, "keeper of the last gate", 16.0, Color::srgba(0.85, 0.82, 0.78, 0.7)),
                    TextShadow { offset: Vec2::new(0.0, 2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) },
                    Node { margin: UiRect::top(px(-4)), ..default() },
                ));
            });
        });
        c.spawn((
            BossPart::Box,
            Node { width: percent(100), max_width: px(820), aspect_ratio: Some(FRAME_ASPECT), margin: UiRect::top(px(2)), ..default() },
        ))
        .with_children(|b| {
            let (l, t, r, btm) = SLOT;
            b.spawn((
                BossPart::Track,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(l * 100.0),
                    right: percent(r * 100.0),
                    top: percent(t * 100.0),
                    bottom: percent(btm * 100.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.85)),
            ))
            .with_children(|tr| {
                let bar = |part| {
                    (
                        part,
                        Node { position_type: PositionType::Absolute, left: px(0), top: px(0), bottom: px(0), width: percent(100), ..default() },
                        BackgroundColor(Color::NONE),
                    )
                };
                tr.spawn(bar(BossPart::Lag));
                tr.spawn(bar(BossPart::Fill));
                // a soft highlight along the top third of the fill
                tr.spawn((
                    BossPart::Shine,
                    Node { position_type: PositionType::Absolute, left: px(0), top: percent(8), height: percent(28), width: percent(100), ..default() },
                    BackgroundColor(Color::NONE),
                ));
                // half-health marker: the Warden starts summoning below it
                tr.spawn((
                    BossPart::Tick,
                    Node { position_type: PositionType::Absolute, left: percent(50), top: px(0), bottom: px(0), width: px(2), ..default() },
                    BackgroundColor(Color::NONE),
                ));
            });
            b.spawn((
                BossPart::Frame,
                ImageNode::new(art.boss_frame.clone()),
                Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            ));
        });
    });
}

pub fn update_boss_bar(
    time: Res<Time>,
    bar: Res<BossBar>,
    mut s: Local<BarAnim>,
    mut parts: Query<(
        &BossPart,
        Option<&mut Node>,
        Option<&mut BackgroundColor>,
        Option<&mut ImageNode>,
        Option<&mut TextColor>,
        Option<&mut UiTransform>,
    )>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    let frac = bar.frac.clamp(0.0, 1.0);

    // wake-up: the bar fills from empty
    if bar.active && !s.was_active {
        s.shown = 0.0;
        s.lag = 0.0;
        s.last = frac;
        s.rising = true;
    }
    s.was_active = bar.active;
    if bar.active {
        s.hold = 1.6;
    } else {
        s.hold -= dt;
    }
    // took damage
    if frac < s.last - 1e-4 {
        s.flash = 1.0;
        s.shake = 1.0;
        s.lag_delay = 0.5;
        s.rising = false;
    }
    s.last = frac;
    if s.rising {
        s.shown = (s.shown + dt * 0.8).min(frac);
        s.rising = s.shown < frac;
    } else {
        s.shown = frac;
    }
    s.lag_delay -= dt;
    if s.lag < s.shown {
        s.lag = s.shown;
    } else if s.lag_delay <= 0.0 {
        s.lag = (s.lag - dt * 0.45).max(s.shown);
    }
    s.flash = (s.flash - dt * 4.0).max(0.0);
    s.shake = (s.shake - dt * 3.5).max(0.0);
    let target = if s.hold > 0.0 { 1.0 } else { 0.0 };
    s.alpha += (target - s.alpha).clamp(-dt * 2.0, dt * 3.0);
    let a = s.alpha;

    let low = s.shown < 0.5;
    let pulse = if low { 0.5 + 0.5 * (t * 5.0).sin() } else { 0.0 };
    let fill = FILL.mix(&FILL_LOW, 0.35 * pulse).mix(&Color::WHITE, s.flash * 0.75);

    for (part, node, bg, img, txt, xf) in &mut parts {
        match part {
            BossPart::Root => {
                if let Some(mut n) = node {
                    n.display = if a > 0.005 { Display::Flex } else { Display::None };
                }
                if let Some(mut x) = xf {
                    x.translation = Val2::px(0.0, -16.0 * (1.0 - a));
                }
            }
            BossPart::Box => {
                if let Some(mut x) = xf {
                    let k = s.shake * s.shake * 5.0;
                    x.translation = Val2::px((t * 70.0).sin() * k, (t * 55.0).cos() * k * 0.5);
                }
            }
            BossPart::Name | BossPart::Sub => {
                if let Some(mut x) = xf {
                    x.scale = Vec2::splat(1.0 + 0.04 * s.flash);
                }
                if let Some(mut c) = txt {
                    let base = if *part == BossPart::Name { NAME } else { Color::srgba(0.85, 0.82, 0.78, 0.7) };
                    c.0 = base.with_alpha(base.alpha() * a);
                }
            }
            BossPart::Track => {
                if let Some(mut b) = bg {
                    b.0 = Color::srgba(0.02, 0.02, 0.03, 0.85 * a);
                }
            }
            BossPart::Lag => {
                if let Some(mut n) = node {
                    n.width = percent(s.lag * 100.0);
                }
                if let Some(mut b) = bg {
                    b.0 = LAG.with_alpha(0.8 * a);
                }
            }
            BossPart::Fill => {
                if let Some(mut n) = node {
                    n.width = percent(s.shown * 100.0);
                }
                if let Some(mut b) = bg {
                    b.0 = fill.with_alpha(a);
                }
            }
            BossPart::Shine => {
                if let Some(mut n) = node {
                    n.width = percent(s.shown * 100.0);
                }
                if let Some(mut b) = bg {
                    b.0 = Color::srgba(1.0, 0.75, 0.6, (0.22 + 0.4 * s.flash) * a);
                }
            }
            BossPart::Tick => {
                if let Some(mut b) = bg {
                    b.0 = Color::srgba(0.0, 0.0, 0.0, 0.55 * a);
                }
            }
            BossPart::Frame | BossPart::Emblem => {
                if let Some(mut i) = img {
                    let glow = if *part == BossPart::Emblem { 1.0 + 0.6 * s.flash + 0.25 * pulse } else { 1.0 };
                    i.color = Color::srgba(glow, glow, glow, a);
                }
            }
        }
    }
}
