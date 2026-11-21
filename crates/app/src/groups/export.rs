//! Groups out of the app: a group as a PNG picture to share (the photo or
//! a grid, the marks, the rings and the key numbers), drawn on the CPU with
//! tiny-skia and the cards' fonts so it works the same in the browser; and
//! the groups and their shots as CSV.

use ballistics_store::sun::Date;
use ballistics_store::{Calibration, Group, Pending, PhotoStore};
use groups_core::{GroupStats, Point, VelocityStats};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, StrokeDash, Transform};

use crate::app::{Action, Data};
use crate::cards::fonts::Font;
use crate::files::FileOut;
use crate::groups::measure::{self, Measured};
use crate::i18n::{tr, trf};
use crate::theme;
use crate::units::{Quantity, UnitPrefs};

const WIDTH: u32 = 1080;
const DRAWING: u32 = 1080;
const FOOTER: u32 = 340;
/// The photo is read at most this large for the picture.
const PHOTO_LONG_EDGE: u32 = 3000;
const DAY_MS: i64 = 86_400_000;

/// A picture being made: waiting for the photo's bytes, when it has one.
pub struct PictureJob {
    group_id: String,
    units: UnitPrefs,
    photo: Option<Pending<Option<Vec<u8>>>>,
}

impl PictureJob {
    pub fn start(group: &Group, data: &Data, photos: &dyn PhotoStore, units: UnitPrefs) -> Self {
        let photo = crate::groups::photo_of(group, data).map(|photo| photos.get(&photo.id));
        PictureJob {
            group_id: group.id.clone(),
            units,
            photo,
        }
    }

    /// The picture to save once it is drawn.
    pub fn poll(&mut self, data: &Data) -> Option<Action> {
        let bytes = match &self.photo {
            Some(pending) => pending.take()?.unwrap_or_default(),
            None => None,
        };
        let Some(group) = data.groups.groups.iter().find(|g| g.id == self.group_id) else {
            return Some(Action::Problem(tr("The group is gone").to_owned()));
        };
        Some(
            match group_png(group, data, bytes.as_deref(), &self.units) {
                Ok(png) => Action::SaveFile(Box::new(FileOut {
                    name: file_name(group, data, "png"),
                    bytes: png,
                    mime: "image/png",
                })),
                Err(error) => Action::Problem(error),
            },
        )
    }
}

/// "group-2026-11-21-1430.png", on the local clock.
pub fn file_name(group: &Group, data: &Data, extension: &str) -> String {
    let local = crate::groups::local_ms(group.taken_at, data.country);
    let date = Date::from_days(local.div_euclid(DAY_MS));
    let minutes = local.rem_euclid(DAY_MS) / 60_000;
    format!(
        "group-{:04}-{:02}-{:02}-{:02}{:02}.{extension}",
        date.year,
        date.month,
        date.day,
        minutes / 60,
        minutes % 60
    )
}

fn colour(colour: eframe::egui::Color32) -> Color {
    let [r, g, b, a] = colour.to_srgba_unmultiplied();
    Color::from_rgba8(r, g, b, a)
}

fn paint(colour_of: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(colour_of);
    paint.anti_alias = true;
    paint
}

/// Text drawn from a TrueType font's outlines.
struct Text {
    face: ttf_parser::Face<'static>,
}

struct Outline(PathBuilder, f32, f32, f32);

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(self.1 + x * self.3, self.2 - y * self.3);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(self.1 + x * self.3, self.2 - y * self.3);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (ox, oy, s) = (self.1, self.2, self.3);
        self.0
            .quad_to(ox + x1 * s, oy - y1 * s, ox + x * s, oy - y * s);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (ox, oy, s) = (self.1, self.2, self.3);
        self.0.cubic_to(
            ox + x1 * s,
            oy - y1 * s,
            ox + x2 * s,
            oy - y2 * s,
            ox + x * s,
            oy - y * s,
        );
    }
    fn close(&mut self) {
        self.0.close();
    }
}

impl Text {
    fn new(font: Font) -> Option<Text> {
        let face = ttf_parser::Face::parse(font.file()?, 0).ok()?;
        Some(Text { face })
    }

