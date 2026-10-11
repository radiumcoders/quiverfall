//! Game flow: states, progress, level spawning/teardown, deaths and checkpoints.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::art::{self, Art, palette};
use crate::config::*;
use crate::enemies::{Enemy, spawn_enemy};
use crate::level::*;
use crate::physics::{Body, Platform};
use crate::player::{Player, spawn_player};
use crate::props::{Mover, Prop, spawn_prop};

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading,
    Title,
    Playing,
    LevelDone,
    Victory,
}

#[derive(Resource, Default)]
pub struct Paused(pub bool);

#[derive(Resource)]
pub struct Progress {
    pub level: usize,
    pub unlocked: [bool; 6],
    /// arrows unlocked when the current level began (restored on restart)
    pub unlocked_at_start: [bool; 6],
    pub deaths: u32,
    pub time: f32,
    pub level_time: f32,
}

impl Default for Progress {
    fn default() -> Self {
        let mut u = [false; 6];
        u[0] = true;
        Self { level: 0, unlocked: u, unlocked_at_start: u, deaths: 0, time: 0.0, level_time: 0.0 }
    }
}

/// Where the player respawns after dying.
#[derive(Resource, Default)]
pub struct Checkpoint {
    pub pos: Option<Vec2>,
}

/// Everything that belongs to the current level (despawned on reload).
#[derive(Component)]
pub struct LevelEntity;

#[derive(Component)]
pub struct TileSprite;

/// Sprites per tile so they can be rebuilt when tiles change.
#[derive(Resource, Default)]
pub struct TileSprites(pub HashMap<IVec2, Vec<Entity>>);

#[derive(Component)]
pub struct Parallax {
    pub factor: Vec2,
    pub base: Vec2,
}

#[derive(Component)]
pub struct GateVisual {
    pub id: u8,
    pub base_y: f32,
}

#[derive(Component)]
pub struct BossDoorVisual;

#[derive(Component)]
pub struct WaterVisual(pub f32);

#[derive(Component)]
pub struct CrumbleVisual(pub IVec2, pub Vec2);

#[derive(Message, Clone, Copy)]
pub enum LoadLevel {
    /// fresh start of the level at its start point
    Start(usize),
    /// reload after death: respawn at the checkpoint
    Respawn,
}

#[derive(Resource)]
pub struct Debug {
    pub enabled: bool,
}

