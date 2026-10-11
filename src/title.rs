//! The title screen: a living key-art scene (parallax painting, light shafts, drifting embers,
//! a glowing bow) behind a menu with chapter select and a controls page.

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;

use crate::MainCamera;
use crate::art::{Art, bg_paths};
use crate::audio::{PlaySfx, Sfx};
use crate::config::VIEW_H;
use crate::game::{AppState, LoadLevel, Progress};
use crate::hud::{ScreenFade, unlock_for_level};
use crate::input::{Controls, Script};
use crate::level::{LEVEL_SOURCES, parse_level};

const INK: Color = Color::srgb(0.93, 0.91, 0.85);
const DIM: Color = Color::srgba(0.82, 0.84, 0.82, 0.55);
const GOLD: Color = Color::srgb(1.0, 0.9, 0.7);
const MAIN_ITEMS: [&str; 4] = ["Begin the Descent", "Chapters", "Controls", "Quit"];
const NUMERALS: [&str; 5] = ["I", "II", "III", "IV", "V"];
/// the painted archer sits left of centre: keep it in frame on narrow windows
const FOCUS_X: f32 = 0.34;
const LEAVE_TIME: f32 = 0.55;

const CONTROL_ROWS: [(&str, &str, &str); 9] = [
    ("Move", "A / D  ·  arrows", "left stick"),
    ("Jump (hold = higher)", "Space / Z", "A"),
    ("Dash, also mid-air", "Shift / K / C", "RB / B"),
    ("Draw & fire", "hold Mouse / J, release", "hold RT / X"),
    ("Aim", "mouse, or keys while drawing", "right stick"),
    ("Swipe  (down + swipe = pogo)", "Right Mouse / F", "Y"),
    ("Switch arrow", "1-6  ·  Q E  ·  wheel", "LB / LT"),
    ("Drop through ledges", "S + Space", "down + A"),
    ("Pause  ·  restart", "Esc  ·  R", "Start"),
];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Main,
    Chapters,
    Controls,
}

#[derive(Resource, Default)]
pub struct TitleMenu {
    page: Page,
    sel: usize,
    chapter: usize,
    t: f32,
    /// level chosen; the screen fades out before it loads
    leaving: Option<(usize, f32)>,
    look: Vec2,
    nav_hold: f32,
}

#[derive(Component)]
pub struct TitleBg;
#[derive(Component)]
pub struct TitleFg;
#[derive(Component)]
pub struct TitleRays(f32);
#[derive(Component)]
pub struct BowGlow;
#[derive(Component)]
pub struct TitleFog;
#[derive(Component)]
pub struct TitleMote {
    vel: Vec2,
    life: f32,
    max: f32,
    near: bool,
}
#[derive(Component)]
pub struct LogoBlock;
#[derive(Component)]
pub struct PageRoot(Page);
#[derive(Component)]
pub struct MenuItem(usize);
#[derive(Component)]
pub struct MenuPointer(usize);
#[derive(Component)]
pub struct ChapterCard(usize);
#[derive(Component)]
pub struct FooterHint;

fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (Text::new(s), TextFont { font: font.clone().into(), font_size: size.into(), ..default() }, TextColor(color))
}

fn shadow() -> TextShadow {
    TextShadow { offset: Vec2::new(0.0, 3.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) }
}