    fn glyph(&self, c: char) -> Option<ttf_parser::GlyphId> {
        self.face
            .glyph_index(c)
            .or_else(|| self.face.glyph_index('?'))
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        let scale = size / f32::from(self.face.units_per_em());
        text.chars()
            .filter_map(|c| self.glyph(c))
            .map(|glyph| f32::from(self.face.glyph_hor_advance(glyph).unwrap_or(0)) * scale)
            .sum()
    }

    /// Draws `text` with its baseline at `y`, starting at `x` (or ending
    /// there when `right`).
    fn draw(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        (x, y): (f32, f32),
        size: f32,
        fill: Color,
        right: bool,
    ) {
        let scale = size / f32::from(self.face.units_per_em());
        let mut at = if right { x - self.width(text, size) } else { x };
        let paint = paint(fill);
        for c in text.chars() {
            let Some(glyph) = self.glyph(c) else {
                continue;
            };
            let mut outline = Outline(PathBuilder::new(), at, y, scale);
            if self.face.outline_glyph(glyph, &mut outline).is_some()
                && let Some(path) = outline.0.finish()
            {
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            at += f32::from(self.face.glyph_hor_advance(glyph).unwrap_or(0)) * scale;
        }
    }

    /// Cuts `text` with "…" to fit `width`.
    fn fitted(&self, text: &str, size: f32, width: f32) -> String {
        if self.width(text, size) <= width {
            return text.to_owned();
        }
        let mut cut: String = text.to_owned();
        while !cut.is_empty() && self.width(&format!("{cut}…"), size) > width {
            cut.pop();
        }
        format!("{}…", cut.trim_end())
    }
}

/// Millimetres on the target to pixels of the drawing, y up.
struct Fit {
    centre: Point,
    scale: f64,
}

impl Fit {
    fn around(points: &[Point], extra_mm: f64, min_half_mm: f64) -> Fit {
        let (mut low, mut high) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        for point in points {
            low = Point::new(low.x.min(point.x), low.y.min(point.y));
            high = Point::new(high.x.max(point.x), high.y.max(point.y));
        }
        if points.is_empty() {
            (low, high) = (Point::ORIGIN, Point::ORIGIN);
        }
        let centre = Point::new((low.x + high.x) / 2.0, (low.y + high.y) / 2.0);
        let half = ((high.x - low.x).max(high.y - low.y) / 2.0 + extra_mm).max(min_half_mm) * 1.15;
        Fit {
            centre,
            scale: f64::from(DRAWING) / (2.0 * half),
        }
    }

    fn at(&self, point: Point) -> (f32, f32) {
        let middle = f64::from(DRAWING) / 2.0;
        (
            (middle + (point.x - self.centre.x) * self.scale) as f32,
            (middle - (point.y - self.centre.y) * self.scale) as f32,
        )
    }

