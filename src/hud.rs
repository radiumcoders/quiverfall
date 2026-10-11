//! HUD (masks, charges, arrow bar, boss bar, banners, hints) and the menu screens.

use bevy::prelude::*;

use crate::arrows::ArrowKind;
use crate::art::{self, Art};
use crate::enemies::BossBar;
use crate::game::{AppState, LoadLevel, Paused, Progress};
use crate::input::Controls;
use crate::level::{LEVEL_SOURCES, parse_level};

/// Full-screen black overlay used for fades between screens and on level (re)loads.
#[derive(Resource)]
pub struct ScreenFade {
    pub alpha: f32,
    pub target: f32,
}

impl Default for ScreenFade {
    fn default() -> Self {
        Self { alpha: 1.0, target: 0.0 }
    }
}

#[derive(Component)]
pub struct FadeOverlay;

pub fn spawn_fade(mut commands: Commands) {
    commands.spawn((
        FadeOverlay,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::BLACK),
        GlobalZIndex(100),
        Pickable::IGNORE,
    ));
}

pub fn update_fade(
    time: Res<Time<Real>>,
    mut fade: ResMut<ScreenFade>,
    mut loads: MessageReader<LoadLevel>,
    mut q: Query<&mut BackgroundColor, With<FadeOverlay>>,
) {
    for l in loads.read() {
        fade.alpha = fade.alpha.max(match l {
            LoadLevel::Start(_) => 1.0,
            LoadLevel::Respawn => 0.8,
        });
        fade.target = 0.0;
    }
    let dt = time.delta_secs().min(1.0 / 20.0);
    let speed = if fade.target > fade.alpha { 2.4 } else { 1.5 };
    fade.alpha += (fade.target - fade.alpha).clamp(-speed * dt, speed * dt);
    if let Ok(mut bg) = q.single_mut() {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, fade.alpha.clamp(0.0, 1.0).powf(1.3));
    }
}
use crate::player::Player;

#[derive(Resource, Default)]
pub struct Banner {
    pub title: String,
    pub sub: String,
    pub t: f32,
    pub dur: f32,
}

impl Banner {
    pub fn new(title: &str, sub: &str) -> Self {
        Self { title: title.into(), sub: sub.into(), t: 0.0, dur: 4.0 }
    }
    pub fn alpha(&self) -> f32 {
        if self.t < 0.6 { self.t / 0.6 } else { ((self.dur - self.t) / 1.0).clamp(0.0, 1.0) }
    }
    pub fn unlock(&mut self, k: ArrowKind) {
        *self = Self {
            title: format!("{} acquired", k.name()),
            sub: k.blurb().into(),
            t: 0.0,
            dur: 6.0,
        };
    }
}

#[derive(Resource, Default)]
pub struct HintText(pub Option<String>);

#[derive(Component)]
pub struct MaskIcon(usize);
#[derive(Component)]
pub struct OrbIcon(usize);
#[derive(Component)]
pub struct ArrowSlot(usize);
#[derive(Component)]
pub struct BannerTitle;
#[derive(Component)]
pub struct BannerSub;
#[derive(Component)]
pub struct BannerFlourish;
#[derive(Component)]
pub struct HintNode;
#[derive(Component)]
pub struct BossBarRoot;
#[derive(Component)]
pub struct BossBarFill;
#[derive(Component)]
pub struct PauseOverlay;
#[derive(Component)]
pub struct ArrowName;

const INK: Color = Color::srgb(0.92, 0.9, 0.84);
const DIM: Color = Color::srgba(0.85, 0.85, 0.8, 0.55);

fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (Text::new(s), TextFont { font: font.clone().into(), font_size: size.into(), ..default() }, TextColor(color))
}

