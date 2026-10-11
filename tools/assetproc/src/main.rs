//! Turns raw generated art (art/raw/*.png) into game-ready assets (assets/).
//!
//!   cargo run -p assetproc --release            # process everything
//!   cargo run -p assetproc --release -- player  # just these
//!
//! - sprites/layers: magenta background keyed to alpha, despilled, trimmed, resized
//! - sheets: keyed, split into frames by connected components (falls back to the grid),
//!   ordered row by row, then written as `name_N.png` (animated sheets share one cell size,
//!   feet on the bottom edge) plus `name.json` with the frame count
//! - textures / backgrounds: resized only

use image::{Rgba, RgbaImage, imageops};
use serde_json::Value;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

struct Opts {
    /// resize so the cell (or sprite) is at most this tall
    max_h: u32,
    /// ... and at most this wide
    max_w: u32,
    /// animated: all frames share one cell, bottom-aligned
    uniform: bool,
    bottom: bool,
}

fn opts(name: &str) -> Opts {
    let o = |max_w, max_h, uniform, bottom| Opts { max_w, max_h, uniform, bottom };
    match name {
        "player" => o(256, 160, true, true),
        "crawler" | "hopper" | "spitter" | "shieldbearer" => o(256, 140, true, true),
        "moth" => o(256, 140, true, false),
        "warden" => o(512, 360, true, true),
        "arrows" => o(200, 64, false, false),
        "icons" => o(128, 128, false, false),
        "props" => o(256, 192, false, true),
        "platforms" => o(384, 128, false, false),
        "tiles" => o(128, 128, false, false),
        "bow" => o(160, 256, false, false),
        "exit" => o(320, 400, false, true),
        _ => o(512, 512, false, false),
    }
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(root.join("tools/assets.json")).unwrap()).unwrap();
    let only: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = root.join("assets/art");
    fs::create_dir_all(&out_dir).unwrap();
    for a in manifest["assets"].as_array().unwrap() {
        let name = a["name"].as_str().unwrap();
        if !only.is_empty() && !only.iter().any(|n| n == name) {
            continue;
        }
        let raw = root.join(format!("art/raw/{name}.png"));
        if !raw.exists() {
            println!("skip    {name} (no raw image)");
            continue;
        }
        let img = image::ImageReader::open(&raw).unwrap().with_guessed_format().unwrap().decode().unwrap().to_rgba8();
        let kind = a["kind"].as_str().unwrap();
        match kind {
            "sprite" | "layer" => {
                let mut k = key(&img);
                if kind == "sprite" {
                    k = trim(&k, 2);
                    let o = opts(name);
                    k = fit(&k, o.max_w, o.max_h);
                } else {
                    k = fit(&k, 1600, 1600);
                }
                save(&k, &out_dir.join(format!("{name}.png")));
                println!("sprite  {name} {}x{}", k.width(), k.height());
            }
            "texture" => {
                let t = imageops::resize(&img, 512, 512, imageops::FilterType::Lanczos3);
                save(&t, &out_dir.join(format!("{name}.png")));
                println!("texture {name}");
            }
            "bg" => {
                let w = 1600;
                let h = img.height() * w / img.width();
                let t = imageops::resize(&img, w, h, imageops::FilterType::Lanczos3);
                save(&t, &out_dir.join(format!("{name}.png")));
                println!("bg      {name} {w}x{h}");
            }
            "sheet" => {
                let frames = a["frames"].as_u64().unwrap() as usize;
                let cols = a["cols"].as_u64().unwrap() as usize;
                let rows = a["rows"].as_u64().unwrap() as usize;
                sheet(name, &img, frames, cols, rows, &out_dir);
            }
            other => panic!("unknown kind {other}"),
        }
    }
}

fn save(img: &RgbaImage, p: &Path) {
    img.save(p).unwrap();
}

