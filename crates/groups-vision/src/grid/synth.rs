//! Synthetic photos of grid paper with a known geometry, for the tests:
//! a grid drawn through a homography, with paper texture, uneven light and
//! a shadow, scoring rings, a black centre and bullet holes, lens blur,
//! sensor noise and a JPEG round trip.

use super::InkImage;
use super::model::{Matrix, apply};

fn invert(m: &Matrix) -> Matrix {
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let adj = [
        [c(1, 2, 1, 2), -c(0, 2, 1, 2), c(0, 1, 1, 2)],
        [-c(1, 2, 0, 2), c(0, 2, 0, 2), -c(0, 1, 0, 2)],
        [c(1, 2, 0, 1), -c(0, 2, 0, 1), c(0, 1, 0, 1)],
    ];
    let det = m[0][0] * adj[0][0] + m[0][1] * adj[1][0] + m[0][2] * adj[2][0];
    adj.map(|row| row.map(|v| v / det))
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

/// How the camera saw the target.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub width: u32,
    pub height: u32,
    /// In the middle of the photo.
    pub px_per_mm: f64,
    /// The grid's turn, degrees clockwise on screen.
    pub rotation_deg: f64,
    /// Perspective, per px from the middle: how fast the scale falls
    /// off along x and y.
    pub tilt: [f64; 2],
    /// The grid point (mm) in the middle of the photo.
    pub centre_mm: [f64; 2],
}

impl Camera {
    /// Grid mm (y down, as the photo) to photo px.
    pub fn to_px(self) -> Matrix {
        let (sin, cos) = self.rotation_deg.to_radians().sin_cos();
        let s = self.px_per_mm;
        let shift_mm = [
            [1.0, 0.0, -self.centre_mm[0]],
            [0.0, 1.0, -self.centre_mm[1]],
            [0.0, 0.0, 1.0],
        ];
        let turn = [
            [s * cos, -s * sin, 0.0],
            [s * sin, s * cos, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let tilt = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [self.tilt[0], self.tilt[1], 1.0],
        ];
        let middle = [
            [1.0, 0.0, 0.5 * self.width as f64],
            [0.0, 1.0, 0.5 * self.height as f64],
            [0.0, 0.0, 1.0],
        ];
        multiply(&middle, &multiply(&tilt, &multiply(&turn, &shift_mm)))
    }
}

/// A sheet of grid paper and what is on it.
#[derive(Clone, Debug)]
pub struct Scene {
    pub camera: Camera,
    pub square_mm: f64,
    /// Heavier lines every this many squares; 0 for none.
    pub major_every: u32,
    pub line_mm: f64,
    pub major_line_mm: f64,
    /// Each line's width varies by up to this share.
    pub width_jitter: f64,
    pub paper: [f32; 3],
    pub ink: [f32; 3],
    pub major_ink: [f32; 3],
    /// Scoring rings: centre (mm), radius (mm), line width (mm).
    pub rings: Vec<([f64; 2], f64, f64)>,
    /// A filled black centre: centre and radius, mm.
    pub bullseye: Option<([f64; 2], f64)>,
    /// Bullet holes: centre and radius, mm.
    pub holes: Vec<([f64; 2], f64)>,
    /// Light falling off across the photo, as a share.
    pub light_gradient: f64,
    /// A soft shadow over part of the photo: how much darker.
    pub shadow: f64,
    pub texture: f64,
    pub blur_px: f64,
    /// Sensor noise, one standard deviation, 0–255.
    pub noise: f64,
    pub jpeg_quality: Option<u8>,
    pub seed: u64,
    /// Paper curling away on the right: the share of the photo's width the
    /// right edge is squeezed by (not a homography).
    pub curl: f64,
    /// A photographed screen over the picture: pixel pitch, px.
    pub screen_px: Option<f64>,
    /// Which line families are printed (across, down).
    pub families: [bool; 2],
}

impl Scene {
    /// A plain black grid of `square_mm`, seen square-on.
    pub fn plain(width: u32, height: u32, px_per_mm: f64, square_mm: f64) -> Scene {
        Scene {
            camera: Camera {
                width,
                height,
                px_per_mm,
                rotation_deg: 0.0,
                tilt: [0.0, 0.0],
                centre_mm: [0.37 * square_mm, 0.61 * square_mm],
            },
            square_mm,
            major_every: 0,
            line_mm: 0.25,
            major_line_mm: 0.6,
            width_jitter: 0.0,
            paper: [240.0, 238.0, 230.0],
            ink: [30.0, 30.0, 35.0],
            major_ink: [20.0, 20.0, 25.0],
            rings: Vec::new(),
            bullseye: None,
            holes: Vec::new(),
            light_gradient: 0.0,
            shadow: 0.0,
            texture: 0.0,
            blur_px: 0.6,
            noise: 1.0,
            jpeg_quality: None,
            seed: 1,
            curl: 0.0,
            screen_px: None,
            families: [true, true],
        }
    }