pub fn spawn_hud(mut commands: Commands, art: Res<Art>) {
    commands
        .spawn((
            DespawnOnExit(AppState::Playing),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        ))
        .with_children(|root| {
            // top-left: masks + charges
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(28),
                top: px(22),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            })
            .with_children(|c| {
                c.spawn(Node { column_gap: px(2), ..default() }).with_children(|r| {
                    for i in 0..crate::config::MAX_HP as usize {
                        r.spawn((MaskIcon(i), ImageNode::new(art.icons[art::ICON_MASK].clone()), Node { width: px(44), height: px(44), ..default() }));
                    }
                });
                c.spawn(Node { column_gap: px(4), padding: UiRect::left(px(6)), ..default() }).with_children(|r| {
                    for i in 0..crate::config::MAX_CHARGES as usize {
                        r.spawn((OrbIcon(i), ImageNode::new(art.icons[art::ICON_ORB].clone()), Node { width: px(28), height: px(28), ..default() }));
                    }
                });
                // quiver: one icon per unlocked arrow, the selected one larger and ringed
                c.spawn(Node { column_gap: px(6), align_items: AlignItems::Center, padding: UiRect::new(px(4), px(0), px(6), px(0)), ..default() })
                    .with_children(|r| {
                        for (i, _) in ArrowKind::ALL.iter().enumerate() {
                            r.spawn((
                                ArrowSlot(i),
                                Node {
                                    width: px(34),
                                    height: px(34),
                                    border: UiRect::all(px(2)),
                                    border_radius: BorderRadius::all(px(30)),
                                    ..default()
                                },
                                BorderColor::all(Color::NONE),
                                ImageNode::new(art.icons[i].clone()),
                            ))
                            .with_children(|s| {
                                s.spawn((
                                    Node { position_type: PositionType::Absolute, bottom: px(-15), left: px(12), ..default() },
                                    text(&art.font_title, format!("{}", i + 1), 11.0, DIM),
                                ));
                            });
                        }
                        r.spawn((ArrowName, Node { margin: UiRect::left(px(8)), ..default() }, text(&art.font_title, "", 18.0, INK)));
                    });
            });
            // boss bar
            root.spawn((
                BossBarRoot,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(26),
                    left: percent(30),
                    width: percent(40),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(6),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(text(&art.font_title, "THE WARDEN", 20.0, Color::srgb(0.95, 0.75, 0.5)));
                c.spawn((
                    Node { width: percent(100), height: px(10), border: UiRect::all(px(1)), ..default() },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    BorderColor::all(Color::srgba(0.95, 0.8, 0.6, 0.6)),
                ))
                .with_children(|b| {
                    b.spawn((BossBarFill, Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(Color::srgb(0.85, 0.35, 0.2))));
                });
            });
            // banner
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: percent(34),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(10),
                ..default()
            })
            .with_children(|c| {
                c.spawn((BannerTitle, text(&art.font_title, "", 54.0, INK), TextLayout::justify(Justify::Center)));
                c.spawn((
                    BannerFlourish,
                    ImageNode::new(art.ui_flourish.clone()).with_color(Color::NONE),
                    Node { width: px(380), aspect_ratio: Some(720.0 / 118.0), margin: UiRect::vertical(px(-10)), ..default() },
                ));
                c.spawn((BannerSub, text(&art.font_body, "", 20.0, DIM), TextLayout::justify(Justify::Center)));
            });
            // hint
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: px(56),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|c| {
                c.spawn((HintNode, text(&art.font_body, "", 20.0, Color::srgb(0.85, 0.95, 0.92)), TextLayout::justify(Justify::Center)));
            });
            // pause overlay
            root.spawn((
                PauseOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(14),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.02, 0.7)),
            ))
            .with_children(|c| {
                c.spawn(text(&art.font_title, "PAUSED", 56.0, INK));
                c.spawn(text(&art.font_body, "Esc  resume        R  restart level        Q  quit to title", 18.0, DIM));
                c.spawn(text(&art.font_body, CONTROLS, 15.0, DIM));
            });
        });
}

pub const CONTROLS: &str = "A/D move   Space jump   Shift dash   hold Mouse/J draw & release to fire   Right-click/F swipe (down+swipe in air = pogo)\n1-6 / Q E / wheel switch arrows   S+Space drop through ledges   Gamepad: A jump, RB dash, RT fire, Y swipe, LB switch";