pub fn spawn_title(
    mut commands: Commands,
    art: Res<Art>,
    progress: Res<Progress>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
    mut fade: ResMut<ScreenFade>,
) {
    commands.insert_resource(TitleMenu::default());
    fade.alpha = 1.0;
    fade.target = 0.0;
    if let Ok(mut t) = cam.single_mut() {
        t.translation.x = 0.0;
        t.translation.y = 0.0;
    }
    let scene = DespawnOnExit(AppState::Title);

    // ---------- world: the key-art scene ----------
    commands
        .spawn((scene.clone(), TitleBg, Sprite { image: art.title.clone(), ..default() }, Transform::from_xyz(0.0, 0.0, -100.0)))
        .with_children(|bg| {
            // the bow in the painting gets a breathing glow (local coords are fixed up in `animate`)
            bg.spawn((
                BowGlow,
                Sprite { image: art.glow.clone(), color: Color::srgba(0.7, 1.1, 1.3, 0.3), ..default() },
                Transform::from_xyz(0.0, 0.0, 0.5),
            ));
        });
    for (i, (alpha, phase)) in [(0.20, 0.0), (0.12, 2.1)].into_iter().enumerate() {
        commands.spawn((
            scene.clone(),
            TitleRays(phase),
            Sprite { image: art.fx_rays.clone(), color: Color::srgba(0.75, 0.95, 1.0, alpha), flip_x: true, ..default() },
            Transform::from_xyz(0.0, 0.0, -90.0 + i as f32),
        ));
    }
    commands.spawn((
        scene.clone(),
        TitleFog,
        Sprite { image: art.fog.clone(), color: Color::srgba(0.35, 0.6, 0.62, 0.22), ..default() },
        Transform::from_xyz(0.0, 0.0, -30.0),
    ));
    commands.spawn((
        scene.clone(),
        TitleFg,
        Sprite { image: art.title_fg.clone(), color: Color::srgb(0.16, 0.2, 0.24), ..default() },
        Transform::from_xyz(0.0, 0.0, -10.0),
    ));

    // ---------- UI ----------
    let levels: Vec<_> = LEVEL_SOURCES.iter().map(|s| parse_level(s)).collect();
    commands
        .spawn((
            scene.clone(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        ))
        .with_children(|root| {
            // logo
            root.spawn((
                LogoBlock,
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(7),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn((
                    Text::new("QUIVERFALL"),
                    TextFont { font: art.font_title.clone().into(), font_size: FontSize::VMin(10.5), ..default() },
                    TextColor(INK),
                    TextShadow { offset: Vec2::new(0.0, 5.0), color: Color::srgba(0.0, 0.02, 0.04, 0.9) },
                ));
                c.spawn((
                    ImageNode::new(art.ui_flourish.clone()).with_color(Color::srgba(0.9, 0.92, 0.9, 0.85)),
                    Node { width: Val::VMin(46.0), aspect_ratio: Some(720.0 / 118.0), margin: UiRect::top(px(-6)), ..default() },
                ));
                c.spawn((
                    text(&art.font_body, "an archer's descent", 24.0, DIM),
                    shadow(),
                    Node { margin: UiRect::top(px(6)), ..default() },
                ));
            });

            // main menu
            root.spawn((
                PageRoot(Page::Main),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: percent(11),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4),
                    ..default()
                },
            ))
            .with_children(|c| {
                for (i, label) in MAIN_ITEMS.iter().enumerate() {
                    c.spawn((
                        MenuItem(i),
                        RelativeCursorPosition::default(),
                        Node { align_items: AlignItems::Center, column_gap: px(14), padding: UiRect::axes(px(10), px(4)), ..default() },
                    ))
                    .with_children(|r| {
                        r.spawn((
                            MenuPointer(i),
                            ImageNode::new(art.ui_pointer.clone()),
                            Node { width: px(30), height: px(28), ..default() },
                        ));
                        r.spawn((text(&art.font_title, *label, 28.0, DIM), shadow()));
                        r.spawn((
                            MenuPointer(i),
                            ImageNode::new(art.ui_pointer.clone()).with_flip_x(),
                            Node { width: px(30), height: px(28), ..default() },
                        ));
                    });
                }
            });

            // chapters
            root.spawn((
                PageRoot(Page::Chapters),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: percent(9),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(18),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn((text(&art.font_title, "Chapters", 30.0, INK), shadow()));
                c.spawn(Node {
                    column_gap: px(14),
                    row_gap: px(14),
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::FlexStart,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(px(16)),
                    ..default()
                })
                .with_children(|row| {
                    for (i, def) in levels.iter().enumerate() {
                        row.spawn((
                            ChapterCard(i),
                            RelativeCursorPosition::default(),
                            Node {
                                width: Val::Vw(17.0),
                                max_width: px(250),
                                min_width: px(150),
                                min_height: px(210),
                                flex_direction: FlexDirection::Column,
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::all(px(4)),
                                padding: UiRect::all(px(6)),
                                row_gap: px(4),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.78)),
                            BorderColor::all(Color::srgba(0.8, 0.85, 0.8, 0.25)),
                            UiTransform::default(),
                        ))
                        .with_children(|card| {
                            card.spawn((
                                ImageNode::new(art_path_handle(&art, i)),
                                Node {
                                    width: percent(100),
                                    height: Val::Vw(8.6),
                                    max_height: px(134),
                                    min_height: px(77),
                                    border_radius: BorderRadius::all(px(3)),
                                    ..default()
                                },
                            ));
                            card.spawn((text(&art.font_title, NUMERALS[i.min(4)], 13.0, GOLD), Node { margin: UiRect::top(px(4)), ..default() }));
                            card.spawn(text(&art.font_title, def.name.clone(), 16.0, INK));
                            card.spawn(text(&art.font_body, def.subtitle.clone(), 15.0, DIM));
                        });
                    }
                });
            });

            // controls
            root.spawn((
                PageRoot(Page::Controls),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: percent(8),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(px(28), px(18)),
                    row_gap: px(5),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    max_width: percent(94),
                    ..default()
                })
                .insert((BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.82)), BorderColor::all(Color::srgba(0.8, 0.85, 0.8, 0.2))))
                .with_children(|panel| {
                    let row = |panel: &mut ChildSpawnerCommands, a: &str, k: &str, g: &str, head: bool| {
                        let (font, size, ca, ck) =
                            if head { (&art.font_title, 15.0, GOLD, GOLD) } else { (&art.font_body, 18.0, INK, DIM) };
                        panel.spawn(Node { column_gap: px(18), ..default() }).with_children(|r| {
                            r.spawn((Node { width: px(250), ..default() },)).with_child(text(font, a, size, ca));
                            r.spawn((Node { width: px(250), ..default() },)).with_child(text(font, k, size, ck));
                            r.spawn((Node { width: px(130), ..default() },)).with_child(text(font, g, size, ck));
                        });
                    };
                    row(panel, "Action", "Keyboard + mouse", "Gamepad", true);
                    for (a, k, g) in CONTROL_ROWS {
                        row(panel, a, k, g, false);
                    }
                });
            });

            // footer
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: px(18),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2),
                ..default()
            })
            .with_children(|c| {
                c.spawn((FooterHint, text(&art.font_body, "", 16.0, DIM), shadow()));
                if progress.deaths > 0 {
                    c.spawn(text(&art.font_body, format!("deaths so far: {}", progress.deaths), 14.0, DIM));
                }
            });
        });
}