    /// The same with the usual troubles of a phone photo.
    pub fn realistic(mut self) -> Scene {
        self.width_jitter = 0.2;
        self.light_gradient = 0.3;
        self.shadow = 0.35;
        self.texture = 0.04;
        self.blur_px = 1.0;
        self.noise = 3.0;
        self.jpeg_quality = Some(80);
        self
    }

    /// Where crossing `(i, j)` of the squares (major ones when `major`) is.
    pub fn crossing(&self, i: f64, j: f64) -> [f64; 2] {
        let step = self.square_mm;
        apply(&self.camera.to_px(), i * step, j * step).expect("in front of the camera")
    }

    /// The grid coordinates (squares) of photo point `p`.
    pub fn grid_at(&self, p: [f64; 2]) -> [f64; 2] {
        let x = self.curled(p[0]).0;
        let mm = apply(&invert(&self.camera.to_px()), x, p[1]).expect("on the sheet");
        [mm[0] / self.square_mm, mm[1] / self.square_mm]
    }

    /// Where photo column `x` shows the flat sheet, and how fast that moves.
    fn curled(&self, x: f64) -> (f64, f64) {
        let w = f64::from(self.camera.width);
        let t = ((x / w - 0.5) * 2.0).max(0.0);
        (
            x + self.curl * w * t * t * t / 3.0,
            1.0 + 2.0 * self.curl * t * t,
        )
    }