/// Background colour = median of the border pixels.
fn bg_color(img: &RgbaImage) -> [f32; 3] {
    let (w, h) = img.dimensions();
    let mut px: Vec<[u8; 3]> = Vec::new();
    for x in (0..w).step_by(3) {
        for y in [0, 1, h - 2, h - 1] {
            let p = img.get_pixel(x, y);
            px.push([p[0], p[1], p[2]]);
        }
    }
    for y in (0..h).step_by(3) {
        for x in [0, 1, w - 2, w - 1] {
            let p = img.get_pixel(x, y);
            px.push([p[0], p[1], p[2]]);
        }
    }
    let mut c = [0.0; 3];
    for (i, ci) in c.iter_mut().enumerate() {
        let mut v: Vec<u8> = px.iter().map(|p| p[i]).collect();
        v.sort();
        *ci = v[v.len() / 2] as f32;
    }
    c
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Magenta chroma key with despill.
fn key(img: &RgbaImage) -> RgbaImage {
    let bg = bg_color(img);
    let bg_m = bg[0].min(bg[2]) - bg[1];
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        // magenta-ness relative to the background's
        let m = (r.min(b) - g) / bg_m.max(1.0);
        let d = ((r - bg[0]).powi(2) + (g - bg[1]).powi(2) + (b - bg[2]).powi(2)).sqrt();
        let mut a = 1.0 - smoothstep(0.45, 0.75, m);
        if d < 45.0 {
            a = 0.0;
        }
        // despill: pull magenta excess out of semi-transparent edges
        let (mut r2, mut b2) = (r, b);
        if a < 0.999 {
            let spill = (r.min(b) - g).max(0.0);
            r2 = r - spill;
            b2 = b - spill;
            // un-mix the bg colour from edge pixels
            if a > 0.05 {
                let un = |c: f32, k: f32| ((c - k * (1.0 - a)) / a).clamp(0.0, 255.0);
                r2 = un(r, bg[0]).min(r2.max(g)).max(0.0);
                b2 = un(b, bg[2]).min(b2.max(g)).max(0.0);
            }
        }
        *p = Rgba([r2 as u8, g as u8, b2 as u8, (a * 255.0) as u8]);
    }
    out
}

fn bbox(img: &RgbaImage, thresh: u8) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > thresh {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x1 >= x0 && y1 >= y0).then_some((x0, y0, x1 + 1, y1 + 1))
}

fn trim(img: &RgbaImage, pad: u32) -> RgbaImage {
    let Some((x0, y0, x1, y1)) = bbox(img, 8) else { return img.clone() };
    let (w, h) = (x1 - x0 + pad * 2, y1 - y0 + pad * 2);
    let mut out = RgbaImage::new(w, h);
    imageops::overlay(&mut out, &imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image(), pad as i64, pad as i64);
    out
}

fn fit(img: &RgbaImage, max_w: u32, max_h: u32) -> RgbaImage {
    let s = (max_w as f32 / img.width() as f32).min(max_h as f32 / img.height() as f32).min(1.0);
    if s >= 1.0 {
        return img.clone();
    }
    let w = ((img.width() as f32 * s).round() as u32).max(1);
    let h = ((img.height() as f32 * s).round() as u32).max(1);
    imageops::resize(img, w, h, imageops::FilterType::Lanczos3)
}

