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
pub fn sd_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.length_squared()).clamp(0.0, 1.0);
    (pa - ba * h).length()
}
pub fn sd_box(p: Vec2, c: Vec2, half: Vec2, round: f32) -> f32 {
    let d = (p - c).abs() - half + Vec2::splat(round);
    d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0) - round
}
// ---------- hashing ----------

pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xffffff) as f32 / 16777215.0
}