pub fn wait_for_assets(
    server: Res<AssetServer>,
    art: Option<Res<Art>>,
    mut next: ResMut<NextState<AppState>>,
    mut loads: MessageWriter<LoadLevel>,
    progress: Res<Progress>,
) {
    let Some(art) = art else { return };
    let all = art
        .player
        .iter()
        .chain(&art.arrows)
        .chain(&art.icons)
        .chain(&art.tiles)
        .chain([&art.tex_stone, &art.tex_wood, &art.bow, &art.exit]);
    for h in all {
        if let Some(bevy::asset::LoadState::Failed(_)) = server.get_load_state(h) {
            continue; // missing art shows as blank, don't block
        }
        if !server.is_loaded_with_dependencies(h) {
            return;
        }
    }
    if std::env::var("QF_LEVEL").is_ok() {
        loads.write(LoadLevel::Start(progress.level));
        next.set(AppState::Playing);
    } else {
        next.set(AppState::Title);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn load_level(
    mut commands: Commands,
    mut msgs: MessageReader<LoadLevel>,
    existing: Query<Entity, With<LevelEntity>>,
    art: Res<Art>,
    images: Res<Assets<Image>>,
    mut progress: ResMut<Progress>,
    mut checkpoint: ResMut<Checkpoint>,
    mut tile_sprites: ResMut<TileSprites>,
    mut clear: ResMut<ClearColor>,
    mut cams: Query<&mut Transform, With<crate::MainCamera>>,
    server: Res<AssetServer>,
) {
    let Some(msg) = msgs.read().last().copied() else { return };
    for e in &existing {
        commands.entity(e).despawn();
    }
    tile_sprites.0.clear();
    let index = match msg {
        LoadLevel::Start(i) => {
            checkpoint.pos = None;
            progress.unlocked_at_start = progress.unlocked;
            progress.level_time = 0.0;
            i
        }
        LoadLevel::Respawn => progress.level,
    };
    progress.level = index;
    let def = parse_level(LEVEL_SOURCES[index]);
    let mut level = Level::from_def(index, &def);
    let pal = palette(level.biome);
    clear.0 = pal.clear;

    // ---- tiles ----
    let mut all = Vec::new();
    for y in 0..level.h {
        for x in 0..level.w {
            all.push(IVec2::new(x, y));
        }
    }
    for p in all {
        let ents = spawn_tile_visual(&mut commands, &level, p, &art, &images);
        if !ents.is_empty() {
            tile_sprites.0.insert(p, ents);
        }
    }
    level.dirty.clear();

    // ---- backgrounds ----
    let size = level.size_px();
    let (far, mid) = art::bg_paths(level.biome);
    let far_h = VIEW_H * 1.25;
    commands.spawn((
        LevelEntity,
        Sprite { image: server.load(far.clone()), custom_size: Some(Vec2::new(far_h * 16.0 / 9.0, far_h)), color: Color::srgb(0.75, 0.75, 0.8), ..default() },
        Transform::from_xyz(0.0, 0.0, -100.0),
        Parallax { factor: Vec2::new(0.92, 0.95), base: Vec2::ZERO },
    ));
    // mid layer, mirrored copies across the level
    let mid_h = VIEW_H * 1.1;
    let mid_w = mid_h * 16.0 / 9.0;
    let span = size.x * 0.45 + VIEW_H * 2.0;
    let n = (span / mid_w).ceil() as i32 + 1;
    for i in -1..n {
        commands.spawn((
            LevelEntity,
            Sprite {
                image: server.load(mid.clone()),
                custom_size: Some(Vec2::new(mid_w + 1.0, mid_h)),
                flip_x: i % 2 != 0,
                color: Color::srgb(0.55, 0.58, 0.65),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, -60.0),
            Parallax { factor: Vec2::new(0.55, 0.7), base: Vec2::new(i as f32 * mid_w, -40.0) },
        ));
    }
    // low fog band over the whole level
    commands.spawn((
        LevelEntity,
        Sprite { image: art.fog.clone(), custom_size: Some(Vec2::new(size.x + 2000.0, size.y * 0.6)), color: pal.fog, ..default() },
        Transform::from_xyz(size.x / 2.0, size.y * 0.3 - 40.0, -40.0),
    ));

    // ---- entities ----
    let mut player_pos = Vec2::new(64.0, 64.0);
    for (s, p) in &def.spawns {
        let world = Vec2::new(p.x as f32 * TILE + TILE / 2.0, p.y as f32 * TILE);
        match *s {
            Place::Player => player_pos = world + Vec2::new(0.0, PLAYER_HALF.1 + 0.5),
            Place::Enemy(kind) => spawn_enemy(&mut commands, &art, kind, world),
            Place::Mover { dx, dy, speed, width } => {
                let half = Vec2::new(width * TILE / 2.0, 8.0);
                let c = world + Vec2::new(width * TILE / 2.0 - TILE / 2.0, TILE - 8.0);
                commands.spawn((
                    LevelEntity,
                    Platform { half, delta: Vec2::ZERO },
                    Mover { origin: c, offset: Vec2::new(dx, dy) * TILE, speed, t: 0.0 },
                    Sprite {
                        image: art.platforms[art::PLAT_SLAB].clone(),
                        custom_size: Some(Vec2::new(half.x * 2.0 + 8.0, 30.0)),
                        ..default()
                    },
                    bevy::sprite::Anchor(Vec2::new(0.0, 0.25)),
                    Transform::from_translation(c.extend(3.0)),
                ));
            }
            other => spawn_prop(&mut commands, &art, other, world, &level),
        }
    }
    if matches!(msg, LoadLevel::Respawn)
        && let Some(cp) = checkpoint.pos
    {
        player_pos = cp;
    }
    if let Ok(v) = std::env::var("QF_TELEPORT")
        && let Some((x, y)) = v.split_once(',')
    {
        player_pos = Vec2::new(x.parse::<f32>().unwrap_or(2.0) * TILE + TILE / 2.0, y.parse::<f32>().unwrap_or(2.0) * TILE + PLAYER_HALF.1);
    }
    spawn_player(&mut commands, &art, player_pos);
    for mut t in &mut cams {
        t.translation.x = player_pos.x;
        t.translation.y = player_pos.y + 40.0;
    }
    commands.insert_resource(level);
    commands.insert_resource(crate::hud::Banner::new(&def.name, &def.subtitle));
}

/// Distance (in tiles, capped) from a solid tile to the nearest non-solid tile — used to darken
/// deep rock so terrain reads as a silhouette with lit edges.
fn depth(level: &Level, p: IVec2) -> i32 {
    let fam = |t: Tile| matches!(t, Tile::Stone | Tile::Wood);
    for r in 1..=3i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let q = p + IVec2::new(dx, dy);
                if q.x < 0 || q.x >= level.w || q.y < 0 || q.y >= level.h {
                    continue;
                }
                if !fam(level.get(q.x, q.y)) {
                    return r;
                }
            }
        }
    }
    4
}