    fn mm(&self, x: f64, y: f64) -> Point {
        let middle = f64::from(DRAWING) / 2.0;
        Point::new(
            self.centre.x + (x - middle) / self.scale,
            self.centre.y - (y - middle) / self.scale,
        )
    }
}

/// The photo under the drawing, straightened through its scale: each pixel
/// of the drawing is looked up on the photo.
fn photo_under(
    pixmap: &mut Pixmap,
    fit: &Fit,
    calibration: &Calibration,
    bytes: &[u8],
    width_px: u32,
) -> bool {
    let Ok(mapping) = measure::scale(calibration) else {
        return false;
    };
    let Ok(photo) = groups_vision::thumbnail(bytes, PHOTO_LONG_EDGE) else {
        return false;
    };
    let factor = f64::from(photo.width_px) / f64::from(width_px.max(1));
    let (w, h) = (photo.width_px as usize, photo.height_px as usize);
    let sample = |x: f64, y: f64| -> Option<[u8; 3]> {
        if !(0.0..(w - 1) as f64).contains(&x) || !(0.0..(h - 1) as f64).contains(&y) {
            return None;
        }
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (fx, fy) = (x - x0 as f64, y - y0 as f64);
        let at = |x: usize, y: usize, c: usize| f64::from(photo.rgba[(y * w + x) * 4 + c]);
        let mut rgb = [0u8; 3];
        for (c, value) in rgb.iter_mut().enumerate() {
            let top = at(x0, y0, c) * (1.0 - fx) + at(x0 + 1, y0, c) * fx;
            let bottom = at(x0, y0 + 1, c) * (1.0 - fx) + at(x0 + 1, y0 + 1, c) * fx;
            *value = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
        }
        Some(rgb)
    };
    let stride = WIDTH as usize * 4;
    let data = pixmap.data_mut();
    for y in 0..DRAWING as usize {
        for x in 0..WIDTH as usize {
            let mm = fit.mm(x as f64 + 0.5, y as f64 + 0.5);
            let Some(px) = mapping.to_px(mm) else {
                continue;
            };
            if let Some([r, g, b]) = sample(px.x * factor, px.y * factor) {
                let at = y * stride + x * 4;
                data[at..at + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    true
}

fn circle(x: f32, y: f32, r: f32) -> Option<tiny_skia::Path> {
    PathBuilder::from_circle(x, y, r.max(0.5))
}

fn line(pixmap: &mut Pixmap, from: (f32, f32), to: (f32, f32), fill: Color, width: f32) {
    let mut path = PathBuilder::new();
    path.move_to(from.0, from.1);
    path.line_to(to.0, to.1);
    if let Some(path) = path.finish() {
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint(fill), &stroke, Transform::identity(), None);
    }
}

fn ring(pixmap: &mut Pixmap, at: (f32, f32), radius: f32, fill: Color, width: f32, dashed: bool) {
    let Some(path) = circle(at.0, at.1, radius) else {
        return;
    };
    let stroke = Stroke {
        width,
        dash: dashed
            .then(|| StrokeDash::new(vec![14.0, 9.0], 0.0))
            .flatten(),
        ..Stroke::default()
    };
    pixmap.stroke_path(&path, &paint(fill), &stroke, Transform::identity(), None);
}

fn grid(pixmap: &mut Pixmap, fit: &Fit, fill: Color) -> f64 {
    let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]
        .into_iter()
        .find(|step| step * fit.scale >= 60.0)
        .unwrap_or(1000.0);
    let low = fit.mm(0.0, f64::from(DRAWING));
    let high = fit.mm(f64::from(WIDTH), 0.0);
    let mut x = (low.x / step).ceil() * step;
    while x <= high.x {
        let (px, _) = fit.at(Point::new(x, 0.0));
        line(pixmap, (px, 0.0), (px, DRAWING as f32), fill, 1.5);
        x += step;
    }
    let mut y = (low.y / step).ceil() * step;
    while y <= high.y {
        let (_, py) = fit.at(Point::new(0.0, y));
        line(pixmap, (0.0, py), (WIDTH as f32, py), fill, 1.5);
        y += step;
    }
    step
}

fn angle_and_size(u: &UnitPrefs, mm: f64, distance: Option<f64>) -> (String, String) {
    match distance {
        Some(distance) => (
            u.format(
                Quantity::GroupAngle,
                groups_core::angles::mrad(mm, distance),
            ),
            u.format(Quantity::GroupSize, mm),
        ),
        None => (u.format(Quantity::GroupSize, mm), String::new()),
    }
}

/// The group as a PNG: the drawing (on its photo when `photo` holds the
/// photo's bytes) above its key numbers.
pub fn group_png(
    group: &Group,
    data: &Data,
    photo: Option<&[u8]>,
    u: &UnitPrefs,
) -> Result<Vec<u8>, String> {
    let measured = crate::groups::measure_group(group, data)
        .map_err(|_| tr("The group can't be drawn yet: set its scale first").to_owned())?;
    let stats = measured.stats.as_ref();
    let calibre_mm = crate::groups::form::calibre_of(group.bullet_id.as_deref(), data);
    let regular = Text::new(Font::BarlowCondensed).ok_or("font")?;
    let bold = Text::new(Font::BarlowCondensedBold).ok_or("font")?;
    let mut pixmap = Pixmap::new(WIDTH, DRAWING + FOOTER).ok_or("picture")?;
    let marks = &theme::SUN;
    pixmap.fill(colour(theme::BONE));

    let radius_mm = calibre_mm.unwrap_or(6.0) / 2.0;
    let mut points = framed(&measured);
    let mut extra = radius_mm;
    if let Some(stats) = stats {
        points.push(stats.centre);
        let cep = stats.spread.map_or(0.0, |spread| spread.cep.value);
        extra = extra.max(stats.mean_radius.max(cep) - stats.width.max(stats.height) / 2.0);
    }
    let fit = Fit::around(&points, extra, 3.5 * 2.0 * radius_mm);
    let on_photo = match (crate::groups::photo_of(group, data), photo) {
        (Some(record), Some(bytes)) => record.calibration.as_ref().is_some_and(|calibration| {
            photo_under(&mut pixmap, &fit, calibration, bytes, record.width_px)
        }),
        _ => false,
    };
    let step = if on_photo {
        None
    } else {
        Some(grid(
            &mut pixmap,
            &fit,
            colour(marks.stroke.gamma_multiply(0.35)),
        ))
    };

    let radius = ((radius_mm * fit.scale) as f32).max(6.0);
    let outline = if on_photo {
        colour(eframe::egui::Color32::WHITE)
    } else {
        colour(theme::BONE)
    };
    for (index, shot) in measured.shots_mm.iter().enumerate() {
        let at = fit.at(*shot);
        let fill = if measured.cold_bore.contains(&index) {
            marks.measure
        } else if measured.called_fliers.contains(&index) {
            marks.warning
        } else {
            marks.text
        };
        if let Some(path) = circle(at.0, at.1, radius) {
            let fill = if on_photo {
                fill.gamma_multiply(0.55)
            } else {
                fill
            };
            pixmap.fill_path(
                &path,
                &paint(colour(fill)),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        ring(&mut pixmap, at, radius, outline, 2.5, false);
        if radius >= 14.0 {
            let size = (radius * 1.0).min(30.0);
            let number = (index + 1).to_string();
            let width = bold.width(&number, size);
            bold.draw(
                &mut pixmap,
                &number,
                (at.0 - width / 2.0, at.1 + size * 0.35),
                size,
                outline,
                false,
            );
        }
    }
    if let Some(stats) = stats {
        let centre = fit.at(stats.centre);
        if stats.shots >= 2 {
            ring(
                &mut pixmap,
                centre,
                (stats.mean_radius * fit.scale) as f32,
                colour(marks.accent),
                4.0,
                false,
            );
        }
        if let Some(spread) = stats.spread {
            ring(
                &mut pixmap,
                centre,
                (spread.cep.value * fit.scale) as f32,
                colour(marks.measure),
                3.5,
                true,
            );
        }
        let half = 16.0;
        let accent = colour(marks.accent);
        line(
            &mut pixmap,
            (centre.0 - half, centre.1 - half),
            (centre.0 + half, centre.1 + half),
            accent,
            4.0,
        );
        line(
            &mut pixmap,
            (centre.0 - half, centre.1 + half),
            (centre.0 + half, centre.1 - half),
            accent,
            4.0,
        );
    }
    if let Some(aim) = measured.aim_mm {
        let at = fit.at(aim);
        let alarm = colour(marks.alarm);
        line(
            &mut pixmap,
            (at.0 - 24.0, at.1),
            (at.0 + 24.0, at.1),
            alarm,
            4.0,
        );
        line(
            &mut pixmap,
            (at.0, at.1 - 24.0),
            (at.0, at.1 + 24.0),
            alarm,
            4.0,
        );
    }
    if let Some(step) = step {
        regular.draw(
            &mut pixmap,
            &trf!("grid {}", u.format(Quantity::GroupSize, step)),
            (20.0, DRAWING as f32 - 20.0),
            26.0,
            colour(marks.text_muted),
            false,
        );
    }
    footer(
        &mut pixmap,
        group,
        data,
        &measured,
        stats,
        u,
        &regular,
        &bold,
    );
    pixmap.encode_png().map_err(|error| error.to_string())
}

fn framed(measured: &Measured) -> Vec<Point> {
    let counted: Vec<Point> = measured
        .counted
        .iter()
        .map(|&i| measured.shots_mm[i])
        .collect();
    let shots = if counted.is_empty() {
        measured.shots_mm.clone()
    } else {
        counted
    };
    shots.into_iter().chain(measured.aim_mm).collect()
}

#[allow(clippy::too_many_arguments)]
fn footer(
    pixmap: &mut Pixmap,
    group: &Group,
    data: &Data,
    measured: &Measured,
    stats: Option<&GroupStats>,
    u: &UnitPrefs,
    regular: &Text,
    bold: &Text,
) {
    let palette = &theme::DARK;
    let top = DRAWING as f32;
    if let Some(band) = Rect::from_xywh(0.0, top, WIDTH as f32, FOOTER as f32) {
        pixmap.fill_rect(
            band,
            &paint(colour(palette.background)),
            Transform::identity(),
            None,
        );
    }
    let text = colour(palette.text);
    let muted = colour(palette.text_muted);
    let accent = colour(palette.accent);
    let left = 40.0;
    let room = WIDTH as f32 - 2.0 * left;
    let title = regular.fitted(&crate::groups::title_of(group, data), 50.0, room);
    bold.draw(pixmap, &title, (left, top + 66.0), 50.0, text, false);
    let mut about = vec![trf!("{} shots", measured.shots_mm.len())];
    if let Some(distance) = group.distance_m {
        about.push(u.format(Quantity::Distance, distance));
    }
    if let Some(rifle) = group
        .rifle_id
        .as_ref()
        .and_then(|id| data.rifles.iter().find(|r| &r.id == id))
    {
        about.push(rifle.name.clone());
    }
    let local = crate::groups::local_ms(group.taken_at, data.country);
    let today =
        crate::groups::local_ms(ballistics_store::model::now_ms(), data.country).div_euclid(DAY_MS);
    about.push(crate::groups::library::day_name(
        local.div_euclid(DAY_MS),
        today,
    ));
    let about = regular.fitted(&about.join(" · "), 30.0, room);
    regular.draw(pixmap, &about, (left, top + 110.0), 30.0, muted, false);

    let distance = group.distance_m;
    let mut cells: Vec<(&str, (String, String))> = Vec::new();
    if let Some(stats) = stats {
        if let Some(spread) = stats.spread {
            cells.push((
                tr("Extreme spread"),
                angle_and_size(u, spread.extreme_spread.centre_to_centre, distance),
            ));
        }
        cells.push((
            tr("Mean radius"),
            angle_and_size(u, stats.mean_radius, distance),
        ));
        if let Some(spread) = stats.spread {
            cells.push((
                tr("Radial SD"),
                angle_and_size(u, spread.sigma.value, distance),
            ));
        }
        if let Some(impact) = stats.impact {
            let way = |mm: f64, plus: &str, minus: &str| {
                let (shown, _) = angle_and_size(u, mm.abs(), distance);
                format!("{shown} {}", if mm >= 0.0 { plus } else { minus })
            };
            cells.push((
                tr("Impact"),
                (
                    way(impact.x, tr("right"), tr("left")),
                    way(impact.y, tr("high"), tr("low")),
                ),
            ));
        }
    }
    let width = room / cells.len().max(1) as f32;
    for (index, (title, (big, small))) in cells.iter().enumerate() {
        let x = left + index as f32 * width;
        let cell = width - 16.0;
        regular.draw(
            pixmap,
            &regular.fitted(title, 26.0, cell),
            (x, top + 170.0),
            26.0,
            muted,
            false,
        );
        bold.draw(
            pixmap,
            &bold.fitted(big, 42.0, cell),
            (x, top + 218.0),
            42.0,
            accent,
            false,
        );
        regular.draw(
            pixmap,
            &regular.fitted(small, 28.0, cell),
            (x, top + 254.0),
            28.0,
            text,
            false,
        );
    }
    let readings: Vec<f64> = crate::groups::details::velocity::shot_velocities(&group.marks)
        .into_iter()
        .flatten()
        .chain(group.extra_velocities_mps.iter().copied())
        .collect();
    if let Ok(velocity) = VelocityStats::new(&readings) {
        let mut parts = vec![trf!("avg {}", u.format(Quantity::Speed, velocity.average))];
        if let Some(sd) = velocity.sd {
            parts.push(trf!("SD {}", u.format(Quantity::Speed, sd.value)));
        }
        parts.push(trf!(
            "ES {}",
            u.format(Quantity::Speed, velocity.extreme_spread)
        ));
        let line = trf!("Velocity: {}", parts.join(" · "));
        regular.draw(
            pixmap,
            &regular.fitted(&line, 28.0, room * 0.7),
            (left, top + 310.0),
            28.0,
            muted,
            false,
        );
    }
    regular.draw(
        pixmap,
        crate::APP_NAME,
        (WIDTH as f32 - left, top + 310.0),
        24.0,
        muted,
        true,
    );
}

/// A CSV field: quoted when it holds a comma, a quote or a line break.
/// A CSV text field: quoted when it holds a comma, a quote or a line
/// break, and with a `'` before a leading `=`, `+`, `-`, `@`, tab or carriage
/// return, so a spreadsheet shows it as text rather than running it as a
/// formula.
fn field(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

fn number(value: Option<f64>, decimals: usize) -> String {
    value.map_or_else(String::new, |value| format!("{value:.decimals$}"))
}

/// "2026-11-21T14:30:05Z".
pub fn utc_text(unix_ms: i64) -> String {
    let date = Date::from_days(unix_ms.div_euclid(DAY_MS));
    let seconds = unix_ms.rem_euclid(DAY_MS) / 1000;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        date.year,
        date.month,
        date.day,
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn row(fields: &[String]) -> String {
    let mut line = fields.join(",");
    line.push_str("\r\n");
    line
}

/// The columns of `groups_csv`.
pub const GROUP_COLUMNS: [&str; 39] = [
    "group_id",
    "taken_at_utc",
    "rifle",
    "barrel",
    "bullet",
    "powder",
    "charge_g",
    "cbto_mm",
    "distance_m",
    "session",
    "shots",
    "counted_shots",
    "extreme_spread_mm",
    "extreme_spread_mrad",
    "mean_radius_mm",
    "mean_radius_mrad",
    "sigma_mm",
    "sigma_mrad",
    "cep_mm",
    "cep_mrad",
    "width_mm",
    "height_mm",
    "impact_right_mm",
    "impact_up_mm",
    "impact_right_mrad",
    "impact_up_mrad",
    "temperature_c",
    "powder_temperature_c",
    "pressure_hpa",
    "humidity_percent",
    "wind_speed_mps",
    "wind_clock",
    "velocity_count",
    "velocity_avg_mps",
    "velocity_sd_mps",
    "velocity_es_mps",
    "tags",
    "notes",
    "archived",
];

/// The columns of `shots_csv`.
pub const SHOT_COLUMNS: [&str; 18] = [
    "group_id",
    "taken_at_utc",
    "rifle",
    "bullet",
    "distance_m",
    "shot",
    "cold_bore",
    "called_flier",
    "counted",
    "right_of_aim_mm",
    "up_from_aim_mm",
    "right_of_aim_mrad",
    "up_from_aim_mrad",
    "right_of_centre_mm",
    "up_from_centre_mm",
    "right_of_centre_mrad",
    "up_from_centre_mrad",
    "velocity_mps",
];

fn names(group: &Group, data: &Data) -> (String, String, String) {
    let rifle = group
        .rifle_id
        .as_ref()
        .and_then(|id| data.rifles.iter().find(|r| &r.id == id))
        .map(|r| r.name.clone())
        .unwrap_or_default();
    let barrel = group
        .barrel_id
        .as_ref()
        .and_then(|id| data.groups.barrels.iter().find(|b| &b.id == id))
        .map(|b| b.name.clone())
        .unwrap_or_default();
    let bullet = group
        .bullet_id
        .as_ref()
        .and_then(|id| data.bullets.iter().find(|b| &b.id == id))
        .map(|b| b.display_name())
        .unwrap_or_default();
    (rifle, barrel, bullet)
}

/// One row per group, with its load and conditions, metric units.
pub fn groups_csv(groups: &[&Group], data: &Data) -> String {
    let mut csv = row(&GROUP_COLUMNS.map(str::to_owned));
    for group in groups {
        let (rifle, barrel, bullet_name) = names(group, data);
        let bullet = group
            .bullet_id
            .as_ref()
            .and_then(|id| data.bullets.iter().find(|b| &b.id == id));
        let recipe = bullet.and_then(|bullet| bullet.recipe.as_ref());
        let session = group
            .session_id
            .as_ref()
            .and_then(|id| data.groups.sessions.iter().find(|s| &s.id == id));
        let conditions = session.map_or(group.conditions, |s| group.conditions.or(s.conditions));
        let measured = crate::groups::measure_group(group, data).ok();
        let stats = measured.as_ref().and_then(|m| m.stats.as_ref());
        let spread = stats.and_then(|stats| stats.spread);
        let mrad = |mm: Option<f64>| {
            mm.zip(group.distance_m)
                .map(|(mm, distance)| groups_core::angles::mrad(mm, distance))
        };
        let es = spread.map(|s| s.extreme_spread.centre_to_centre);
        let mr = stats.map(|s| s.mean_radius);
        let sigma = spread.map(|s| s.sigma.value);
        let cep = spread.map(|s| s.cep.value);
        let impact = stats.and_then(|s| s.impact);
        let readings: Vec<f64> = crate::groups::details::velocity::shot_velocities(&group.marks)
            .into_iter()
            .flatten()
            .chain(group.extra_velocities_mps.iter().copied())
            .collect();
        let velocity = VelocityStats::new(&readings).ok();
        let fields = vec![
            group.id.clone(),
            utc_text(group.taken_at),
            field(&rifle),
            field(&barrel),
            field(&bullet_name),
            field(recipe.map_or("", |r| r.powder.as_str())),
            number(recipe.and_then(|r| r.charge_g), 3),
            number(recipe.and_then(|r| r.cbto_mm), 2),
            number(group.distance_m, 1),
            field(&session.map(|s| s.place.clone()).unwrap_or_default()),
            group.shot_count().to_string(),
            measured
                .as_ref()
                .map_or(String::new(), |m| m.counted.len().to_string()),
            number(es, 2),
            number(mrad(es), 3),
            number(mr, 2),
            number(mrad(mr), 3),
            number(sigma, 2),
            number(mrad(sigma), 3),
            number(cep, 2),
            number(mrad(cep), 3),
            number(stats.map(|s| s.width), 2),
            number(stats.map(|s| s.height), 2),
            number(impact.map(|i| i.x), 2),
            number(impact.map(|i| i.y), 2),
            number(mrad(impact.map(|i| i.x)), 3),
            number(mrad(impact.map(|i| i.y)), 3),
            number(conditions.temperature_c, 1),
            number(conditions.powder_temperature_c, 1),
            number(conditions.pressure_hpa, 1),
            number(conditions.humidity_percent, 0),
            number(conditions.wind_speed_mps, 1),
            number(conditions.wind_clock, 1),
            velocity
                .as_ref()
                .map_or(String::new(), |v| v.shots.to_string()),
            number(velocity.as_ref().map(|v| v.average), 1),
            number(velocity.as_ref().and_then(|v| v.sd).map(|sd| sd.value), 2),
            number(velocity.as_ref().map(|v| v.extreme_spread), 1),
            field(&group.tags.join(" ")),
            field(&group.notes),
            group.archived.to_string(),
        ];
        csv.push_str(&row(&fields));
    }
    csv
}

/// One row per shot: where it is from the aim and from the group's centre
/// (of the shots that count), in mm and mrad, with its velocity.
pub fn shots_csv(groups: &[&Group], data: &Data) -> String {
    let mut csv = row(&SHOT_COLUMNS.map(str::to_owned));
    for group in groups {
        let Ok(measured) = crate::groups::measure_group(group, data) else {
            continue;
        };
        let (rifle, _, bullet) = names(group, data);
        let centre = measured.stats.as_ref().map(|stats| stats.centre);
        let velocities = crate::groups::details::velocity::shot_velocities(&group.marks);
        let mrad = |mm: Option<f64>| {
            mm.zip(group.distance_m)
                .map(|(mm, distance)| groups_core::angles::mrad(mm, distance))
        };
        for (index, shot) in measured.shots_mm.iter().enumerate() {
            let from_aim = measured.aim_mm.map(|aim| *shot - aim);
            let from_centre = centre.map(|centre| *shot - centre);
            let fields = vec![
                group.id.clone(),
                utc_text(group.taken_at),
                field(&rifle),
                field(&bullet),
                number(group.distance_m, 1),
                (index + 1).to_string(),
                measured.cold_bore.contains(&index).to_string(),
                measured.called_fliers.contains(&index).to_string(),
                measured.counted.contains(&index).to_string(),
                number(from_aim.map(|p| p.x), 2),
                number(from_aim.map(|p| p.y), 2),
                number(mrad(from_aim.map(|p| p.x)), 3),
                number(mrad(from_aim.map(|p| p.y)), 3),
                number(from_centre.map(|p| p.x), 2),
                number(from_centre.map(|p| p.y), 2),
                number(mrad(from_centre.map(|p| p.x)), 3),
                number(mrad(from_centre.map(|p| p.y)), 3),
                number(velocities.get(index).copied().flatten(), 1),
            ];
            csv.push_str(&row(&fields));
        }
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;
    use ballistics_store::{GridShot, Marks};

    fn grid_group() -> Group {
        let mut group = Group::on_grid(200.0, 200.0, 1_792_473_562_000);
        group.distance_m = Some(100.0);
        group.notes = "windy, gusts".into();
        if let Marks::Grid(marks) = &mut group.marks {
            for (x, y, velocity) in [
                (10.0, 0.0, Some(800.0)),
                (-10.0, 0.0, None),
                (0.0, 10.0, Some(810.0)),
            ] {
                marks.shots.push(GridShot {
                    x_mm: x,
                    y_mm: y,
                    cold_bore: false,
                    called_flier: false,
                    velocity_mps: velocity,
                });
            }
        }
        group
    }

    fn data_with(group: &Group) -> Data {
        let mut data = Data::default();
        data.groups.groups.push(group.clone());
        data
    }

    fn cells(line: &str) -> Vec<String> {
        let mut cells = Vec::new();
        let mut cell = String::new();
        let mut quoted = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    cell.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                ',' if !quoted => cells.push(std::mem::take(&mut cell)),
                c => cell.push(c),
            }
        }
        cells.push(cell);
        cells
    }

    #[test]
    fn text_that_a_spreadsheet_would_run_is_kept_as_text() {
        assert_eq!(field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(field("@SUM(1+1)"), "'@SUM(1+1)");
        assert_eq!(field("+1"), "'+1");
        assert_eq!(field("-2"), "'-2");
        assert_eq!(field("\tx"), "'\tx");
        assert_eq!(field("\rx"), "\"'\rx\"");
        assert_eq!(field("Lapua, 3rd"), "\"Lapua, 3rd\"");
        assert_eq!(field("plain"), "plain");

        let mut group = grid_group();
        group.notes = "=HYPERLINK(\"https://example.com\")".into();
        group.tags = vec!["@SUM(1+1)".into()];
        let data = data_with(&group);
        let csv = groups_csv(&[&group], &data);
        let row = cells(csv.lines().nth(1).unwrap());
        let at = |name: &str| &row[GROUP_COLUMNS.iter().position(|c| *c == name).unwrap()];
        assert_eq!(at("notes"), "'=HYPERLINK(\"https://example.com\")");
        assert_eq!(at("tags"), "'@SUM(1+1)");
    }

    #[test]
    fn the_group_csv_has_a_row_per_group_under_its_columns() {
        let group = grid_group();
        let data = data_with(&group);
        let csv = groups_csv(&[&group], &data);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], GROUP_COLUMNS.join(","));
        let row = cells(lines[1]);
        assert_eq!(row.len(), GROUP_COLUMNS.len());
        let at = |name: &str| &row[GROUP_COLUMNS.iter().position(|c| *c == name).unwrap()];
        assert_eq!(at("taken_at_utc"), "2026-10-20T05:19:22Z");
        assert_eq!(at("shots"), "3");
        assert_eq!(at("extreme_spread_mm"), "20.00");
        assert_eq!(at("extreme_spread_mrad"), "0.200");
        assert_eq!(at("velocity_count"), "2");
        assert_eq!(at("velocity_avg_mps"), "805.0");
        assert_eq!(at("notes"), "windy, gusts");
    }

    #[test]
    fn the_shot_csv_measures_from_the_aim_and_the_centre() {
        let group = grid_group();
        let data = data_with(&group);
        let csv = shots_csv(&[&group], &data);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], SHOT_COLUMNS.join(","));
        let third = cells(lines[3]);
        assert_eq!(third.len(), SHOT_COLUMNS.len());
        let at = |name: &str| &third[SHOT_COLUMNS.iter().position(|c| *c == name).unwrap()];
        assert_eq!(at("shot"), "3");
        assert_eq!(at("up_from_aim_mm"), "10.00");
        assert_eq!(at("up_from_aim_mrad"), "0.100");
        assert_eq!(at("up_from_centre_mm"), "6.67");
        assert_eq!(at("velocity_mps"), "810.0");
        let second = cells(lines[2]);
        assert_eq!(second[SHOT_COLUMNS.len() - 1], "");
    }

    #[test]
    fn a_grid_group_makes_a_png() {
        let group = grid_group();
        let data = data_with(&group);
        let png = group_png(&group, &data, None, &UnitPrefs::metric()).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        let picture = Pixmap::decode_png(&png).unwrap();
        assert_eq!(
            (picture.width(), picture.height()),
            (WIDTH, DRAWING + FOOTER)
        );
    }
}