pub fn update_hud(
    time: Res<Time>,
    art: Res<Art>,
    players: Query<&Player>,
    progress: Res<Progress>,
    mut banner: ResMut<Banner>,
    hint: Res<HintText>,
    boss: Res<BossBar>,
    paused: Res<Paused>,
    mut masks: Query<(&MaskIcon, &mut ImageNode), (Without<OrbIcon>, Without<ArrowSlot>)>,
    mut orbs: Query<(&OrbIcon, &mut ImageNode), (Without<MaskIcon>, Without<ArrowSlot>)>,
    mut slots: Query<(&ArrowSlot, &mut Node, &mut ImageNode, &mut BorderColor), (Without<MaskIcon>, Without<OrbIcon>)>,
    mut texts: ParamSet<(
        Query<(&mut Text, &mut TextColor), With<BannerTitle>>,
        Query<(&mut Text, &mut TextColor), With<BannerSub>>,
        Query<&mut Text, With<HintNode>>,
        Query<(&mut Text, &mut TextColor), With<ArrowName>>,
    )>,
    mut boss_root: Query<&mut Node, (With<BossBarRoot>, Without<ArrowSlot>, Without<BossBarFill>, Without<PauseOverlay>)>,
    mut boss_fill: Query<&mut Node, (With<BossBarFill>, Without<ArrowSlot>, Without<BossBarRoot>, Without<PauseOverlay>)>,
    mut pause: Query<&mut Node, (With<PauseOverlay>, Without<ArrowSlot>, Without<BossBarRoot>, Without<BossBarFill>)>,
    mut sel: Local<(Option<usize>, f32)>,
) {
    let Ok(p) = players.single() else { return };
    for (m, mut img) in &mut masks {
        let full = (m.0 as i32) < p.hp;
        img.image = art.icons[if full { art::ICON_MASK } else { art::ICON_MASK_EMPTY }].clone();
        img.color = if full { Color::WHITE } else { Color::srgba(1.0, 1.0, 1.0, 0.6) };
    }
    for (o, mut img) in &mut orbs {
        let f = (p.charges - o.0 as f32).clamp(0.0, 1.0);
        img.image = art.icons[if f >= 1.0 { art::ICON_ORB } else { art::ICON_ORB_EMPTY }].clone();
        img.color = if f >= 1.0 { Color::srgb(1.3, 1.3, 1.3) } else { Color::srgba(1.0, 1.0, 1.0, 0.4 + 0.5 * f) };
    }
    for (s, mut node, mut img, mut border) in &mut slots {
        let unlocked = progress.unlocked[s.0];
        node.display = if unlocked { Display::Flex } else { Display::None };
        let sel = p.selected == s.0;
        let sz = if sel { 44.0 } else { 32.0 };
        node.width = px(sz);
        node.height = px(sz);
        img.color = if sel { Color::WHITE } else { Color::srgba(0.8, 0.8, 0.8, 0.6) };
        *border = BorderColor::all(if sel { Color::srgba(1.0, 0.95, 0.8, 0.9) } else { Color::NONE });
    }
    // banner fade
    banner.t += time.delta_secs();
    let a = banner.alpha();
    if let Ok((mut t, mut c)) = texts.p0().single_mut() {
        t.0 = banner.title.clone();
        c.0 = INK.with_alpha(a);
    }
    if let Ok((mut t, mut c)) = texts.p1().single_mut() {
        t.0 = banner.sub.clone();
        c.0 = DIM.with_alpha(0.75 * a);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        t.0 = hint.0.clone().unwrap_or_default();
    }
    // the arrow's name shows briefly after switching
    if sel.0 != Some(p.selected) {
        *sel = (Some(p.selected), 0.0);
    }
    sel.1 += time.delta_secs();
    if let Ok((mut t, mut col)) = texts.p3().single_mut() {
        let k = ArrowKind::ALL[p.selected];
        let n = progress.unlocked.iter().filter(|u| **u).count();
        t.0 = if n > 1 { k.name().to_string() } else { String::new() };
        col.0 = INK.with_alpha((2.0 - sel.1).clamp(0.0, 1.0));
    }
    if let Ok(mut n) = boss_root.single_mut() {
        n.display = if boss.active { Display::Flex } else { Display::None };
    }
    if let Ok(mut n) = boss_fill.single_mut() {
        n.width = percent(boss.frac.max(0.0) * 100.0);
    }
    if let Ok(mut n) = pause.single_mut() {
        n.display = if paused.0 { Display::Flex } else { Display::None };
    }
}

