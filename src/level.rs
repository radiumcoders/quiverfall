//! Tile grid, level file parsing and collision queries.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::arrows::ArrowKind;
use crate::config::TILE;
use crate::enemies::EnemyKind;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Empty,
    Stone,
    Wood,
    OneWay,
    Spikes,
    SpikesDown,
    Water,
    Thorns,
    Grate,
    Crumble,
    Ice,
    Gate(u8),
    /// Seals the boss arena while the Warden fight is on.
    BossDoor,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    Player,
    Exit,
    Lantern,
    Anchor,
    Shrine(ArrowKind),
    Target(u8),
    Brazier(u8),
    Enemy(EnemyKind),
    Mover { dx: f32, dy: f32, speed: f32, width: f32 },
    Hint(usize),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CrumbleState {
    Shaking(f32),
    Gone(f32),
}

pub struct LevelDef {
    pub name: String,
    pub subtitle: String,
    pub biome: Biome,
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<Tile>,
    pub spawns: Vec<(Place, IVec2)>,
    pub hints: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Gate,
    Shafts,
    Roots,
    Cistern,
    Summit,
}

impl Biome {
    pub fn key(self) -> &'static str {
        match self {
            Biome::Gate => "gate",
            Biome::Shafts => "shafts",
            Biome::Roots => "roots",
            Biome::Cistern => "cistern",
            Biome::Summit => "summit",
        }
    }
}

pub const LEVEL_SOURCES: &[&str] = &[
    include_str!("../levels/1_hollow_gate.txt"),
    include_str!("../levels/2_lift_shafts.txt"),
    include_str!("../levels/3_rootdeep.txt"),
    include_str!("../levels/4_frozen_cistern.txt"),
    include_str!("../levels/5_wardens_ascent.txt"),
];