fn art_path_handle(art: &Art, i: usize) -> Handle<Image> {
    art.chapter_thumbs.get(i).cloned().unwrap_or_default()
}

/// Thumbnails for the chapter cards: each level's far background.
pub fn chapter_thumbs(server: &AssetServer) -> Vec<Handle<Image>> {
    LEVEL_SOURCES.iter().map(|s| server.load(bg_paths(parse_level(s).biome).0)).collect()
}

fn rnd() -> f32 {
    rand::random::<f32>()
}

/// Cover-fit the painting and frame, drift them with the mouse, breathe the lights, drift motes.
pub fn animate_scene(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    images: Res<Assets<Image>>,
    mut menu: ResMut<TitleMenu>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut bg: Query<(&mut Sprite, &mut Transform), (With<TitleBg>, Without<TitleFg>, Without<TitleRays>, Without<BowGlow>, Without<TitleFog>, Without<TitleMote>)>,
    mut fg: Query<(&mut Sprite, &mut Transform), (With<TitleFg>, Without<TitleBg>, Without<TitleRays>, Without<BowGlow>, Without<TitleFog>, Without<TitleMote>)>,
    mut rays: Query<(&TitleRays, &mut Sprite, &mut Transform), (Without<TitleBg>, Without<TitleFg>, Without<BowGlow>, Without<TitleFog>, Without<TitleMote>)>,
    mut glow: Query<(&mut Sprite, &mut Transform), (With<BowGlow>, Without<TitleBg>, Without<TitleFg>, Without<TitleRays>, Without<TitleFog>, Without<TitleMote>)>,
    mut fog: Query<(&mut Sprite, &mut Transform), (With<TitleFog>, Without<TitleBg>, Without<TitleFg>, Without<TitleRays>, Without<BowGlow>, Without<TitleMote>)>,
    mut motes: Query<(Entity, &mut TitleMote, &mut Transform, &mut Sprite), (Without<TitleBg>, Without<TitleFg>, Without<TitleRays>, Without<BowGlow>, Without<TitleFog>)>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    menu.t += dt;
    let aspect = window.width() / window.height().max(1.0);
    let view = Vec2::new(VIEW_H * aspect, VIEW_H);

    // where the mouse is, -1..1, eased
    let want = window
        .cursor_position()
        .map(|p| Vec2::new(p.x / window.width() * 2.0 - 1.0, 1.0 - p.y / window.height() * 2.0))
        .unwrap_or(Vec2::ZERO)
        .clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
    let k = 1.0 - (-dt * 2.5).exp();
    let look0 = menu.look;
    menu.look = look0 + (want - look0) * k;
    let look = menu.look + Vec2::new((t * 0.13).sin(), (t * 0.09).cos()) * 0.25;

    // painting: cover the view with a little margin, slow push-in
    let img = crate::art::img_size(&images, &art.title);
    let img = if img.x > 0.0 { img } else { Vec2::new(1600.0, 900.0) };
    let zoom = 1.06 + 0.02 * (1.0 - (-menu.t * 0.15).exp()) + 0.008 * (t * 0.2).sin();
    let scale = (view.x / img.x).max(view.y / img.y) * zoom;
    let size = img * scale;
    let half_vis = view.x / size.x / 2.0;
    let cx = FOCUS_X.clamp(half_vis, 1.0 - half_vis);
    let margin = (size - view) / 2.0;
    let base = Vec2::new((0.5 - cx) * size.x, 0.0);
    let bg_pos = base - look * margin.min(Vec2::splat(40.0)) * 0.5;
    if let Ok((mut s, mut tr)) = bg.single_mut() {
        s.custom_size = Some(size);
        tr.translation.x = bg_pos.x;
        tr.translation.y = bg_pos.y;
    }
    // image-fraction -> world position on the painting
    let at = |fx: f32, fy: f32| bg_pos + Vec2::new((fx - 0.5) * size.x, (0.5 - fy) * size.y);
    if let Ok((mut s, mut tr)) = glow.single_mut() {
        let p = at(0.255, 0.45) - bg_pos;
        tr.translation.x = p.x;
        tr.translation.y = p.y;
        let pulse = 0.5 + 0.5 * (t * 1.7).sin() * (t * 0.6).cos().abs();
        s.custom_size = Some(Vec2::splat(size.y * (0.14 + 0.03 * pulse)));
        s.color = Color::srgba(0.75, 1.15, 1.35, 0.16 + 0.14 * pulse);
    }
    for (r, mut s, mut tr) in &mut rays {
        let p = at(0.47, 0.34);
        tr.translation.x = p.x + (t * 0.21 + r.0).sin() * 10.0;
        tr.translation.y = p.y;
        let rays_img = crate::art::img_size(&images, &art.fx_rays);
        let w = size.x * 0.62;
        s.custom_size = Some(Vec2::new(w, w * rays_img.y / rays_img.x.max(1.0)));
        let a = if r.0 == 0.0 { 0.15 } else { 0.08 };
        s.color = s.color.with_alpha(a * (0.7 + 0.3 * (t * 0.5 + r.0).sin()));
        tr.rotation = Quat::from_rotation_z(0.03 * (t * 0.17 + r.0).sin());
    }
    if let Ok((mut s, mut tr)) = fog.single_mut() {
        s.custom_size = Some(Vec2::new(view.x + 200.0, view.y * 0.45));
        tr.translation.x = (t * 0.1).sin() * 30.0;
        tr.translation.y = -view.y * 0.5 + view.y * 0.2;
    }
    // foreground frame: oversize, moves twice as much as the painting
    let fg_img = crate::art::img_size(&images, &art.title_fg);
    if let Ok((mut s, mut tr)) = fg.single_mut()
        && fg_img.x > 0.0
    {
        let fs = (view.x / fg_img.x).max(view.y / fg_img.y) * 1.16;
        let fsize = fg_img * fs;
        s.custom_size = Some(fsize);
        let fm = (fsize - view) / 2.0;
        tr.translation.x = -look.x * fm.x * 0.7;
        tr.translation.y = -look.y * fm.y * 0.7 - fm.y * 0.35;
    }

    // embers and dust drifting up out of the abyss
    let count = motes.iter().count();
    if count < 90 && rnd() < 0.7 {
        let near = rnd() < 0.2;
        let x = (rnd() - 0.5) * (view.x + 100.0);
        let y = -view.y * 0.5 + rnd() * view.y * 0.9;
        let warm = rnd() < 0.18;
        let col = if warm { Color::srgba(2.2, 1.2, 0.6, 0.0) } else { Color::srgba(0.9, 1.4, 1.5, 0.0) };
        commands.spawn((
            DespawnOnExit(AppState::Title),
            TitleMote { vel: Vec2::new((rnd() - 0.5) * 14.0, 6.0 + rnd() * 16.0), life: 0.0, max: 5.0 + rnd() * 6.0, near },
            Sprite { image: art.dot.clone(), custom_size: Some(Vec2::splat(if near { 5.0 } else { 2.0 + rnd() * 1.5 })), color: col, ..default() },
            Transform::from_xyz(x, y, if near { -5.0 } else { -60.0 }),
        ));
    }
    for (e, mut m, mut tr, mut s) in &mut motes {
        m.life += dt;
        if m.life > m.max {
            commands.entity(e).despawn();
            continue;
        }
        let fade = (m.life / m.max * std::f32::consts::PI).sin();
        let par = if m.near { 1.6 } else { 0.6 };
        tr.translation.x += (m.vel.x + (t * 0.7 + tr.translation.y * 0.02).sin() * 8.0) * dt - (want.x - menu.look.x) * par;
        tr.translation.y += m.vel.y * dt;
        s.color = s.color.with_alpha(if m.near { 0.5 } else { 0.7 } * fade);
    }
}

