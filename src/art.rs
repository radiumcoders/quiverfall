//! All image handles: generated art from assets/art (see tools/) plus a few procedural helpers.

use bevy::prelude::*;

use crate::level::Biome;
use crate::raster::*;

#[derive(Resource)]
pub struct Art {
    pub player: Vec<Handle<Image>>,
    pub bow: Handle<Image>,
    pub arrows: Vec<Handle<Image>>,
    pub icons: Vec<Handle<Image>>,
    pub crawler: Vec<Handle<Image>>,
    pub hopper: Vec<Handle<Image>>,
    pub moth: Vec<Handle<Image>>,
    pub spitter: Vec<Handle<Image>>,
    pub shieldbearer: Vec<Handle<Image>>,
    pub warden: Vec<Handle<Image>>,
    pub props: Vec<Handle<Image>>,
    pub exit: Handle<Image>,
    pub platforms: Vec<Handle<Image>>,
    pub tiles: Vec<Handle<Image>>,
    pub tex_stone: Handle<Image>,
    pub tex_wood: Handle<Image>,
    pub tex_ice: Handle<Image>,
    pub title: Handle<Image>,
    pub font_title: Handle<Font>,
    pub font_body: Handle<Font>,
    // procedural
    pub glow: Handle<Image>,
    pub dot: Handle<Image>,
    pub pixel: Handle<Image>,
    pub rim: Handle<Image>,
    pub moss: Handle<Image>,
    pub vignette: Handle<Image>,
    pub fog: Handle<Image>,
}

// sheet indices
pub const ICON_MASK: usize = 6;
pub const ICON_MASK_EMPTY: usize = 7;
pub const ICON_ORB: usize = 8;
pub const ICON_ORB_EMPTY: usize = 9;
pub const PROP_LANTERN: usize = 0;
pub const PROP_LANTERN_LIT: usize = 1;
pub const PROP_TARGET: usize = 2;
pub const PROP_TARGET_HIT: usize = 3;
pub const PROP_BRAZIER: usize = 4;
pub const PROP_BRAZIER_LIT: usize = 5;
pub const PROP_ANCHOR: usize = 6;
pub const PROP_SHRINE: usize = 7;
pub const PLAT_LIFT: usize = 0;
pub const PLAT_SLAB: usize = 1;
pub const TILE_SPIKES: usize = 0;
pub const TILE_THORNS: usize = 1;
pub const TILE_GRATE: usize = 2;
pub const TILE_CRUMBLE: usize = 3;
pub const TILE_GATE: usize = 4;
pub const TILE_PLANK: usize = 5;
pub const TILE_WATER: usize = 7;

pub struct Palette {
    pub clear: Color,
    pub accent: Color,
    pub moss: Color,
    pub fog: Color,
    pub stone_tint: Color,
}

pub fn palette(b: Biome) -> Palette {
    match b {
        Biome::Gate => Palette {
            clear: Color::srgb_u8(8, 12, 16),
            accent: Color::srgb(0.75, 1.0, 0.8),
            moss: Color::srgb_u8(120, 160, 120),
            fog: Color::srgba(0.45, 0.65, 0.6, 0.10),
            stone_tint: Color::srgb(0.85, 0.92, 0.95),
        },
        Biome::Shafts => Palette {
            clear: Color::srgb_u8(6, 9, 18),
            accent: Color::srgb(0.6, 0.8, 1.2),
            moss: Color::srgb_u8(120, 150, 190),
            fog: Color::srgba(0.4, 0.55, 0.85, 0.10),
            stone_tint: Color::srgb(0.8, 0.88, 1.0),
        },
        Biome::Roots => Palette {
            clear: Color::srgb_u8(14, 8, 8),
            accent: Color::srgb(1.3, 0.75, 0.4),
            moss: Color::srgb_u8(170, 110, 80),
            fog: Color::srgba(0.8, 0.45, 0.3, 0.09),
            stone_tint: Color::srgb(1.0, 0.85, 0.8),
        },
        Biome::Cistern => Palette {
            clear: Color::srgb_u8(4, 12, 16),
            accent: Color::srgb(0.6, 1.1, 1.3),
            moss: Color::srgb_u8(130, 200, 210),
            fog: Color::srgba(0.4, 0.75, 0.85, 0.10),
            stone_tint: Color::srgb(0.8, 0.95, 1.0),
        },
        Biome::Summit => Palette {
            clear: Color::srgb_u8(14, 12, 10),
            accent: Color::srgb(1.3, 1.05, 0.6),
            moss: Color::srgb_u8(200, 180, 130),
            fog: Color::srgba(0.85, 0.7, 0.45, 0.09),
            stone_tint: Color::srgb(1.0, 0.95, 0.88),
        },
    }
}