pub fn parse_level(src: &str) -> LevelDef {
    let mut name = String::new();
    let mut subtitle = String::new();
    let mut biome = Biome::Gate;
    let mut legend: HashMap<char, Vec<String>> = HashMap::new();
    let mut hints = Vec::new();
    let mut rows: Vec<&str> = Vec::new();
    let mut in_grid = false;
    for line in src.lines() {
        if in_grid {
            rows.push(line);
            continue;
        }
        let l = line.trim();
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        if l == "grid:" {
            in_grid = true;
        } else if let Some(v) = l.strip_prefix("name:") {
            name = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("subtitle:") {
            subtitle = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("hint:") {
            hints.push(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("biome:") {
            biome = match v.trim() {
                "shafts" => Biome::Shafts,
                "roots" => Biome::Roots,
                "cistern" => Biome::Cistern,
                "summit" => Biome::Summit,
                _ => Biome::Gate,
            };
        } else if let Some((k, v)) = l.split_once('=') {
            let c = k.trim().chars().next().expect("legend char");
            legend.insert(c, v.split_whitespace().map(str::to_string).collect());
        }
    }
    while rows.last().is_some_and(|r| r.trim().is_empty()) {
        rows.pop();
    }
    let h = rows.len() as i32;
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
    let mut tiles = vec![Tile::Empty; (w * h) as usize];
    let mut spawns = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        let y = h - 1 - row as i32;
        for (x, c) in line.chars().enumerate() {
            let x = x as i32;
            let pos = IVec2::new(x, y);
            let mut tile = Tile::Empty;
            if let Some(def) = legend.get(&c) {
                let num = |i: usize| def.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                match def[0].as_str() {
                    "target" => spawns.push((Place::Target(num(1) as u8), pos)),
                    "brazier" => spawns.push((Place::Brazier(num(1) as u8), pos)),
                    "gate" => tile = Tile::Gate(num(1) as u8),
                    "shrine" => spawns.push((Place::Shrine(ArrowKind::from_name(&def[1])), pos)),
                    "mover" => spawns.push((
                        Place::Mover { dx: num(1), dy: num(2), speed: num(3), width: num(4).max(2.0) },
                        pos,
                    )),
                    "hint" => spawns.push((Place::Hint(num(1) as usize), pos)),
                    other => panic!("unknown legend entry {other}"),
                }
            } else {
                match c {
                    '#' => tile = Tile::Stone,
                    'W' => tile = Tile::Wood,
                    '-' => tile = Tile::OneWay,
                    '^' => tile = Tile::Spikes,
                    'v' => tile = Tile::SpikesDown,
                    '~' => tile = Tile::Water,
                    'T' => tile = Tile::Thorns,
                    '|' => tile = Tile::Grate,
                    '%' => tile = Tile::Crumble,
                    'Z' => tile = Tile::BossDoor,
                    'P' => spawns.push((Place::Player, pos)),
                    'E' => spawns.push((Place::Exit, pos)),
                    'L' => spawns.push((Place::Lantern, pos)),
                    'o' => spawns.push((Place::Anchor, pos)),
                    'c' => spawns.push((Place::Enemy(EnemyKind::Crawler), pos)),
                    'h' => spawns.push((Place::Enemy(EnemyKind::Hopper), pos)),
                    'm' => spawns.push((Place::Enemy(EnemyKind::Moth), pos)),
                    's' => spawns.push((Place::Enemy(EnemyKind::Spitter), pos)),
                    'k' => spawns.push((Place::Enemy(EnemyKind::Shieldbearer), pos)),
                    'B' => spawns.push((Place::Enemy(EnemyKind::Warden), pos)),
                    _ => {}
                }
            }
            tiles[(y * w + x) as usize] = tile;
        }
    }
    LevelDef { name, subtitle, biome, w, h, tiles, spawns, hints }
}

#[derive(Resource)]
pub struct Level {
    pub index: usize,
    pub name: String,
    pub subtitle: String,
    pub biome: Biome,
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<Tile>,
    pub crumble: HashMap<IVec2, CrumbleState>,
    pub burning: HashMap<IVec2, f32>,
    pub gate_open: [bool; 16],
    pub gate_anim: [f32; 16],
    pub boss_active: bool,
    /// tiles whose sprite needs a refresh (also their neighbours, for edge shading)
    pub dirty: Vec<IVec2>,
    pub hints: Vec<String>,
}

impl Level {
    pub fn from_def(index: usize, def: &LevelDef) -> Self {
        Self {
            index,
            name: def.name.clone(),
            subtitle: def.subtitle.clone(),
            biome: def.biome,
            w: def.w,
            h: def.h,
            tiles: def.tiles.clone(),
            crumble: HashMap::new(),
            burning: HashMap::new(),
            gate_open: [false; 16],
            gate_anim: [0.0; 16],
            boss_active: false,
            dirty: Vec::new(),
            hints: def.hints.clone(),
        }
    }

    pub fn size_px(&self) -> Vec2 {
        Vec2::new(self.w as f32 * TILE, self.h as f32 * TILE)
    }

    pub fn get(&self, x: i32, y: i32) -> Tile {
        if x < 0 || x >= self.w || y < 0 || y >= self.h {
            return Tile::Empty;
        }
        self.tiles[(y * self.w + x) as usize]
    }

    pub fn set(&mut self, p: IVec2, t: Tile) {
        if p.x < 0 || p.x >= self.w || p.y < 0 || p.y >= self.h {
            return;
        }
        self.tiles[(p.y * self.w + p.x) as usize] = t;
        self.dirty.push(p);
    }

    /// Solid for players and enemies. Outside the left/right/top edges is wall; below is a pit.
    pub fn solid(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= self.w || y >= self.h {
            return y >= 0;
        }
        match self.get(x, y) {
            Tile::Stone | Tile::Wood | Tile::Ice | Tile::Grate | Tile::Thorns => true,
            Tile::Crumble => !matches!(self.crumble.get(&IVec2::new(x, y)), Some(CrumbleState::Gone(_))),
            Tile::Gate(id) => !self.gate_open[id as usize],
            Tile::BossDoor => self.boss_active,
            _ => false,
        }
    }

    /// Solid for arrows: grates let arrows through.
    pub fn arrow_solid(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= self.w || y >= self.h || y < 0 {
            return true;
        }
        match self.get(x, y) {
            Tile::Grate => false,
            _ => self.solid(x, y),
        }
    }

    pub fn one_way(&self, x: i32, y: i32) -> bool {
        matches!(self.get(x, y), Tile::OneWay)
    }

    pub fn tile_at(p: Vec2) -> IVec2 {
        IVec2::new((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32)
    }

    pub fn tile_center(t: IVec2) -> Vec2 {
        Vec2::new((t.x as f32 + 0.5) * TILE, (t.y as f32 + 0.5) * TILE)
    }

    /// Does an AABB touch any tile matching `f`? `shrink` insets the box.
    pub fn box_touches(&self, center: Vec2, half: Vec2, f: impl Fn(Tile, IVec2) -> bool) -> Option<IVec2> {
        let min = Self::tile_at(center - half);
        let max = Self::tile_at(center + half - Vec2::splat(0.01));
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                if f(self.get(x, y), IVec2::new(x, y)) {
                    return Some(IVec2::new(x, y));
                }
            }
        }
        None
    }

    pub fn box_solid(&self, center: Vec2, half: Vec2) -> bool {
        let min = Self::tile_at(center - half);
        let max = Self::tile_at(center + half - Vec2::splat(0.01));
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                if self.solid(x, y) {
                    return true;
                }
            }
        }
        false
    }

    /// Hazard boxes are a bit smaller than the tile so grazing a spike tip is forgiven.
    pub fn hazard_at(&self, center: Vec2, half: Vec2) -> Option<IVec2> {
        let min = Self::tile_at(center - half - Vec2::splat(1.0));
        let max = Self::tile_at(center + half + Vec2::splat(1.0));
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let t = self.get(x, y);
                let (lo, hi) = match t {
                    Tile::Spikes => (Vec2::new(4.0, 0.0), Vec2::new(28.0, 16.0)),
                    Tile::SpikesDown => (Vec2::new(4.0, 16.0), Vec2::new(28.0, 32.0)),
                    Tile::Water => (Vec2::new(0.0, 0.0), Vec2::new(32.0, 22.0)),
                    Tile::Thorns => (Vec2::new(-2.0, -2.0), Vec2::new(34.0, 34.0)),
                    _ => continue,
                };
                let o = Vec2::new(x as f32, y as f32) * TILE;
                let (a0, a1) = (o + lo, o + hi);
                let (b0, b1) = (center - half, center + half);
                if a0.x < b1.x && a1.x > b0.x && a0.y < b1.y && a1.y > b0.y {
                    return Some(IVec2::new(x, y));
                }
            }
        }
        None
    }
}

