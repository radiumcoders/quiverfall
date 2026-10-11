//! QUIVERFALL — a Hollow-Knight-flavoured archer platformer.
// Bevy systems routinely take many params and long query filters.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod arrows;
mod art;
mod audio;
mod bossbar;
mod camera;
mod config;
mod enemies;
mod fx;
mod game;
mod hud;
mod input;
mod level;
mod physics;
mod player;
mod props;
mod raster;
mod title;

use bevy::camera::ScalingMode;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;

use game::{AppState, is_playing};

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct Vignette;

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Quiverfall".into(),
                    name: Some(std::env::var("QF_APP_ID").unwrap_or_else(|_| "quiverfall".into())),
                    resolution: WindowResolution::new(1600, 900),
                    ..default()
                }),
                ..default()
            })
            .set(ImagePlugin::default_linear()),
    )
    .init_state::<AppState>()
    .insert_resource(ClearColor(Color::srgb_u8(8, 12, 16)))
    .init_resource::<input::Controls>()
    .init_resource::<game::Paused>()
    .init_resource::<game::Checkpoint>()
    .init_resource::<game::TileSprites>()
    .init_resource::<fx::Shake>()
    .init_resource::<camera::CamState>()
    .init_resource::<hud::Banner>()
    .init_resource::<hud::HintText>()
    .init_resource::<hud::ScreenFade>()
    .init_resource::<title::TitleMenu>()
    .init_resource::<enemies::BossBar>()
    .insert_resource(start_progress())
    .add_message::<audio::PlaySfx>()
    .add_message::<fx::Burst>()
    .add_message::<fx::Flash>()
    .add_message::<game::LoadLevel>()
    .add_systems(Startup, (setup_camera, art::load_art, audio::setup_audio, hud::spawn_fade))
    .add_systems(Update, game::wait_for_assets.run_if(in_state(AppState::Loading)))
    .add_systems(OnEnter(AppState::Title), (game::cleanup_level, title::spawn_title).chain())
    .add_systems(OnEnter(AppState::Playing), hud::spawn_hud)
    .add_systems(OnEnter(AppState::LevelDone), hud::spawn_level_done)
    .add_systems(OnEnter(AppState::Victory), hud::spawn_victory)
    .add_systems(
        Update,
        (
            input::gather_input,
            (
                (title::menu_input, title::menu_visuals, title::animate_scene).chain().run_if(in_state(AppState::Title)),
                hud::level_done_input.run_if(in_state(AppState::LevelDone)),
                hud::victory_input.run_if(in_state(AppState::Victory)),
                hud::pause_input.run_if(in_state(AppState::Playing)),
            ),
            game::load_level,
            (
                props::platforms_update,
                player::player_update,
                enemies::enemies_update,
                enemies::melee,
                arrows::arrows_update,
                enemies::projectiles,
                props::props_update,
                game::tick_tiles,
                game::handle_death,
                game::tick_progress,
                game::restart_level,
            )
                .chain()
                .run_if(is_playing),
            game::refresh_tiles,
            (
                player::player_visuals,
                arrows::aim_preview,
                enemies::enemy_visuals,
                camera::camera_follow,
                camera::parallax,
                fx::spawn_bursts,
                fx::spawn_flashes,
                fx::update_particles,
                fx::update_flashes,
                fx::ambient_motes,
                fx::attach_lights,
                fx::remove_lights,
                fx::flicker_lights,
                camera::fit_vignette,
            )
                .chain(),
            (hud::update_hud, hud::banner_flourish, bossbar::update_boss_bar).chain().run_if(in_state(AppState::Playing)),
            hud::update_fade,
            audio::play_sfx,
        )
            .chain(),
    )
    .add_systems(Update, (debug_tools, trace_player));
    if let Some(s) = input::Script::from_env() {
        app.insert_resource(s);
    }
    app.run();
}

fn start_progress() -> game::Progress {
    let mut p = game::Progress::default();
    if let Ok(v) = std::env::var("QF_LEVEL") {
        let i = v.parse::<usize>().unwrap_or(1).saturating_sub(1).min(level::LEVEL_SOURCES.len() - 1);
        hud::unlock_for_level(&mut p, i);
    }
    if std::env::var("QF_ALL_ARROWS").is_ok() {
        p.unlocked = [true; 6];
    }
    p
}

fn setup_camera(mut commands: Commands, art_vignette: Res<Assets<Image>>) {
    let _ = art_vignette;
    commands.spawn((
        MainCamera,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: config::VIEW_H },
            ..OrthographicProjection::default_2d()
        }),
        Tonemapping::TonyMcMapface,
        Bloom { intensity: 0.22, ..Bloom::NATURAL },
        DebandDither::Enabled,
    ));
}

/// Vignette lives on the camera; spawned once the art exists.
fn attach_vignette(
    mut commands: Commands,
    art: Option<Res<art::Art>>,
    cam: Query<Entity, With<MainCamera>>,
    existing: Query<(), With<Vignette>>,
) {
    let Some(art) = art else { return };
    if !existing.is_empty() {
        return;
    }
    if let Ok(c) = cam.single() {
        commands.entity(c).with_children(|p| {
            p.spawn((Vignette, Sprite { image: art.vignette.clone(), ..default() }, Transform::from_xyz(0.0, 0.0, 50.0)));
        });
    }
}

/// QF_SHOT=path.png[:seconds] saves a screenshot and exits (for automated checks).
fn debug_tools(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut done: Local<u8>,
    mut exit: MessageWriter<AppExit>,
    art: Option<Res<art::Art>>,
    cam: Query<Entity, With<MainCamera>>,
    existing: Query<(), With<Vignette>>,
) {
    attach_vignette(commands.reborrow(), art, cam, existing);
    let Ok(spec) = std::env::var("QF_SHOT") else { return };
    let (path, secs) = match spec.split_once(':') {
        Some((p, s)) => (p.to_string(), s.parse::<f32>().unwrap_or(3.0)),
        None => (spec.clone(), 3.0),
    };
    let t = time.elapsed_secs();
    if *done == 0 && t > secs {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *done = 1;
    } else if *done == 1 && t > secs + 1.0 {
        exit.write(AppExit::Success);
    }
}

/// QF_TRACE=1 logs the player's state ten times a second.
fn trace_player(time: Res<Time>, mut last: Local<f32>, q: Query<(&player::Player, &physics::Body, &Transform)>) {
    if std::env::var("QF_TRACE").is_err() {
        return;
    }
    let t = time.elapsed_secs();
    if t - *last < 0.1 {
        return;
    }
    *last = t;
    for (p, b, tf) in &q {
        info!(
            "t={t:.2} pos=({:.0},{:.0}) vel=({:.0},{:.0}) ground={} plat={} hp={} hz={:.2}",
            tf.translation.x, tf.translation.y, b.vel.x, b.vel.y, b.on_ground, b.ground_platform.is_some(), p.hp, p.hazard_t
        );
    }
}