    pub fn render(&self) -> (image::RgbImage, InkImage) {
        let (w, h) = (self.camera.width as usize, self.camera.height as usize);
        let to_mm = invert(&self.camera.to_px());
        let mut random = Random(self.seed);
        let jitter: Vec<f64> = (0..4096)
            .map(|_| 1.0 + self.width_jitter * (2.0 * random.next() - 1.0))
            .collect();
        let lattice = 24usize;
        let (lw, lh) = (w / lattice + 2, h / lattice + 2);
        let grain: Vec<f64> = (0..lw * lh).map(|_| random.next() - 0.5).collect();
        let shadow_angle = 0.7 + random.next();
        let shadow_at = 0.55 * w as f64;
        let mut planes = vec![vec![0.0f32; w * h]; 3];
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let (flat, stretch) = self.curled(px);
                let (raw_x, px) = (px, flat);
                let d = to_mm[2][0] * px + to_mm[2][1] * py + to_mm[2][2];
                let qx = (to_mm[0][0] * px + to_mm[0][1] * py + to_mm[0][2]) / d;
                let qy = (to_mm[1][0] * px + to_mm[1][1] * py + to_mm[1][2]) / d;
                // mm per px along each grid axis's gradient.
                let gradient = |row: usize, q: f64| {
                    let gx = (to_mm[row][0] - q * to_mm[2][0]) / d * stretch;
                    let gy = (to_mm[row][1] - q * to_mm[2][1]) / d;
                    gx.hypot(gy)
                };
                let mm_per_px = [gradient(0, qx), gradient(1, qy)];
                let mut colour = self.paper;
                for (axis, q) in [qx, qy].into_iter().enumerate() {
                    if !self.families[axis] {
                        continue;
                    }
                    let line = (q / self.square_mm).round();
                    let index = line as i64;
                    let major =
                        self.major_every > 0 && index.rem_euclid(i64::from(self.major_every)) == 0;
                    let width_mm = if major {
                        self.major_line_mm
                    } else {
                        self.line_mm
                    } * jitter[(index.rem_euclid(2048) as usize) * 2 + axis];
                    let distance = (q - line * self.square_mm).abs() / mm_per_px[axis];
                    let width = width_mm / mm_per_px[axis];
                    let cover = ((distance + 0.5).min(0.5 * width)
                        - (distance - 0.5).max(-0.5 * width))
                    .clamp(0.0, 1.0) as f32;
                    let ink = if major { self.major_ink } else { self.ink };
                    for c in 0..3 {
                        colour[c] += cover * (ink[c] - colour[c]);
                    }
                }
                let scale = 0.5 * (mm_per_px[0] + mm_per_px[1]);
                let black = [15.0f32, 15.0, 18.0];
                for &(centre, radius, width) in &self.rings {
                    let r = (qx - centre[0]).hypot(qy - centre[1]);
                    let distance = (r - radius).abs() / scale;
                    let width = width / scale;
                    let cover = ((distance + 0.5).min(0.5 * width)
                        - (distance - 0.5).max(-0.5 * width))
                    .clamp(0.0, 1.0) as f32;
                    for c in 0..3 {
                        colour[c] += cover * (black[c] - colour[c]);
                    }
                }
                if let Some((centre, radius)) = self.bullseye {
                    let r = (qx - centre[0]).hypot(qy - centre[1]);
                    let cover = ((radius - r) / scale + 0.5).clamp(0.0, 1.0) as f32;
                    for c in 0..3 {
                        colour[c] += cover * (black[c] - colour[c]);
                    }
                }
                for &(centre, radius) in &self.holes {
                    let r = (qx - centre[0]).hypot(qy - centre[1]);
                    let cover = ((radius - r) / scale + 0.5).clamp(0.0, 1.0) as f32;
                    let rim = ((radius * 1.25 - r) / scale + 0.5).clamp(0.0, 1.0) as f32 * 0.5;
                    let hole = [35.0f32, 30.0, 28.0];
                    let torn = [200.0f32, 195.0, 185.0];
                    for c in 0..3 {
                        colour[c] += rim * (1.0 - cover) * (torn[c] - colour[c]);
                        colour[c] += cover * (hole[c] - colour[c]);
                    }
                }
                let (fx, fy) = (px / lattice as f64, py / lattice as f64);
                let (ix, iy) = (fx as usize, fy as usize);
                let (ax, ay) = (fx - ix as f64, fy - iy as f64);
                let g = |i: usize, j: usize| grain[j * lw + i];
                let texture = (1.0 - ay) * ((1.0 - ax) * g(ix, iy) + ax * g(ix + 1, iy))
                    + ay * ((1.0 - ax) * g(ix, iy + 1) + ax * g(ix + 1, iy + 1));
                let across = px / w as f64 - 0.5;
                let side = (px - shadow_at) * shadow_angle.cos()
                    + (py - 0.5 * h as f64) * shadow_angle.sin();
                let shade = 1.0 - self.shadow / (1.0 + (-side / 30.0).exp());
                let light = ((1.0 - self.light_gradient * across)
                    * shade
                    * (1.0 + 2.0 * self.texture * texture)) as f32;
                for c in 0..3 {
                    let screen = self.screen_px.map_or(1.0, |pitch| {
                        let stripe = (raw_x / pitch).fract();
                        let gap = if (py / pitch).fract() < 0.2 { 0.6 } else { 1.0 };
                        let lit = (c as f64 / 3.0..(c + 1) as f64 / 3.0).contains(&stripe);
                        gap * if lit { 1.0 } else { 0.35 }
                    }) as f32;
                    planes[c][y * w + x] = colour[c] * light * screen;
                }
            }
        }
        if self.blur_px > 0.0 {
            for plane in &mut planes {
                gaussian(plane, w, h, self.blur_px);
            }
        }
        let mut rgb = Vec::with_capacity(w * h * 3);
        for i in 0..w * h {
            for plane in &planes {
                let value = f64::from(plane[i]) + self.noise * random.gaussian();
                rgb.push(value.round().clamp(0.0, 255.0) as u8);
            }
        }
        let mut picture = image::RgbImage::from_raw(w as u32, h as u32, rgb).expect("sized");
        if let Some(quality) = self.jpeg_quality {
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode_image(&picture)
                .expect("encodes");
            picture = image::load_from_memory(&jpeg).expect("decodes").into_rgb8();
        }
        let ink = InkImage::from_rgb(picture.width(), picture.height(), picture.as_raw());
        (picture, ink)
    }
}

fn gaussian(plane: &mut [f32], w: usize, h: usize, sigma: f64) {
    let radius = (3.0 * sigma).ceil() as i64;
    let kernel: Vec<f32> = (-radius..=radius)
        .map(|k| (-(k * k) as f64 / (2.0 * sigma * sigma)).exp() as f32)
        .collect();
    let total: f32 = kernel.iter().sum();
    let kernel: Vec<f32> = kernel.iter().map(|k| k / total).collect();
    let mut line = Vec::new();
    let mut pass = |plane: &mut [f32], start: usize, step: usize, count: usize| {
        line.clear();
        line.extend((0..count).map(|i| plane[start + i * step]));
        for i in 0..count {
            let mut sum = 0.0;
            for (k, weight) in kernel.iter().enumerate() {
                let j = (i as i64 + k as i64 - radius).clamp(0, count as i64 - 1) as usize;
                sum += weight * line[j];
            }
            plane[start + i * step] = sum;
        }
    };
    for y in 0..h {
        pass(plane, y * w, 1, w);
    }
    for x in 0..w {
        pass(plane, x, w, h);
    }
}

/// SplitMix64.
struct Random(u64);

impl Random {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn next(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn gaussian(&mut self) -> f64 {
        let u = self.next().max(1e-12);
        let v = self.next();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}
