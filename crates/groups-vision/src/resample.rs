//! Lanczos-3 resampling, row by row: besides the source and the result it
//! holds only the few rows the filter spans, so reducing a large photo
//! needs little memory beyond the photo itself.

use std::collections::VecDeque;
use std::f64::consts::PI;

use image::RgbImage;

const LOBES: f64 = 3.0;

/// The source samples one output sample is made of: from `start`, one
/// weight each.
struct Taps {
    start: usize,
    weights: Vec<f32>,
}

fn lanczos(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else if x.abs() >= LOBES {
        0.0
    } else {
        let angle = PI * x;
        LOBES * angle.sin() * (angle / LOBES).sin() / (angle * angle)
    }
}

/// The taps for each of `target` samples taken from `source`, pixel
/// centres aligned; when reducing, the filter widens to cover every source
/// pixel.
fn taps(source: u32, target: u32) -> Vec<Taps> {
    let scale = f64::from(source) / f64::from(target);
    let stretch = scale.max(1.0);
    let support = LOBES * stretch;
    (0..target)
        .map(|index| {
            let centre = (f64::from(index) + 0.5) * scale;
            let start = (centre - support).floor().max(0.0) as usize;
            let end = ((centre + support).ceil() as usize).min(source as usize);
            let weights: Vec<f64> = (start..end)
                .map(|sample| lanczos((sample as f64 + 0.5 - centre) / stretch))
                .collect();
            let sum: f64 = weights.iter().sum();
            Taps {
                start,
                weights: weights.iter().map(|weight| (weight / sum) as f32).collect(),
            }
        })
        .collect()
}

/// `source` resampled to `width` × `height`.
pub(crate) fn resize(source: &RgbImage, width: u32, height: u32) -> RgbImage {
    let columns = taps(source.width(), width);
    let rows = taps(source.height(), height);
    let source_row = source.width() as usize * 3;
    let target_row = width as usize * 3;
    let raw = source.as_raw();
    let across = |y: usize| -> Vec<f32> {
        let line = &raw[y * source_row..(y + 1) * source_row];
        let mut out = Vec::with_capacity(target_row);
        for tap in &columns {
            let mut sum = [0.0f32; 3];
            for (offset, weight) in tap.weights.iter().enumerate() {
                let pixel = &line[(tap.start + offset) * 3..][..3];
                for (total, value) in sum.iter_mut().zip(pixel) {
                    *total += weight * f32::from(*value);
                }
            }
            out.extend_from_slice(&sum);
        }
        out
    };
    // The rows done across, from source row `first`.
    let mut done: VecDeque<Vec<f32>> = VecDeque::new();
    let mut first = 0;
    let mut out = Vec::with_capacity(target_row * height as usize);
    let mut sum = vec![0.0f32; target_row];
    for tap in &rows {
        if tap.start > first {
            let passed = (tap.start - first).min(done.len());
            done.drain(..passed);
            first = tap.start;
        }
        while first + done.len() < tap.start + tap.weights.len() {
            done.push_back(across(first + done.len()));
        }
        sum.fill(0.0);
        for (row, weight) in done.iter().zip(&tap.weights) {
            for (total, value) in sum.iter_mut().zip(row) {
                *total += weight * value;
            }
        }
        out.extend(
            sum.iter()
                .map(|value| value.round().clamp(0.0, 255.0) as u8),
        );
    }
    RgbImage::from_raw(width, height, out).expect("the buffer holds width × height pixels")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_colour_stays_flat() {
        let source = RgbImage::from_pixel(97, 61, image::Rgb([200, 40, 7]));
        let reduced = resize(&source, 31, 20);
        assert_eq!(reduced.dimensions(), (31, 20));
        assert!(reduced.pixels().all(|pixel| pixel.0 == [200, 40, 7]));
        let enlarged = resize(&source, 150, 90);
        assert!(enlarged.pixels().all(|pixel| pixel.0 == [200, 40, 7]));
    }

    #[test]
    fn stripes_finer_than_the_result_become_flat_grey() {
        // One-pixel stripes reduced 3 and 2.5 times: the filter widens with
        // the reduction, so no stripe survives as a stripe.
        let source = RgbImage::from_fn(300, 12, |x, _| {
            image::Rgb([if x % 2 == 0 { 0 } else { 255 }; 3])
        });
        for width in [100, 120] {
            let reduced = resize(&source, width, 4);
            let row: Vec<u8> = (3..width - 3).map(|x| reduced.get_pixel(x, 2)[0]).collect();
            assert!(
                row.iter().all(|value| value.abs_diff(128) <= 12),
                "{width}: {row:?}"
            );
        }
    }

    /// The brightness-weighted centre, with pixel centres at half pixels.
    fn centroid(picture: &RgbImage) -> (f64, f64) {
        let (mut total, mut x_sum, mut y_sum) = (0.0, 0.0, 0.0);
        for (x, y, pixel) in picture.enumerate_pixels() {
            let weight = f64::from(pixel[0]);
            total += weight;
            x_sum += weight * (f64::from(x) + 0.5);
            y_sum += weight * (f64::from(y) + 0.5);
        }
        (x_sum / total, y_sum / total)
    }

    #[test]
    fn a_spot_stays_where_it_was() {
        // A soft spot centred at (61.0, 138.5) in a 200 × 200 picture.
        let source = RgbImage::from_fn(200, 200, |x, y| {
            let distance = (f64::from(x) + 0.5 - 61.0).hypot(f64::from(y) + 0.5 - 138.5);
            image::Rgb([(255.0 * (-(distance / 8.0).powi(2)).exp()).round() as u8; 3])
        });
        let (x, y) = centroid(&source);
        assert!((x - 61.0).abs() < 0.01 && (y - 138.5).abs() < 0.01);
        for (width, height) in [(50, 50), (80, 40), (300, 300)] {
            let resized = resize(&source, width, height);
            let (x, y) = centroid(&resized);
            let (sx, sy) = (f64::from(width) / 200.0, f64::from(height) / 200.0);
            assert!(
                (x - 61.0 * sx).abs() < 0.05 && (y - 138.5 * sy).abs() < 0.05,
                "{width} × {height}: ({x}, {y})"
            );
        }
    }

    #[test]
    fn halving_averages_neighbours_and_keeps_features_in_place() {
        // Left half black, right half white: the edge stays in the middle.
        let source = RgbImage::from_fn(200, 10, |x, _| {
            image::Rgb(if x < 100 { [0; 3] } else { [255; 3] })
        });
        let reduced = resize(&source, 100, 5);
        let row: Vec<u8> = (0..100).map(|x| reduced.get_pixel(x, 2)[0]).collect();
        assert!(row[..45].iter().all(|&value| value < 5), "{row:?}");
        assert!(row[55..].iter().all(|&value| value > 250), "{row:?}");
        assert!(row[49] < 128 && row[50] > 128, "{row:?}");
    }
}