pub fn spawn_tile_visual(commands: &mut Commands, level: &Level, p: IVec2, art: &Art, images: &Assets<Image>) -> Vec<Entity> {
    let t = level.get(p.x, p.y);
    let c = Level::tile_center(p);
    let pal = palette(level.biome);
    let mut out = Vec::new();
    let mut spawn = |commands: &mut Commands, sprite: Sprite, tr: Transform| {
        out.push(commands.spawn((LevelEntity, TileSprite, sprite, tr)).id());
    };
    let tsize = Vec2::splat(TILE);
    match t {
        Tile::Empty => {}
        Tile::Stone | Tile::Wood => {
            let (tex, repeat) = if t == Tile::Stone { (&art.tex_stone, 4) } else { (&art.tex_wood, 4) };
            let isz = art::img_size(images, tex).max(Vec2::splat(1.0));
            let cell = isz / repeat as f32;
            let (rx, ry) = (p.x.rem_euclid(repeat), (level.h - 1 - p.y).rem_euclid(repeat));
            let flip_x = p.x.div_euclid(repeat) % 2 != 0;
            let flip_y = (level.h - 1 - p.y).div_euclid(repeat) % 2 != 0;
            let rx = if flip_x { repeat - 1 - rx } else { rx };
            let ry = if flip_y { repeat - 1 - ry } else { ry };
            let rect = Rect::new(rx as f32 * cell.x, ry as f32 * cell.y, (rx + 1) as f32 * cell.x, (ry + 1) as f32 * cell.y);
            let d = depth(level, p);
            let shade = match d {
                1 => 1.0,
                2 => 0.62,
                3 => 0.42,
                _ => 0.3,
            };
            let tint = pal.stone_tint.to_srgba();
            let base = if t == Tile::Wood { 1.15 } else { 1.0 };
            spawn(
                commands,
                Sprite {
                    image: tex.clone(),
                    rect: Some(rect),
                    flip_x,
                    flip_y,
                    custom_size: Some(tsize + Vec2::splat(0.6)),
                    color: Color::srgb(tint.red * shade * base, tint.green * shade * base, tint.blue * shade * base),
                    ..default()
                },
                Transform::from_translation(c.extend(0.0)),
            );
            // lit edges on faces that border open space
            let open = |dx: i32, dy: i32| {
                let q = p + IVec2::new(dx, dy);
                !matches!(level.get(q.x, q.y), Tile::Stone | Tile::Wood | Tile::Crumble | Tile::Thorns)
                    && q.y < level.h
                    && q.x >= 0
                    && q.x < level.w
            };
            let rim_col = pal.moss.to_srgba();
            for (dx, dy, rot) in [(0, 1, 0.0f32), (1, 0, -90.0), (0, -1, 180.0), (-1, 0, 90.0)] {
                if open(dx, dy) {
                    let off = Vec2::new(dx as f32, dy as f32) * (TILE / 2.0 - 5.0);
                    let alpha = if dy == 1 { 0.55 } else if dy == -1 { 0.18 } else { 0.32 };
                    spawn(
                        commands,
                        Sprite {
                            image: art.rim.clone(),
                            custom_size: Some(Vec2::new(TILE + 0.5, 10.0)),
                            color: Color::srgba(rim_col.red, rim_col.green, rim_col.blue, alpha),
                            ..default()
                        },
                        Transform::from_translation((c + off).extend(0.1))
                            .with_rotation(Quat::from_rotation_z(rot.to_radians())),
                    );
                }
            }
            if open(0, 1) && t == Tile::Stone {
                let flip = crate::raster::hash2(p.x, p.y, 3) > 0.5;
                spawn(
                    commands,
                    Sprite {
                        image: art.moss.clone(),
                        rect: Some(Rect::new((p.x.rem_euclid(4)) as f32 * 32.0, 0.0, (p.x.rem_euclid(4) + 1) as f32 * 32.0, 32.0)),
                        custom_size: Some(Vec2::new(TILE + 2.0, 12.0)),
                        flip_x: flip,
                        color: pal.moss.with_alpha(0.9),
                        ..default()
                    },
                    Transform::from_translation((c + Vec2::new(0.0, TILE / 2.0 + 3.0)).extend(4.5)),
                );
            }
        }
        Tile::OneWay => spawn(
            commands,
            Sprite { image: art.tiles[art::TILE_PLANK].clone(), custom_size: Some(Vec2::new(TILE + 2.0, 12.0)), ..default() },
            Transform::from_translation((c + Vec2::new(0.0, TILE / 2.0 - 5.0)).extend(0.5)),
        ),
        Tile::Spikes | Tile::SpikesDown => {
            let down = t == Tile::SpikesDown;
            let off = if down { TILE / 2.0 - 10.0 } else { -TILE / 2.0 + 10.0 };
            spawn(
                commands,
                Sprite {
                    image: art.tiles[art::TILE_SPIKES].clone(),
                    custom_size: Some(Vec2::new(TILE + 2.0, 22.0)),
                    flip_y: down,
                    color: Color::srgb(0.9, 0.9, 0.9),
                    ..default()
                },
                Transform::from_translation((c + Vec2::new(0.0, off)).extend(0.4)),
            );
        }
        Tile::Water => {
            let surface = level.get(p.x, p.y + 1) != Tile::Water;
            spawn(
                commands,
                Sprite {
                    image: if surface { art.tiles[art::TILE_WATER].clone() } else { art.pixel.clone() },
                    custom_size: Some(tsize + Vec2::new(1.0, 0.5)),
                    color: if surface { Color::srgba(0.8, 0.9, 1.0, 0.95) } else { Color::srgba(0.01, 0.03, 0.05, 0.96) },
                    ..default()
                },
                Transform::from_translation(c.extend(5.5)),
            );
            if let Some(e) = out.last() {
                commands.entity(*e).insert(WaterVisual(p.x as f32 * 0.7));
            }
        }
        Tile::Thorns => spawn(
            commands,
            Sprite {
                image: art.tiles[art::TILE_THORNS].clone(),
                custom_size: Some(tsize * 1.25),
                flip_x: (p.x + p.y) % 2 == 0,
                ..default()
            },
            Transform::from_translation(c.extend(0.3)),
        ),
        Tile::Grate => spawn(
            commands,
            Sprite { image: art.tiles[art::TILE_GRATE].clone(), custom_size: Some(tsize), ..default() },
            Transform::from_translation(c.extend(0.2)),
        ),
        Tile::Crumble => {
            spawn(
                commands,
                Sprite { image: art.tiles[art::TILE_CRUMBLE].clone(), custom_size: Some(tsize + Vec2::splat(1.0)), ..default() },
                Transform::from_translation(c.extend(0.2)),
            );
            if let Some(e) = out.last() {
                commands.entity(*e).insert(CrumbleVisual(p, c));
            }
        }
        Tile::Ice => spawn(
            commands,
            Sprite { image: art.tiles[art::TILE_ICE].clone(), custom_size: Some(tsize + Vec2::splat(1.0)), ..default() },
            Transform::from_translation(c.extend(0.2)),
        ),
        Tile::Gate(id) => {
            spawn(
                commands,
                Sprite { image: art.tiles[art::TILE_GATE].clone(), custom_size: Some(tsize + Vec2::new(0.0, 1.0)), ..default() },
                Transform::from_translation(c.extend(0.25)),
            );
            if let Some(e) = out.last() {
                commands.entity(*e).insert(GateVisual { id, base_y: c.y });
            }
        }
        Tile::BossDoor => {
            spawn(
                commands,
                Sprite {
                    image: art.tiles[art::TILE_GATE].clone(),
                    custom_size: Some(tsize + Vec2::new(0.0, 1.0)),
                    color: Color::srgba(1.4, 0.6, 0.4, 0.0),
                    ..default()
                },
                Transform::from_translation(c.extend(0.25)),
            );
            if let Some(e) = out.last() {
                commands.entity(*e).insert(BossDoorVisual);
            }
        }
    }
    out
}

