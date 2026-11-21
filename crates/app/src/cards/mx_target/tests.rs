use groups_vision::grid::InkImage;
use groups_vision::marks::{Seen, find_marks};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use super::*;
use crate::cards::pdf;
use crate::cards::sheet::Mark;

type Matrix = [[f64; 3]; 3];

fn apply(m: &Matrix, [x, y]: [f64; 2]) -> [f64; 2] {
    let w = m[2][0] * x + m[2][1] * y + m[2][2];
    [
        (m[0][0] * x + m[0][1] * y + m[0][2]) / w,
        (m[1][0] * x + m[1][1] * y + m[1][2]) / w,
    ]
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

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

/// A camera's view of the sheet, tilted `tilt` degrees about a line at
/// `axis` degrees, turned `roll` degrees and filling `fill` of the photo:
/// sheet mm to photo px.
fn view(sheet: [f64; 2], photo: [u32; 2], tilt: f64, axis: f64, roll: f64, fill: f64) -> Matrix {
    let (st, ct) = tilt.to_radians().sin_cos();
    let (y, x) = axis.to_radians().sin_cos();
    let r = [
        [ct + x * x * (1.0 - ct), x * y * (1.0 - ct), y * st],
        [x * y * (1.0 - ct), ct + y * y * (1.0 - ct), -x * st],
        [-y * st, x * st, ct],
    ];
    let (sr, cr) = roll.to_radians().sin_cos();
    let r = multiply(&[[cr, -sr, 0.0], [sr, cr, 0.0], [0.0, 0.0, 1.0]], &r);
    let distance = 0.9 * sheet[0].max(sheet[1]);
    let shift = |row: usize| -r[row][0] * sheet[0] / 2.0 - r[row][1] * sheet[1] / 2.0;
    let seen = [
        [r[0][0], r[0][1], shift(0)],
        [r[1][0], r[1][1], shift(1)],
        [r[2][0], r[2][1], shift(2) + distance],
    ];
    let corners = [[0.0, 0.0], [sheet[0], 0.0], sheet, [0.0, sheet[1]]].map(|p| apply(&seen, p));
    let lo = [0, 1].map(|i| corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min));
    let hi = [0, 1].map(|i| {
        corners
            .iter()
            .map(|p| p[i])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let (pw, ph) = (f64::from(photo[0]), f64::from(photo[1]));
    let s = fill * (pw / (hi[0] - lo[0])).min(ph / (hi[1] - lo[1]));
    let fit = [
        [s, 0.0, pw / 2.0 - s * (lo[0] + hi[0]) / 2.0 + 0.31],
        [0.0, s, ph / 2.0 - s * (lo[1] + hi[1]) / 2.0 - 0.17],
        [0.0, 0.0, 1.0],
    ];
    multiply(&fit, &seen)
}

/// The sheet drawn at `px_per_mm` as grey levels (text left out: it is
/// drawn by the PDF viewer's fonts).
fn raster(sheet: &Sheet, px_per_mm: f32) -> (usize, usize, Vec<u8>) {
    let w = (sheet.width * px_per_mm).ceil() as u32;
    let h = (sheet.height * px_per_mm).ceil() as u32;
    let mut pixmap = Pixmap::new(w, h).expect("a pixmap");
    pixmap.fill(Color::WHITE);
    let transform = Transform::from_scale(px_per_mm, px_per_mm);
    for mark in &sheet.marks {
        let (points, ink, width, closed) = match mark {
            Mark::Fill { points, ink, .. } => (points, ink, None, true),
            Mark::Line {
                points,
                width,
                ink,
                closed,
                ..
            } => (points, ink, Some(*width), *closed),
            Mark::Text { .. } => continue,
        };
        let mut path = PathBuilder::new();
        path.move_to(points[0][0], points[0][1]);
        for p in &points[1..] {
            path.line_to(p[0], p[1]);
        }
        if closed {
            path.close();
        }
        let Some(path) = path.finish() else {
            continue;
        };
        let [r, g, b] = ink.rgb();
        let mut paint = Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = true;
        match width {
            None => pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None),
            Some(width) => {
                let stroke = Stroke {
                    width,
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, transform, None);
            }
        }
    }
    let grey = pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| ((u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2])) / 10) as u8)
        .collect();
    (w as usize, h as usize, grey)
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn gaussian(&mut self) -> f64 {
        let (u, v) = (self.next().max(1e-12), self.next());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

fn blur(plane: &mut [f32], w: usize, h: usize, sigma: f64) {
    let radius = (3.0 * sigma).ceil() as i64;
    let kernel: Vec<f32> = (-radius..=radius)
        .map(|k| (-(k * k) as f64 / (2.0 * sigma * sigma)).exp() as f32)
        .collect();
    let total: f32 = kernel.iter().sum();
    let pass = |plane: &mut [f32], start: usize, step: usize, count: usize| {
        let line: Vec<f32> = (0..count).map(|i| plane[start + i * step]).collect();
        for i in 0..count {
            let mut sum = 0.0;
            for (k, weight) in kernel.iter().enumerate() {
                let j = (i as i64 + k as i64 - radius).clamp(0, count as i64 - 1) as usize;
                sum += weight * line[j];
            }
            plane[start + i * step] = sum / total;
        }
    };
    for y in 0..h {
        pass(plane, y * w, 1, w);
    }
    for x in 0..w {
        pass(plane, x, w, h);
    }
}

/// A phone photo of the printed sheet: warped through `to_px`, unevenly
/// lit, blurred, noisy and kept as a JPEG the way the app keeps photos.
fn photograph(sheet: &Sheet, to_px: &Matrix, photo: [u32; 2], seed: u64) -> InkImage {
    let scale = 9.0f32;
    let (sw, sh, grey) = raster(sheet, scale);
    let to_mm = invert(to_px);
    let (w, h) = (photo[0] as usize, photo[1] as usize);
    let mut plane = vec![0f32; w * h];
    let sample = |x: f64, y: f64| -> f32 {
        let (fx, fy) = (x - 0.5, y - 0.5);
        if fx < 0.0 || fy < 0.0 || fx >= (sw - 1) as f64 || fy >= (sh - 1) as f64 {
            return 55.0;
        }
        let (ix, iy) = (fx as usize, fy as usize);
        let (ax, ay) = ((fx - ix as f64) as f32, (fy - iy as f64) as f32);
        let at = |i: usize, j: usize| f32::from(grey[j * sw + i]);
        let top = at(ix, iy) + ax * (at(ix + 1, iy) - at(ix, iy));
        let bottom = at(ix, iy + 1) + ax * (at(ix + 1, iy + 1) - at(ix, iy + 1));
        top + ay * (bottom - top)
    };
    for y in 0..h {
        for x in 0..w {
            let mm = apply(&to_mm, [x as f64 + 0.5, y as f64 + 0.5]);
            let s = f64::from(scale);
            let light = 0.95 - 0.3 * (x as f64 / w as f64 - 0.5) - 0.15 * (y as f64 / h as f64);
            plane[y * w + x] = sample(mm[0] * s, mm[1] * s) * light as f32;
        }
    }
    blur(&mut plane, w, h, 1.1);
    let mut random = Random(seed);
    let rgb: Vec<u8> = plane
        .iter()
        .flat_map(|&v| {
            let v = f64::from(v);
            [1.0, 0.97, 0.92].map(|tint| {
                (v * tint + 3.0 * random.gaussian())
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
        })
        .collect();
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, photo[0], photo[1]);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().expect("a png header");
        writer.write_image_data(&rgb).expect("png data");
    }
    let kept = groups_vision::import_photo(
        &png,
        groups_vision::ImportOptions {
            max_long_edge_px: None,
            jpeg_quality: 80,
        },
    )
    .expect("kept as a JPEG");
    let decoded = groups_vision::thumbnail(&kept.jpeg, u32::MAX).expect("the JPEG reads");
    InkImage::from_rgba(decoded.width_px, decoded.height_px, &decoded.rgba)
}

#[test]
fn a_photo_of_the_printed_card_is_measured() {
    let cases = [
        (
            MxTargetOptions {
                paper: MxPaper::A4,
                ..MxTargetOptions::default()
            },
            [2400, 1800],
            (25.0, 30.0, 92.0),
        ),
        (
            MxTargetOptions {
                paper: MxPaper::Letter,
                landscape: true,
                aims: 6,
                grid: MxGrid::Inch,
                square: true,
                size_mm: 25,
            },
            [2000, 1500],
            (35.0, 100.0, -8.0),
        ),
        (
            MxTargetOptions {
                paper: MxPaper::A3,
                aims: 1,
                grid: MxGrid::Off,
                size_mm: 80,
                ..MxTargetOptions::default()
            },
            [1800, 2400],
            (15.0, 250.0, 181.0),
        ),
    ];
    for (n, (options, photo, (tilt, axis, roll))) in cases.into_iter().enumerate() {
        let layout = options.layout();
        let sheet = page(&layout);
        let to_px = view(layout.sheet_mm(), photo, tilt, axis, roll, 0.9);
        let image = photograph(&sheet, &to_px, photo, n as u64);
        let marks = find_marks(
            &image,
            [0.0, 0.0, f64::from(image.width), f64::from(image.height)],
        )
        .unwrap_or_else(|e| panic!("case {n}: {e}"));
        assert_eq!(marks.layout, layout, "case {n}");
        assert_eq!(marks.seen, [Seen::Read; 4], "case {n}");
        let mut corner: f64 = 0.0;
        for (got, truth) in marks.centres_px.iter().zip(layout.mark_centres_mm()) {
            let truth = apply(&to_px, truth);
            corner = corner.max((got[0] - truth[0]).hypot(got[1] - truth[1]));
        }
        let to_mm = invert(&to_px);
        let fix = marks.calibration().expect("a calibration");
        let mut aim: f64 = 0.0;
        for (got, truth) in fix.aim_points_px.iter().zip(layout.aim_points_mm()) {
            let mm = apply(&to_mm, [got.x, got.y]);
            aim = aim.max((mm[0] - truth[0]).hypot(mm[1] - truth[1]));
        }
        println!("card {n}: corners {corner:.3} px, aims {aim:.3} mm");
        assert!(corner < 0.3, "case {n}: {corner}");
        assert!(aim < 0.5, "case {n}: {aim}");
    }
}

#[test]
fn every_size_and_layout_makes_a_pdf() {
    for paper in [MxPaper::A4, MxPaper::A3, MxPaper::Letter] {
        for landscape in [false, true] {
            for aims in [1, 2, 4, 6] {
                let options = MxTargetOptions {
                    paper,
                    landscape,
                    aims,
                    ..MxTargetOptions::default()
                };
                let layout = options.layout();
                let sheet = page(&layout);
                let [w, h] = layout.sheet_mm();
                assert!((f64::from(sheet.width) - w).abs() < 1e-3);
                assert!((f64::from(sheet.height) - h).abs() < 1e-3);
                for mark in &sheet.marks {
                    let points: Vec<[f32; 2]> = match mark {
                        Mark::Line { points, .. } | Mark::Fill { points, .. } => points.clone(),
                        Mark::Text { at, .. } => vec![*at],
                    };
                    for p in points {
                        assert!(p[0] >= 10.0 && p[0] <= sheet.width - 10.0, "{p:?}");
                        assert!(p[1] >= 10.0 && p[1] <= sheet.height - 10.0, "{p:?}");
                    }
                }
                let bytes = pdf::write(&[sheet], "MX target");
                assert!(bytes.starts_with(b"%PDF-1.7"));
                let text = String::from_utf8_lossy(&bytes);
                let media = format!(
                    "/MediaBox [0 0 {:.2} {:.2}]",
                    w as f32 * 72.0 / 25.4,
                    h as f32 * 72.0 / 25.4
                );
                assert!(text.contains(&media), "{media}");
                assert!(options.file_name().ends_with(&format!("-{aims}.pdf")));
            }
        }
    }
}

#[test]
fn options_keep_to_the_layouts_there_are() {
    let odd = MxTargetOptions {
        paper: MxPaper::A4,
        aims: 5,
        size_mm: 30,
        ..MxTargetOptions::default()
    };
    assert_eq!(odd.layout().aims, AimPoints::Four);
    assert_eq!(odd.layout().size, AimSize::Mm25);
    let json = serde_json::to_string(&MxTargetOptions::default()).unwrap();
    let back: MxTargetOptions = serde_json::from_str(&json).unwrap();
    assert_eq!(back, MxTargetOptions::default());
    let partial: MxTargetOptions = serde_json::from_str(r#"{"aims": 6}"#).unwrap();
    assert_eq!(partial.aims, 6);
    assert_eq!(partial.paper, MxTargetOptions::default().paper);
    assert_eq!(MxPaper::for_locale(Some("en-US")), MxPaper::Letter);
    assert_eq!(MxPaper::for_locale(Some("fr_CA")), MxPaper::Letter);
    assert_eq!(MxPaper::for_locale(Some("sv-SE")), MxPaper::A4);
    assert_eq!(MxPaper::for_locale(Some("en")), MxPaper::A4);
    assert_eq!(MxPaper::for_locale(None), MxPaper::A4);
    assert!(words(&odd.layout()).contains("A4"));
}

/// Writes one PDF for each paper and layout, and a photo of one, to
/// `target/screens/m4` (`cargo test -p mxsc write_mx_targets -- --ignored`).
#[test]
#[ignore]
fn write_mx_targets() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screens/m4");
    std::fs::create_dir_all(&directory).expect("a folder");
    let mut written = Vec::new();
    for paper in [MxPaper::A4, MxPaper::A3, MxPaper::Letter] {
        for (aims, landscape, square, grid) in [
            (1, false, false, MxGrid::Mm10),
            (2, true, true, MxGrid::Inch),
            (4, false, false, MxGrid::Mm10),
            (6, true, true, MxGrid::Off),
        ] {
            let options = MxTargetOptions {
                paper,
                landscape,
                aims,
                grid,
                square,
                size_mm: if aims > 2 { 40 } else { 60 },
            };
            let sheet = page(&options.layout());
            let path = directory.join(options.file_name());
            std::fs::write(&path, pdf::write(&[sheet], "MX target")).expect("written");
            written.push(path);
        }
    }
    let layout = MxTargetOptions {
        paper: MxPaper::A4,
        ..MxTargetOptions::default()
    }
    .layout();
    let to_px = view(layout.sheet_mm(), [2400, 1800], 25.0, 30.0, 92.0, 0.9);
    let image = photograph(&page(&layout), &to_px, [2400, 1800], 1);
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, image.width, image.height);
        encoder.set_color(png::ColorType::Grayscale);
        let mut writer = encoder.write_header().expect("a png header");
        writer.write_image_data(&image.pixels).expect("png data");
    }
    std::fs::write(directory.join("mx-target-photo.png"), png).expect("written");
    for path in written {
        println!("{}", path.display());
    }
}

