//! Tiny anti-aliased SDF rasterizer used to paint every procedural sprite at startup.
//! Coordinates are in pixels, origin top-left, y down (like an image).

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

#[derive(Clone)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    /// premultiplied linear-ish rgba (we stay in sRGB space for simplicity)
    px: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[0.0; 4]; w * h] }
    }

    #[inline]
    pub fn blend(&mut self, x: usize, y: usize, c: [f32; 4], cov: f32) {
        let a = c[3] * cov;
        if a <= 0.0 {
            return;
        }
        let p = &mut self.px[y * self.w + x];
        let inv = 1.0 - a;
        p[0] = c[0] * a + p[0] * inv;
        p[1] = c[1] * a + p[1] * inv;
        p[2] = c[2] * a + p[2] * inv;
        p[3] = a + p[3] * inv;
    }

    /// Fill everything where `sdf(p) < 0`, anti-aliased over one pixel. `color(p)` may vary per pixel.
    pub fn fill_with(&mut self, sdf: impl Fn(Vec2) -> f32, color: impl Fn(Vec2) -> [f32; 4]) {
        for y in 0..self.h {
            for x in 0..self.w {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let d = sdf(p);
                if d < 0.75 {
                    let cov = (0.5 - d).clamp(0.0, 1.0);
                    self.blend(x, y, color(p), cov);
                }
            }
        }
    }

    pub fn fill(&mut self, sdf: impl Fn(Vec2) -> f32, color: [f32; 4]) {
        self.fill_with(sdf, |_| color);
    }

    /// Soft glow: alpha falls off with distance outside the shape.
    pub fn glow(&mut self, sdf: impl Fn(Vec2) -> f32, color: [f32; 4], radius: f32) {
        for y in 0..self.h {
            for x in 0..self.w {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let d = sdf(p).max(0.0);
                if d < radius {
                    let t = 1.0 - d / radius;
                    self.blend(x, y, color, t * t);
                }
            }
        }
    }

    pub fn map(&mut self, f: impl Fn(usize, usize, [f32; 4]) -> [f32; 4]) {
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                self.px[i] = f(x, y, self.px[i]);
            }
        }
    }

    pub fn get(&self, x: usize, y: usize) -> [f32; 4] {
        self.px[y * self.w + x]
    }

    pub fn into_image(self, nearest: bool) -> Image {
        let mut data = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            let a = p[3].clamp(0.0, 1.0);
            let un = |v: f32| if a > 0.0 { (v / a).clamp(0.0, 1.0) } else { 0.0 };
            data.push((un(p[0]) * 255.0 + 0.5) as u8);
            data.push((un(p[1]) * 255.0 + 0.5) as u8);
            data.push((un(p[2]) * 255.0 + 0.5) as u8);
            data.push((a * 255.0 + 0.5) as u8);
        }
        let mut img = Image::new(
            Extent3d { width: self.w as u32, height: self.h as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        img.sampler = if nearest {
            ImageSampler::nearest()
        } else {
            ImageSampler::Descriptor(ImageSamplerDescriptor::linear())
        };
        img
    }
}

// ---------- signed distance helpers ----------

pub fn sd_circle(p: Vec2, c: Vec2, r: f32) -> f32 {
    (p - c).length() - r
}

pub fn sd_ellipse(p: Vec2, c: Vec2, r: Vec2) -> f32 {
    // cheap approximation, good enough for sprites
    let q = (p - c) / r;
    (q.length() - 1.0) * r.min_element()
}

pub fn sd_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.length_squared()).clamp(0.0, 1.0);
    (pa - ba * h).length()
}

pub fn sd_capsule(p: Vec2, a: Vec2, b: Vec2, r: f32) -> f32 {
    sd_segment(p, a, b) - r
}

pub fn sd_box(p: Vec2, c: Vec2, half: Vec2, round: f32) -> f32 {
    let d = (p - c).abs() - half + Vec2::splat(round);
    d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0) - round
}

/// Exact SDF of an arbitrary simple polygon.
pub fn sd_poly(p: Vec2, v: &[Vec2]) -> f32 {
    let n = v.len();
    let mut d = (p - v[0]).length_squared();
    let mut s = 1.0;
    let mut j = n - 1;
    for i in 0..n {
        let e = v[j] - v[i];
        let w = p - v[i];
        let b = w - e * (w.dot(e) / e.length_squared()).clamp(0.0, 1.0);
        d = d.min(b.length_squared());
        let c1 = p.y >= v[i].y;
        let c2 = p.y < v[j].y;
        let c3 = e.x * w.y > e.y * w.x;
        if (c1 && c2 && c3) || (!c1 && !c2 && !c3) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

/// Arc stroke (ring section) centred at c, radius r, thickness t, angles in radians (y down).
pub fn sd_arc(p: Vec2, c: Vec2, r: f32, t: f32, a0: f32, a1: f32) -> f32 {
    let q = p - c;
    let ang = q.y.atan2(q.x);
    if ang >= a0 && ang <= a1 {
        (q.length() - r).abs() - t
    } else {
        let e0 = c + Vec2::new(a0.cos(), a0.sin()) * r;
        let e1 = c + Vec2::new(a1.cos(), a1.sin()) * r;
        (p - e0).length().min((p - e1).length()) - t
    }
}

pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

// ---------- value noise ----------

pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xffffff) as f32 / 16777215.0
}

pub fn vnoise(p: Vec2, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let (ix, iy) = (i.x as i32, i.y as i32);
    let a = hash2(ix, iy, seed);
    let b = hash2(ix + 1, iy, seed);
    let c = hash2(ix, iy + 1, seed);
    let d = hash2(ix + 1, iy + 1, seed);
    a + (b - a) * u.x + (c - a) * u.y + (a - b - c + d) * u.x * u.y
}

/// Tileable value noise with period `per` (in noise cells).
pub fn vnoise_tile(p: Vec2, per: i32, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let w = |v: i32| v.rem_euclid(per);
    let (ix, iy) = (i.x as i32, i.y as i32);
    let a = hash2(w(ix), w(iy), seed);
    let b = hash2(w(ix + 1), w(iy), seed);
    let c = hash2(w(ix), w(iy + 1), seed);
    let d = hash2(w(ix + 1), w(iy + 1), seed);
    a + (b - a) * u.x + (c - a) * u.y + (a - b - c + d) * u.x * u.y
}

pub fn fbm(p: Vec2, oct: u32, seed: u32) -> f32 {
    let mut s = 0.0;
    let mut a = 0.5;
    let mut q = p;
    for o in 0..oct {
        s += a * vnoise(q, seed + o * 17);
        q *= 2.03;
        a *= 0.5;
    }
    s
}

pub fn rgba(hex: u32, a: f32) -> [f32; 4] {
    [
        ((hex >> 16) & 255) as f32 / 255.0,
        ((hex >> 8) & 255) as f32 / 255.0,
        (hex & 255) as f32 / 255.0,
        a,
    ]
}

pub fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