/// Rebuild sprites for tiles that changed (burnt thorns, frozen water, ...), plus neighbours.
pub fn refresh_tiles(
    mut commands: Commands,
    level: Option<ResMut<Level>>,
    art: Res<Art>,
    images: Res<Assets<Image>>,
    mut ts: ResMut<TileSprites>,
) {
    let Some(mut level) = level else { return };
    if level.dirty.is_empty() {
        return;
    }
    let mut todo: Vec<IVec2> = Vec::new();
    for p in level.dirty.drain(..) {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let q = p + IVec2::new(dx, dy);
                if !todo.contains(&q) {
                    todo.push(q);
                }
            }
        }
    }
    for p in todo {
        if let Some(old) = ts.0.remove(&p) {
            for e in old {
                commands.entity(e).despawn();
            }
        }
        let ents = spawn_tile_visual(&mut commands, &level, p, &art, &images);
        if !ents.is_empty() {
            ts.0.insert(p, ents);
        }
    }
}

/// Tile-level ticking: burning thorns, crumbling floors, gate animation.
#[allow(clippy::type_complexity)]
pub fn tick_tiles(
    time: Res<Time>,
    level: Option<ResMut<Level>>,
    mut gates: Query<(&GateVisual, &mut Transform, &mut Sprite), (Without<BossDoorVisual>, Without<CrumbleVisual>, Without<WaterVisual>)>,
    mut doors: Query<&mut Sprite, (With<BossDoorVisual>, Without<GateVisual>, Without<CrumbleVisual>, Without<WaterVisual>)>,
    mut crumbles: Query<(&CrumbleVisual, &mut Transform, &mut Sprite), (Without<GateVisual>, Without<WaterVisual>)>,
    mut fx: MessageWriter<crate::fx::Burst>,
    mut sfx: MessageWriter<crate::audio::PlaySfx>,
) {
    let Some(mut level) = level else { return };
    let dt = time.delta_secs().min(1.0 / 30.0);
    let burning: Vec<IVec2> = level.burning.keys().copied().collect();
    for p in &burning {
        if rand::random::<f32>() < 0.5 {
            fx.write(crate::fx::Burst::embers(Level::tile_center(*p), 1));
        }
    }
    let burnt = tick_level(&mut level, dt);
    for p in burnt {
        fx.write(crate::fx::Burst::embers(Level::tile_center(p), 6));
        sfx.write(crate::audio::PlaySfx(crate::audio::Sfx::Burn, 0.25));
    }
    for i in 0..16 {
        let target = if level.gate_open[i] { 1.0 } else { 0.0 };
        let a = &mut level.gate_anim[i];
        *a += (target - *a).clamp(-dt * 2.0, dt * 1.6);
    }
    for (g, mut t, mut s) in &mut gates {
        let a = level.gate_anim[g.id as usize];
        t.translation.y = g.base_y + a * TILE * 0.9;
        s.color = Color::srgba(1.0, 1.0, 1.0, 1.0 - a);
    }
    for mut s in &mut doors {
        let target = if level.boss_active { 1.0 } else { 0.0 };
        let a = s.color.alpha();
        s.color = s.color.with_alpha(a + (target - a) * (dt * 6.0).min(1.0));
    }
    for (c, mut t, mut s) in &mut crumbles {
        match level.crumble.get(&c.0) {
            Some(CrumbleState::Shaking(_)) => {
                t.translation.x = c.1.x + (rand::random::<f32>() - 0.5) * 3.0;
                t.translation.y = c.1.y + (rand::random::<f32>() - 0.5) * 2.0;
                s.color = Color::WHITE;
            }
            Some(CrumbleState::Gone(_)) => s.color = Color::NONE,
            None => {
                t.translation.x = c.1.x;
                t.translation.y = c.1.y;
                s.color = Color::WHITE;
            }
        }
    }
}