pub fn bg_paths(b: Biome) -> (String, String) {
    (format!("art/bg_{}_far.png", b.key()), format!("art/bg_{}_mid.png", b.key()))
}

pub fn load_art(mut commands: Commands, server: Res<AssetServer>, mut images: ResMut<Assets<Image>>) {
    let seq = |name: &str, n: usize| -> Vec<Handle<Image>> {
        (0..n).map(|i| server.load(format!("art/{name}_{i}.png"))).collect()
    };
    let one = |name: &str| -> Handle<Image> { server.load(format!("art/{name}.png")) };

    // radial glow
    let mut c = Canvas::new(128, 128);
    c.glow(|p| sd_circle(p, Vec2::splat(64.0), 0.0), [1.0, 1.0, 1.0, 1.0], 63.0);
    let glow = images.add(c.into_image(false));
    // soft dot for particles
    let mut c = Canvas::new(32, 32);
    c.fill(|p| sd_circle(p, Vec2::splat(16.0), 10.0), [1.0; 4]);
    c.glow(|p| sd_circle(p, Vec2::splat(16.0), 10.0), [1.0, 1.0, 1.0, 0.5], 5.0);
    let dot = images.add(c.into_image(false));
    let mut c = Canvas::new(4, 4);
    c.fill(|_| -1.0, [1.0; 4]);
    let pixel = images.add(c.into_image(true));
    // rim light strip: bright line at the top fading down (used on exposed tile faces)
    let mut c = Canvas::new(64, 16);
    c.map(|_, y, _| {
        let t = y as f32 / 16.0;
        let a = if y < 2 { 0.9 } else { (1.0 - t).powf(2.5) * 0.35 };
        [1.0, 1.0, 1.0, a]
    });
    let rim = images.add(c.into_image(false));
    // moss / grass tufts hanging over a top edge (white, tinted per biome)
    let mut c = Canvas::new(128, 32);
    for i in 0..22 {
        let x = hash2(i, 3, 7) * 128.0;
        let h = 6.0 + hash2(i, 9, 7) * 16.0;
        let lean = (hash2(i, 5, 7) - 0.5) * 8.0;
        let base = Vec2::new(x, 31.0);
        let tip = Vec2::new(x + lean, 31.0 - h);
        c.fill(
            |p| {
                let d = sd_segment(p, base, tip);
                let t = ((p.y - tip.y) / h).clamp(0.0, 1.0);
                d - (0.5 + 2.2 * t)
            },
            [1.0, 1.0, 1.0, 0.95],
        );
    }
    c.fill(|p| sd_box(p, Vec2::new(64.0, 30.0), Vec2::new(64.0, 3.0), 0.0), [1.0; 4]);
    let moss = images.add(c.into_image(false));
    // screen vignette
    let mut c = Canvas::new(256, 144);
    c.map(|x, y, _| {
        let u = Vec2::new(x as f32 / 255.0 - 0.5, (y as f32 / 143.0 - 0.5) * 0.75);
        let d = (u.length() * 1.6).clamp(0.0, 1.0);
        [0.0, 0.0, 0.0, d.powf(2.2) * 0.85]
    });
    let vignette = images.add(c.into_image(false));
    // vertical fog gradient (dense at the bottom)
    let mut c = Canvas::new(4, 128);
    c.map(|_, y, _| [1.0, 1.0, 1.0, (y as f32 / 127.0).powf(1.8)]);
    let fog = images.add(c.into_image(false));

    commands.insert_resource(Art {
        player: seq("player", 8),
        bow: one("bow"),
        arrows: seq("arrows", 6),
        icons: seq("icons", 10),
        crawler: seq("crawler", 4),
        hopper: seq("hopper", 4),
        moth: seq("moth", 4),
        spitter: seq("spitter", 4),
        shieldbearer: seq("shieldbearer", 4),
        warden: seq("warden", 4),
        props: seq("props", 8),
        exit: one("exit"),
        platforms: seq("platforms", 3),
        tiles: seq("tiles", 8),
        tex_stone: one("tex_stone"),
        tex_wood: one("tex_wood"),
        tex_ice: one("tile_ice"),
        title: one("title"),
        font_title: server.load("fonts/Cinzel.ttf"),
        font_body: server.load("fonts/Cormorant.ttf"),
        glow,
        dot,
        pixel,
        rim,
        moss,
        vignette,
        fog,
    });
}

/// Size of a loaded image in pixels (zero until it's loaded).
pub fn img_size(images: &Assets<Image>, h: &Handle<Image>) -> Vec2 {
    images.get(h).map(|i| i.size_f32()).unwrap_or(Vec2::ZERO)
}