/// Menu navigation (keys, gamepad, mouse) and the look of the selected item/card.
pub fn menu_input(
    time: Res<Time>,
    c: Res<Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    script: Option<Res<Script>>,
    mut menu: ResMut<TitleMenu>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<AppState>>,
    mut loads: MessageWriter<LoadLevel>,
    mut sfx: MessageWriter<PlaySfx>,
    mut exit: MessageWriter<AppExit>,
    mut fade: ResMut<ScreenFade>,
    items: Query<(&MenuItem, &RelativeCursorPosition)>,
    cards: Query<(&ChapterCard, &RelativeCursorPosition)>,
    mut prev_move: Local<Vec2>,
) {
    let dt = time.delta_secs();
    // fading out toward the chosen level
    if let Some((lvl, t)) = menu.leaving {
        let t = t + dt;
        menu.leaving = Some((lvl, t));
        fade.target = 1.0;
        if t >= LEAVE_TIME || script.is_some() {
            *progress = Progress::default();
            unlock_for_level(&mut progress, lvl);
            loads.write(LoadLevel::Start(lvl));
            next.set(AppState::Playing);
        }
        return;
    }

    // directional edges with key repeat
    let mv = Vec2::new(c.move_x, c.move_y);
    let mut step = IVec2::ZERO;
    if mv != *prev_move {
        menu.nav_hold = 0.0;
        step = IVec2::new(mv.x as i32, -(mv.y as i32));
    } else if mv != Vec2::ZERO {
        menu.nav_hold += dt;
        if menu.nav_hold > 0.4 {
            menu.nav_hold -= 0.12;
            step = IVec2::new(mv.x as i32, -(mv.y as i32));
        }
    }
    *prev_move = mv;
    let back = keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::Backspace)
        || pads.iter().any(|p| p.just_pressed(GamepadButton::East));
    let click = mouse.just_pressed(MouseButton::Left);
    let mut confirm = c.confirm_pressed;

    let mut start = None;
    for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5].iter().enumerate() {
        if keys.just_pressed(*k) && i < LEVEL_SOURCES.len() {
            start = Some(i);
        }
    }

    let before = (menu.page, menu.sel, menu.chapter);
    match menu.page {
        Page::Main => {
            let n = MAIN_ITEMS.len() as i32;
            if step.y != 0 {
                menu.sel = (menu.sel as i32 + step.y).rem_euclid(n) as usize;
            }
            if c.using_mouse {
                for (it, rc) in &items {
                    if rc.cursor_over() {
                        menu.sel = it.0;
                        confirm |= click;
                    }
                }
            }
            if confirm {
                match menu.sel {
                    0 => start = Some(0),
                    1 => menu.page = Page::Chapters,
                    2 => menu.page = Page::Controls,
                    _ => {
                        exit.write(AppExit::Success);
                    }
                }
            }
        }
        Page::Chapters => {
            let n = LEVEL_SOURCES.len() as i32;
            let d = step.x + step.y;
            if d != 0 {
                menu.chapter = (menu.chapter as i32 + d.signum()).rem_euclid(n) as usize;
            }
            if c.using_mouse {
                for (card, rc) in &cards {
                    if rc.cursor_over() {
                        menu.chapter = card.0;
                        confirm |= click;
                    }
                }
            }
            if confirm {
                start = Some(menu.chapter);
            } else if back {
                menu.page = Page::Main;
            }
        }
        Page::Controls => {
            if confirm || back || click {
                menu.page = Page::Main;
            }
        }
    }
    if let Some(i) = start {
        menu.leaving = Some((i, 0.0));
        sfx.write(PlaySfx(Sfx::Pickup, 0.6));
    } else if before != (menu.page, menu.sel, menu.chapter) {
        sfx.write(PlaySfx(Sfx::Select, 0.4));
    }
}