/// Global timers.
pub fn tick_progress(time: Res<Time>, mut p: ResMut<Progress>) {
    p.time += time.delta_secs();
    p.level_time += time.delta_secs();
}

/// Player died: count it, then reload at the checkpoint (arrows found so far are kept).
pub fn handle_death(players: Query<&Player>, mut progress: ResMut<Progress>, mut loads: MessageWriter<LoadLevel>) {
    for p in &players {
        if p.hp <= 0 && p.dead_t > 1.4 {
            progress.deaths += 1;
            loads.write(LoadLevel::Respawn);
        }
    }
}

pub fn restart_level(
    controls: Res<crate::input::Controls>,
    paused: Res<Paused>,
    progress: Res<Progress>,
    mut loads: MessageWriter<LoadLevel>,
) {
    if controls.restart_pressed && !paused.0 {
        loads.write(LoadLevel::Start(progress.level));
    }
}

pub fn is_playing(state: Res<State<AppState>>, paused: Res<Paused>) -> bool {
    *state.get() == AppState::Playing && !paused.0
}

pub fn cleanup_level(mut commands: Commands, q: Query<Entity, With<LevelEntity>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<Level>();
}

// keep a few imports referenced
#[allow(dead_code)]
fn _unused(_: Body, _: Enemy, _: Prop) {}
