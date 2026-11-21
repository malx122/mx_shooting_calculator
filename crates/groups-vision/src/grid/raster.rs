//! Single-channel pictures for the grid search: the reduced region, its
//! darkness against the paper, and sampling between pixels.

use super::InkImage;

/// A single-channel picture of floats, row by row. Pixel `(x, y)` covers
/// `[x, x + 1) × [y, y + 1)`, so its centre is at `(x + 0.5, y + 0.5)`.
#[derive(Clone, Debug)]
pub(super) struct Plane {
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
}

impl Plane {
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.width + x]
    }

    /// The value at a point between pixel centres (bilinear), or `None`
    /// within half a pixel of the edge.
    pub fn sample(&self, x: f64, y: f64) -> Option<f32> {
        let fx = x - 0.5;
        let fy = y - 0.5;
        if !(fx >= 0.0 && fy >= 0.0) {
            return None;
        }
        let (ix, iy) = (fx as usize, fy as usize);
        if ix + 1 >= self.width || iy + 1 >= self.height {
            return None;
        }
        let ax = (fx - ix as f64) as f32;
        let ay = (fy - iy as f64) as f32;
        let i = iy * self.width + ix;
        let v = &self.values;
        let top = v[i] + ax * (v[i + 1] - v[i]);
        let bottom = v[i + self.width] + ax * (v[i + self.width + 1] - v[i + self.width]);
        Some(top + ay * (bottom - top))
    }
}

/// The brightness of `image` over `[x0, x1) × [y0, y1)`, each output pixel
/// the mean of `factor` × `factor` input pixels (a remainder at the right
/// and bottom is left out).
pub(super) fn reduce(image: &InkImage, [x0, y0, x1, y1]: [usize; 4], factor: usize) -> Plane {
    let width = (x1 - x0) / factor;
    let height = (y1 - y0) / factor;
    let stride = image.width as usize;
    let mut values = Vec::with_capacity(width * height);
    let mut row = vec![0u32; width];
    let norm = 1.0 / (factor * factor) as f32;
    for out_y in 0..height {
        row.iter_mut().for_each(|sum| *sum = 0);
        for y in y0 + out_y * factor..y0 + (out_y + 1) * factor {
            let line = &image.pixels[y * stride + x0..y * stride + x0 + width * factor];
            for (sum, block) in row.iter_mut().zip(line.chunks_exact(factor)) {
                *sum += block.iter().map(|&v| u32::from(v)).sum::<u32>();
            }
        }
        values.extend(row.iter().map(|&sum| sum as f32 * norm));
    }
    Plane {
        width,
        height,
        values,
    }
}

/// The largest value within `radius` along rows, then along columns.
fn max_filter(plane: &Plane, radius: usize) -> Plane {
    let (w, h) = (plane.width, plane.height);
    let mut across = vec![0.0f32; w * h];
    for y in 0..h {
        let row = &plane.values[y * w..(y + 1) * w];
        for x in 0..w {
            let lo = x.saturating_sub(radius);
            let hi = (x + radius + 1).min(w);
            across[y * w + x] = row[lo..hi].iter().copied().fold(f32::MIN, f32::max);
        }
    }
    let mut values = vec![0.0f32; w * h];
    for y in 0..h {
        let lo = y.saturating_sub(radius);
        let hi = (y + radius + 1).min(h);
        for x in 0..w {
            values[y * w + x] = (lo..hi)
                .map(|yy| across[yy * w + x])
                .fold(f32::MIN, f32::max);
        }
    }
    Plane {
        width: w,
        height: h,
        values,
    }
}

/// The mean within `radius` along a line of `count` values `step` apart.
fn box_line(
    values: &mut [f32],
    start: usize,
    step: usize,
    count: usize,
    radius: usize,
    buffer: &mut Vec<f32>,
) {
    buffer.clear();
    buffer.extend((0..count).map(|i| values[start + i * step]));
    let mut sum: f64 = buffer[..radius.min(count)]
        .iter()
        .map(|&v| f64::from(v))
        .sum();
    for i in 0..count {
        if i + radius < count {
            sum += f64::from(buffer[i + radius]);
        }
        if i > radius {
            sum -= f64::from(buffer[i - radius - 1]);
        }
        let lo = i.saturating_sub(radius);
        let hi = (i + radius + 1).min(count);
        values[start + i * step] = (sum / (hi - lo) as f64) as f32;
    }
}

fn box_blur(mut plane: Plane, radius: usize) -> Plane {
    let (w, h) = (plane.width, plane.height);
    let mut buffer = Vec::new();
    for y in 0..h {
        box_line(&mut plane.values, y * w, 1, w, radius, &mut buffer);
    }
    for x in 0..w {
        box_line(&mut plane.values, x, w, h, radius, &mut buffer);
    }
    plane
}

/// How much darker than the paper round it each pixel is, from 0 (paper)
/// to 1 (black). The paper's brightness is the local maximum, which drops
/// thin lines, smoothed over a wide area, so shadows and uneven light
/// cancel out.
pub(super) fn darkness(bright: &Plane) -> Plane {
    let long = bright.width.max(bright.height);
    let paper = box_blur(max_filter(bright, 3), (long / 24).max(8));
    let values = bright
        .values
        .iter()
        .zip(&paper.values)
        .map(|(&value, &paper)| ((paper - value) / paper.max(12.0)).clamp(0.0, 1.0))
        .collect();
    Plane {
        width: bright.width,
        height: bright.height,
        values,
    }
}

/// The pixel noise of a plane, one standard deviation: from the median
/// difference between neighbours, which lines and edges hardly move.
pub(super) fn noise(plane: &Plane) -> f32 {
    let mut differences: Vec<f32> = (0..plane.height)
        .step_by(3)
        .flat_map(|y| {
            (1..plane.width)
                .step_by(3)
                .map(move |x| (plane.get(x, y) - plane.get(x - 1, y)).abs())
        })
        .collect();
    if differences.is_empty() {
        return 0.0;
    }
    let middle = differences.len() / 2;
    let (_, median, _) = differences.select_nth_unstable_by(middle, f32::total_cmp);
    1.4826 * *median / std::f32::consts::SQRT_2
}