/// The page needs no rifle, and its controls fit a phone's width.
#[test]
fn the_page_shows_without_a_rifle() {
    use crate::cards::{CardSettings, CardsPage, Kind};
    let texts = |output: &egui::FullOutput| -> Vec<String> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect()
    };
    for width in [1200.0, 390.0] {
        let context = egui::Context::default();
        let mut page = CardsPage::new(CardSettings::default());
        let frame = |page: &mut CardsPage| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    crate::mobile::apply_style(ui.ctx());
                    page.show(ui, None, true);
                },
            );
            output.textures_delta.clear();
            output
        };
        // Without a rifle the other cards point to Rifles; the MX target
        // is still on offer.
        let output = frame(&mut page);
        assert!(texts(&output).iter().any(|t| t == "MX target"));
        page.settings.kind = Kind::MxTarget;
        let mut output = frame(&mut page);
        for _ in 0..2 {
            output = frame(&mut page);
        }
        let shown = texts(&output);
        for key in ["Save PDF", "Aim points", "Print at 100% (no scaling)"] {
            assert!(
                shown.iter().any(|t| t == key),
                "{key} at {width}: {shown:?}"
            );
        }
        if width < 400.0 {
            for shape in &output.shapes {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && shape.clip_rect.width() >= width - 1.0
                {
                    let right = text.pos.x + text.galley.size().x;
                    assert!(right <= width + 0.5, "{:?} overflows", text.galley.job.text);
                }
            }
        }
    }
}