pub fn banner_flourish(banner: Res<Banner>, mut q: Query<&mut ImageNode, With<BannerFlourish>>) {
    if let Ok(mut img) = q.single_mut() {
        let a = if banner.title.is_empty() { 0.0 } else { 0.8 * banner.alpha() };
        img.color = Color::srgba(0.9, 0.92, 0.88, a);
    }
}

pub fn pause_input(
    c: Res<Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<AppState>>,
    mut loads: MessageWriter<LoadLevel>,
    progress: Res<Progress>,
) {
    if c.pause_pressed {
        paused.0 = !paused.0;
    }
    if paused.0 {
        if keys.just_pressed(KeyCode::KeyQ) {
            paused.0 = false;
            next.set(AppState::Title);
        }
        if keys.just_pressed(KeyCode::KeyR) {
            paused.0 = false;
            loads.write(LoadLevel::Start(progress.level));
        }
    }
    if paused.0 { time.pause() } else { time.unpause() }
}

// ---------------- screens ----------------

/// Arrows you'd have collected before reaching level `i`.
pub fn unlock_for_level(progress: &mut Progress, i: usize) {
    let by_level = [0usize, 0, 1, 3, 5];
    for k in 0..=by_level[i.min(4)] {
        progress.unlocked[k] = true;
    }
    progress.level = i;
}

pub fn spawn_card(commands: &mut Commands, art: &Art, state: AppState, title: &str, lines: &[String]) {
    commands
        .spawn((
            DespawnOnExit(state),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.02, 0.82)),
        ))
        .with_children(|c| {
            c.spawn(text(&art.font_title, title, 60.0, INK));
            c.spawn((
                ImageNode::new(art.ui_flourish.clone()).with_color(Color::srgba(0.9, 0.92, 0.88, 0.8)),
                Node { width: px(440), aspect_ratio: Some(720.0 / 118.0), margin: UiRect::vertical(px(-8)), ..default() },
            ));
            for l in lines {
                c.spawn(text(&art.font_body, l.clone(), 20.0, DIM));
            }
            c.spawn(text(&art.font_title, "Press Enter / Space / (A)", 18.0, INK));
        });
}

fn fmt_time(t: f32) -> String {
    format!("{}:{:02}", (t / 60.0) as u32, (t % 60.0) as u32)
}

pub fn spawn_level_done(mut commands: Commands, progress: Res<Progress>, art: Res<Art>) {
    let name = parse_level(LEVEL_SOURCES[progress.level]).name;
    spawn_card(
        &mut commands,
        &art,
        AppState::LevelDone,
        "Level Cleared",
        &[name, format!("time {}    deaths {}", fmt_time(progress.level_time), progress.deaths)],
    );
}

pub fn level_done_input(
    c: Res<Controls>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<AppState>>,
    mut loads: MessageWriter<LoadLevel>,
) {
    if c.confirm_pressed {
        progress.level += 1;
        loads.write(LoadLevel::Start(progress.level));
        next.set(AppState::Playing);
    }
}

pub fn spawn_victory(mut commands: Commands, progress: Res<Progress>, art: Res<Art>) {
    spawn_card(
        &mut commands,
        &art,
        AppState::Victory,
        "You escaped the deep",
        &[
            "The Warden has fallen and light pours through the last gate.".into(),
            format!("total time {}    deaths {}", fmt_time(progress.time), progress.deaths),
        ],
    );
}

pub fn victory_input(c: Res<Controls>, mut next: ResMut<NextState<AppState>>) {
    if c.confirm_pressed {
        next.set(AppState::Title);
    }
}