/// Ember: thorns burn, then spread to neighbouring thorns.
pub fn tick_level(level: &mut Level, dt: f32) -> Vec<IVec2> {
    let mut burnt = Vec::new();
    let keys: Vec<IVec2> = level.burning.keys().copied().collect();
    for p in keys {
        let t = level.burning.get_mut(&p).unwrap();
        *t -= dt;
        if *t <= 0.0 {
            level.burning.remove(&p);
            level.set(p, Tile::Empty);
            burnt.push(p);
            for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let n = p + d;
                if level.get(n.x, n.y) == Tile::Thorns && !level.burning.contains_key(&n) {
                    level.burning.insert(n, 0.12);
                }
            }
        }
    }
    let keys: Vec<IVec2> = level.crumble.keys().copied().collect();
    for p in keys {
        let s = level.crumble.get_mut(&p).unwrap();
        match s {
            CrumbleState::Shaking(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    *s = CrumbleState::Gone(3.5);
                    level.dirty.push(p);
                }
            }
            CrumbleState::Gone(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    level.crumble.remove(&p);
                    level.dirty.push(p);
                }
            }
        }
    }
    burnt
}

/// Frost: freeze the water surface connected to `p`.
pub fn freeze_water(level: &mut Level, p: IVec2) -> Vec<IVec2> {
    // find the surface tile in this column
    let mut s = p;
    while level.get(s.x, s.y + 1) == Tile::Water {
        s.y += 1;
    }
    if level.get(s.x, s.y) != Tile::Water {
        return vec![];
    }
    let mut out = vec![];
    for dir in [-1, 1] {
        for i in 0..11 {
            if dir == 1 && i == 0 {
                continue;
            }
            let q = IVec2::new(s.x + dir * i, s.y);
            if level.get(q.x, q.y) != Tile::Water || level.get(q.x, q.y + 1) == Tile::Water {
                break;
            }
            level.set(q, Tile::Ice);
            out.push(q);
        }
    }
    out
}