pub fn menu_visuals(
    time: Res<Time>,
    menu: Res<TitleMenu>,
    mut pages: Query<(&PageRoot, &mut Node), (Without<ChapterCard>, Without<LogoBlock>)>,
    items: Query<(&MenuItem, &Children)>,
    mut texts: Query<(&mut TextColor, &mut TextShadow)>,
    mut pointers: Query<(&MenuPointer, &mut ImageNode, &mut UiTransform), Without<ChapterCard>>,
    mut cards: Query<(&ChapterCard, &mut BorderColor, &mut BackgroundColor, &mut UiTransform), Without<MenuPointer>>,
    mut logo: Query<&mut UiTransform, (With<LogoBlock>, Without<MenuPointer>, Without<ChapterCard>)>,
    mut footer: Query<&mut Text, With<FooterHint>>,
) {
    let t = time.elapsed_secs();
    for (p, mut node) in &mut pages {
        node.display = if p.0 == menu.page { Display::Flex } else { Display::None };
    }
    // the logo eases down into place, and lifts away from the bigger pages
    if let Ok(mut lt) = logo.single_mut() {
        let intro = (1.0 - (-menu.t * 2.2).exp()).min(1.0);
        let lift = if menu.page == Page::Main { 0.0 } else { -40.0 };
        lt.translation = Val2::px(0.0, -18.0 * (1.0 - intro) + lift);
    }
    let pulse = 0.5 + 0.5 * (t * 3.0).sin();
    for (it, children) in &items {
        let on = it.0 == menu.sel;
        for ch in children.iter() {
            if let Ok((mut col, mut sh)) = texts.get_mut(ch) {
                col.0 = if on { INK } else { DIM };
                sh.color = if on { Color::srgba(0.4, 0.75, 0.8, 0.35 + 0.2 * pulse) } else { Color::srgba(0.0, 0.0, 0.0, 0.85) };
                sh.offset = if on { Vec2::ZERO } else { Vec2::new(0.0, 3.0) };
            }
        }
    }
    for (pt, mut img, mut tr) in &mut pointers {
        let on = pt.0 == menu.sel;
        img.color = Color::srgba(0.95, 0.95, 0.9, if on { 0.95 } else { 0.0 });
        let dir = if img.flip_x { 1.0 } else { -1.0 };
        tr.translation = Val2::px(dir * 3.0 * pulse, 0.0);
    }
    for (card, mut border, mut bgc, mut tr) in &mut cards {
        let on = card.0 == menu.chapter;
        *border = BorderColor::all(if on { Color::srgba(1.0, 0.92, 0.75, 0.85) } else { Color::srgba(0.8, 0.85, 0.8, 0.2) });
        bgc.0 = if on { Color::srgba(0.06, 0.08, 0.1, 0.9) } else { Color::srgba(0.02, 0.03, 0.05, 0.75) };
        tr.scale = Vec2::splat(if on { 1.05 } else { 1.0 });
        tr.translation = Val2::px(0.0, if on { -6.0 } else { 0.0 });
    }
    if let Ok(mut f) = footer.single_mut() {
        f.0 = match menu.page {
            Page::Main => "↑ ↓  choose      Enter / (A)  confirm      1-5  jump straight to a chapter",
            Page::Chapters => "← →  choose      Enter / (A)  descend      Esc / (B)  back",
            Page::Controls => "Esc / (B)  back",
        }
        .into();
    }
}