struct Comp {
    pixels: Vec<(u32, u32)>,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Comp {
    fn center(&self) -> (f32, f32) {
        ((self.x0 + self.x1) as f32 / 2.0, (self.y0 + self.y1) as f32 / 2.0)
    }
    fn merge(&mut self, o: Comp) {
        self.x0 = self.x0.min(o.x0);
        self.y0 = self.y0.min(o.y0);
        self.x1 = self.x1.max(o.x1);
        self.y1 = self.y1.max(o.y1);
        self.pixels.extend(o.pixels);
    }
}

/// Connected components of the alpha mask, dilated by `r` so nearby fragments join.
fn components(img: &RgbaImage, r: i32) -> Vec<Comp> {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let solid: Vec<bool> = img.pixels().map(|p| p[3] > 40).collect();
    // dilate with a separable box (cheap and good enough)
    let mut tmp = vec![false; (w * h) as usize];
    for y in 0..h {
        let mut run = 0i32;
        for x in -r..w + r {
            if x + r < w && solid[(y * w + (x + r)) as usize] {
                run = 2 * r + 1;
            }
            if x >= 0 && x < w && run > 0 {
                tmp[(y * w + x) as usize] = true;
            }
            run -= 1;
        }
    }
    let mut dil = vec![false; (w * h) as usize];
    for x in 0..w {
        let mut run = 0i32;
        for y in -r..h + r {
            if y + r < h && tmp[((y + r) * w + x) as usize] {
                run = 2 * r + 1;
            }
            if y >= 0 && y < h && run > 0 {
                dil[(y * w + x) as usize] = true;
            }
            run -= 1;
        }
    }
    let mut label = vec![usize::MAX; (w * h) as usize];
    let mut comps = Vec::new();
    for start in 0..(w * h) as usize {
        if !dil[start] || label[start] != usize::MAX {
            continue;
        }
        let id = comps.len();
        let mut c = Comp { pixels: vec![], x0: u32::MAX, y0: u32::MAX, x1: 0, y1: 0 };
        let mut q = VecDeque::from([start]);
        label[start] = id;
        while let Some(i) = q.pop_front() {
            let (x, y) = ((i as i32 % w), (i as i32 / w));
            if solid[i] {
                c.pixels.push((x as u32, y as u32));
                c.x0 = c.x0.min(x as u32);
                c.y0 = c.y0.min(y as u32);
                c.x1 = c.x1.max(x as u32 + 1);
                c.y1 = c.y1.max(y as u32 + 1);
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let j = (ny * w + nx) as usize;
                if dil[j] && label[j] == usize::MAX {
                    label[j] = id;
                    q.push_back(j);
                }
            }
        }
        if !c.pixels.is_empty() {
            comps.push(c);
        }
    }
    comps
}

fn sheet(name: &str, raw: &RgbaImage, frames: usize, cols: usize, rows: usize, out_dir: &Path) {
    let img = key(raw);
    let (w, h) = img.dimensions();
    let r = (w.max(h) / 220).max(3) as i32;
    let mut comps = components(&img, r);
    let min_px = (w * h) as usize / 4000;
    comps.retain(|c| c.pixels.len() > min_px);
    comps.sort_by_key(|c| std::cmp::Reverse(c.pixels.len()));
    // merge the smallest fragments into their nearest big neighbour
    while comps.len() > frames {
        let small = comps.pop().unwrap();
        let (sx, sy) = small.center();
        let best = comps
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let da = (a.1.center().0 - sx).powi(2) + (a.1.center().1 - sy).powi(2);
                let db = (b.1.center().0 - sx).powi(2) + (b.1.center().1 - sy).powi(2);
                da.partial_cmp(&db).unwrap()
            })
            .unwrap()
            .0;
        comps[best].merge(small);
    }
    let method;
    let mut cells: Vec<RgbaImage> = if comps.len() == frames {
        method = "components";
        // reading order: sort by y into rows, then by x
        comps.sort_by(|a, b| a.center().1.partial_cmp(&b.center().1).unwrap());
        let mut ordered = Vec::new();
        for row in comps.chunks_mut(cols) {
            row.sort_by(|a, b| a.center().0.partial_cmp(&b.center().0).unwrap());
            for c in row.iter() {
                let mut cell = RgbaImage::new(c.x1 - c.x0, c.y1 - c.y0);
                for &(x, y) in &c.pixels {
                    cell.put_pixel(x - c.x0, y - c.y0, *img.get_pixel(x, y));
                }
                ordered.push(cell);
            }
        }
        let _ = rows;
        ordered
    } else {
        method = "grid";
        let (cw, ch) = (w / cols as u32, h / rows as u32);
        (0..frames)
            .map(|i| {
                let (cx, cy) = ((i % cols) as u32 * cw, (i / cols) as u32 * ch);
                trim(&imageops::crop_imm(&img, cx, cy, cw, ch).to_image(), 0)
            })
            .collect()
    };
    let o = opts(name);
    if o.uniform {
        // anchor: alpha-weighted centroid x, bbox bottom
        let anchors: Vec<f32> = cells
            .iter()
            .map(|c| {
                let (mut sx, mut sa) = (0.0, 0.0);
                for (x, _, p) in c.enumerate_pixels() {
                    sx += x as f32 * p[3] as f32;
                    sa += p[3] as f32;
                }
                sx / sa.max(1.0)
            })
            .collect();
        let left = cells.iter().zip(&anchors).map(|(_, a)| a.ceil() as u32).max().unwrap();
        let right = cells.iter().zip(&anchors).map(|(c, a)| c.width() - a.floor() as u32).max().unwrap();
        let ch = cells.iter().map(|c| c.height()).max().unwrap();
        let cw = left + right;
        let mut uni = Vec::new();
        for (c, a) in cells.iter().zip(&anchors) {
            let mut cell = RgbaImage::new(cw, ch);
            let x = left as i64 - a.round() as i64;
            let y = if o.bottom { (ch - c.height()) as i64 } else { ((ch - c.height()) / 2) as i64 };
            imageops::overlay(&mut cell, c, x, y);
            uni.push(fit(&cell, o.max_w, o.max_h));
        }
        cells = uni;
    } else {
        cells = cells.iter().map(|c| fit(c, o.max_w, o.max_h)).collect();
    }
    for (i, c) in cells.iter().enumerate() {
        save(c, &out_dir.join(format!("{name}_{i}.png")));
    }
    let sizes: Vec<String> = cells.iter().map(|c| format!("{}x{}", c.width(), c.height())).collect();
    println!("sheet   {name} {} frames via {method}: {}", cells.len(), sizes.join(" "));
}
