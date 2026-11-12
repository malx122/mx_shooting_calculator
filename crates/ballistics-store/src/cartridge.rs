//! A cartridge's outside, from the caliber's standard drawing, for drawing
//! it in 3D: the case's measurements as the C.I.P. (or SAAMI) drawing gives
//! them for the largest cartridge allowed, and a typical bullet seated to
//! the drawing's overall length. Inside it is solid; only the outside counts.
//!
//! All lengths are millimetres along the axis from the case head, and all
//! diameters millimetres. The letters in the comments are the C.I.P.
//! drawing's.

use serde::Deserialize;

/// Points per quarter circle in a rounded corner of the outline.
const FILLET_POINTS: usize = 4;
/// Points along the bullet's curved nose.
const NOSE_POINTS: usize = 16;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Cartridge {
    /// The drawing the case's measurements come from.
    pub source: String,
    pub case: CaseShape,
    pub bullet: BulletShape,
    /// From the case head to the bullet's tip (L6).
    pub overall_length: f64,
    /// The primer seen in the case head; none on a rimfire case.
    pub primer_diameter: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CaseShape {
    /// Rim diameter (R1).
    pub rim_diameter: f64,
    /// Rim thickness (R).
    pub rim_thickness: f64,
    /// The bevel round the back of the rim; none on a rimfire case.
    pub head_bevel: Option<HeadBevel>,
    /// The rounding of the rim's outer edges, back and front: a rimfire's
    /// rim is folded brass, drawn fully round (half its thickness).
    #[serde(default)]
    pub rim_radius: Option<f64>,
    /// The extractor groove ahead of the rim; none on a rimmed case.
    pub groove: Option<Groove>,
    /// From the head to where the body starts (E), and its diameter there
    /// (P1).
    pub body_start: f64,
    pub body_diameter: f64,
    /// The shoulder and neck of a bottleneck case; none on a straight one.
    pub shoulder: Option<Shoulder>,
    /// Diameter at the case mouth (H2).
    pub mouth_diameter: f64,
    /// Case length (L3).
    pub length: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HeadBevel {
    /// Its length along the axis (f).
    pub length: f64,
    /// Its angle to the head's face (β).
    pub degrees: f64,
}

impl HeadBevel {
    /// How far in from the rim's edge it starts on the head's face.
    pub fn inset(&self) -> f64 {
        self.length / self.degrees.to_radians().tan()
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Groove {
    /// Diameter in the groove (E1).
    pub diameter: f64,
    /// The angle of its front bevel, up to the body, to the axis (δ).
    pub bevel_degrees: f64,
    /// The shortest floor the drawing allows (e min): only to check the
    /// other measurements, which give the floor's length.
    pub floor_min: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Shoulder {
    /// From the head to where the shoulder starts (L1), and the body's
    /// diameter there (P2).
    pub start: f64,
    pub diameter: f64,
    /// From the head to where the neck starts (L2), and the neck's diameter
    /// there (H1).
    pub neck_start: f64,
    pub neck_diameter: f64,
    /// The rounding where the body meets the shoulder (r1) and where the
    /// shoulder meets the neck (r2).
    pub radius: f64,
    pub neck_radius: f64,
}

/// A bullet's outside, base to tip.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BulletShape {
    /// What kind of bullet, e.g. "55 gr FMJ boat tail".
    pub name: String,
    /// Where its measurements come from.
    pub source: String,
    pub diameter: f64,
    pub length: f64,
    /// The curved nose, from where it leaves the full diameter to the tip:
    /// a tangent ogive ending at the tip's flat (meplat).
    pub nose_length: f64,
    pub tip_diameter: f64,
    /// A tapered base; none for a flat base.
    pub boat_tail: Option<BoatTail>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BoatTail {
    pub length: f64,
    pub base_diameter: f64,
}

/// What a stretch of the outline is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Case,
    Primer,
    Bullet,
}

/// The cartridge's outline, to turn round its axis: points as [along the
/// axis, radius], from the middle of the case head to the middle of the
/// bullet's tip. Each stretch runs from the last point of the one before.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    pub start: [f64; 2],
    pub stretches: Vec<(Material, Vec<[f64; 2]>)>,
}

impl Cartridge {
    pub fn outline(&self) -> Outline {
        let case = &self.case;
        let mut stretches = Vec::new();
        let rim = case.rim_diameter / 2.0;

        // The head: the primer, then the brass out to the rim's bevel.
        if let Some(primer) = self.primer_diameter {
            stretches.push((Material::Primer, vec![[0.0, primer / 2.0]]));
        }
        let body = case.body_diameter / 2.0;
        // From where the head's middle ends, dropped once the rim is drawn.
        let mut brass = vec![[0.0, self.primer_diameter.map_or(0.0, |primer| primer / 2.0)]];
        match (&case.head_bevel, case.rim_radius) {
            (Some(bevel), _) => {
                brass.push([0.0, rim - bevel.inset()]);
                brass.push([bevel.length, rim]);
                brass.push([case.rim_thickness, rim]);
            }
            (None, Some(radius)) => {
                fillet(&mut brass, [0.0, rim], [case.rim_thickness, rim], radius);
                // Down the rim's front, to the groove or the body.
                let front = match &case.groove {
                    Some(groove) => groove.diameter / 2.0,
                    None => body.min(rim - radius),
                };
                fillet(
                    &mut brass,
                    [case.rim_thickness, rim],
                    [case.rim_thickness, front],
                    radius,
                );
            }
            (None, None) => {
                brass.push([0.0, rim]);
                brass.push([case.rim_thickness, rim]);
            }
        }
        brass.remove(0);

        // The extractor groove and the body.
        if let Some(groove) = &case.groove {
            let depth = groove.diameter / 2.0;
            let bevel = (body - depth) / groove.bevel_degrees.to_radians().tan();
            brass.push([case.rim_thickness, depth]);
            brass.push([case.body_start - bevel, depth]);
        } else if rim > body {
            brass.push([case.rim_thickness, body]);
        }
        brass.push([case.body_start, body]);

        // The shoulder and neck, with their rounded corners.
        let mouth = case.mouth_diameter / 2.0;
        if let Some(shoulder) = &case.shoulder {
            let start = [shoulder.start, shoulder.diameter / 2.0];
            let neck = [shoulder.neck_start, shoulder.neck_diameter / 2.0];
            let end = [case.length, mouth];
            fillet(&mut brass, start, neck, shoulder.radius);
            fillet(&mut brass, neck, end, shoulder.neck_radius);
        }
        brass.push([case.length, mouth]);

        // The bullet from where it leaves the case mouth.
        let bullet = self
            .bullet
            .outline(self.overall_length - self.bullet.length);
        let at_mouth = radius_at(&bullet, case.length);
        brass.push([case.length, at_mouth]);
        stretches.push((Material::Case, brass));

        let mut out = vec![];
        for &point in &bullet {
            if point[0] > case.length {
                out.push(point);
            }
        }
        stretches.push((Material::Bullet, out));
        Outline {
            start: [0.0, 0.0],
            stretches,
        }
    }
}

impl BulletShape {
    /// The bullet's outline with its base at `base`, from the base's rim to
    /// the middle of the tip.
    pub fn outline(&self, base: f64) -> Vec<[f64; 2]> {
        let radius = self.diameter / 2.0;
        let mut points = Vec::new();
        match &self.boat_tail {
            Some(tail) => {
                points.push([base, tail.base_diameter / 2.0]);
                points.push([base + tail.length, radius]);
            }
            None => points.push([base, radius]),
        }
        // A tangent ogive: an arc that leaves the side along it and passes
        // through the tip's edge.
        let tip = self.tip_diameter / 2.0;
        let start = base + self.length - self.nose_length;
        points.push([start, radius]);
        let drop = radius - tip;
        let arc = (self.nose_length.powi(2) + drop.powi(2)) / (2.0 * drop);
        for i in 1..=NOSE_POINTS {
            let along = self.nose_length * i as f64 / NOSE_POINTS as f64;
            let r = radius - arc + (arc * arc - along * along).max(0.0).sqrt();
            points.push([start + along, r]);
        }
        points.push([base + self.length, 0.0]);
        points
    }
}

impl Cartridge {
    /// The cartridge as a closed solid of triangles, standing on its head:
    /// millimetres, the axis along z, the head on z = 0. Each triangle's
    /// corners run anticlockwise seen from outside.
    pub fn solid(&self, slices: usize) -> Solid {
        assert!(slices >= 3, "a solid needs at least three sides");
        let outline = self.outline();
        let mut points: Vec<[f64; 2]> = vec![outline.start];
        for (_, stretch) in &outline.stretches {
            for &point in stretch {
                let last = *points.last().expect("starts with a point");
                if (point[0] - last[0]).abs() > 1e-9 || (point[1] - last[1]).abs() > 1e-9 {
                    points.push(point);
                }
            }
        }
        // A ring of corners round the axis at each outline point, or one
        // corner on the axis.
        let mut vertices = Vec::new();
        let mut rings = Vec::new();
        for &[along, radius] in &points {
            let first = vertices.len() as u32;
            if radius < 1e-9 {
                vertices.push([0.0, 0.0, along]);
                rings.push((first, 1));
            } else {
                for slice in 0..slices {
                    let angle = std::f64::consts::TAU * slice as f64 / slices as f64;
                    vertices.push([radius * angle.cos(), radius * angle.sin(), along]);
                }
                rings.push((first, slices as u32));
            }
        }
        let mut triangles = Vec::new();
        let slices = slices as u32;
        for pair in rings.windows(2) {
            let ((a, a_size), (b, b_size)) = (pair[0], pair[1]);
            for slice in 0..slices {
                let next = (slice + 1) % slices;
                let corner =
                    |first: u32, size: u32, at: u32| if size == 1 { first } else { first + at };
                let (a0, a1) = (corner(a, a_size, slice), corner(a, a_size, next));
                let (b0, b1) = (corner(b, b_size, slice), corner(b, b_size, next));
                if a0 != a1 {
                    triangles.push([a0, a1, b0]);
                }
                if b0 != b1 {
                    triangles.push([a1, b1, b0]);
                }
            }
        }
        Solid {
            vertices,
            triangles,
        }
    }
}

/// A closed surface of triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl Solid {
    /// As a binary STL file, in millimetres.
    pub fn stl(&self, name: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(84 + self.triangles.len() * 50);
        let mut header = [0u8; 80];
        for (byte, source) in header.iter_mut().zip(name.bytes()) {
            *byte = source;
        }
        out.extend_from_slice(&header);
        out.extend_from_slice(&(self.triangles.len() as u32).to_le_bytes());
        for triangle in &self.triangles {
            let [a, b, c] = triangle.map(|i| self.vertices[i as usize]);
            let (u, v) = (sub(b, a), sub(c, a));
            let normal = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let length = (normal[0].powi(2) + normal[1].powi(2) + normal[2].powi(2))
                .sqrt()
                .max(1e-12);
            for value in normal
                .map(|n| n / length)
                .iter()
                .chain(&a)
                .chain(&b)
                .chain(&c)
            {
                out.extend_from_slice(&(*value as f32).to_le_bytes());
            }
            out.extend_from_slice(&0u16.to_le_bytes());
        }
        out
    }

    /// The volume inside, mm³; negative if the triangles face inwards.
    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|triangle| {
                let [a, b, c] = triangle.map(|i| self.vertices[i as usize]);
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Adds the corner at `corner` of the line from the last point to it and
/// on to `next`, rounded with `radius`.
fn fillet(points: &mut Vec<[f64; 2]>, corner: [f64; 2], next: [f64; 2], radius: f64) {
    let last = *points.last().expect("the outline has started");
    let unit = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dy);
        [dx / length, dy / length]
    };
    let into = unit(last, corner);
    let out = unit(corner, next);
    let turn = (into[0] * out[1] - into[1] * out[0]).atan2(into[0] * out[0] + into[1] * out[1]);
    if radius <= 0.0 || turn.abs() < 1e-9 {
        points.push(corner);
        return;
    }
    let reach = radius * (turn.abs() / 2.0).tan();
    let from = [corner[0] - into[0] * reach, corner[1] - into[1] * reach];
    // The arc's centre is beside the incoming line, on the side it turns to.
    let side = turn.signum();
    let centre = [
        from[0] - into[1] * radius * side,
        from[1] + into[0] * radius * side,
    ];
    let begin = (from[1] - centre[1]).atan2(from[0] - centre[0]);
    let steps = ((turn.abs() / std::f64::consts::FRAC_PI_2) * FILLET_POINTS as f64)
        .ceil()
        .max(1.0) as usize;
    for i in 0..=steps {
        let angle = begin + turn * i as f64 / steps as f64;
        points.push([
            centre[0] + radius * angle.cos(),
            centre[1] + radius * angle.sin(),
        ]);
    }
}

/// The outline's radius at `along`, between its points.
fn radius_at(points: &[[f64; 2]], along: f64) -> f64 {
    for pair in points.windows(2) {
        let ([a0, a1], [b0, b1]) = (pair[0], pair[1]);
        if along >= a0 && along <= b0 {
            if b0 - a0 < 1e-12 {
                return a1.max(b1);
            }
            return a1 + (b1 - a1) * (along - a0) / (b0 - a0);
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;

    fn points(outline: &Outline) -> Vec<[f64; 2]> {
        std::iter::once(outline.start)
            .chain(
                outline
                    .stretches
                    .iter()
                    .flat_map(|(_, points)| points.iter().copied()),
            )
            .collect()
    }

    #[test]
    fn every_cartridge_is_a_closed_outline_true_to_its_drawing() {
        let catalog = Catalog::builtin();
        let mut drawn = 0;
        for caliber in &catalog.calibers {
            let Some(cartridge) = &caliber.cartridge else {
                continue;
            };
            drawn += 1;
            let name = &caliber.name;
            let case = &cartridge.case;
            let bullet = &cartridge.bullet;
            assert!(
                (bullet.diameter - caliber.bullet_diameter_mm).abs() < 0.03,
                "{name}: the bullet's diameter is the caliber's"
            );
            assert!(
                case.mouth_diameter - bullet.diameter >= 0.4,
                "{name}: the neck's wall round the bullet is at least 0.2 mm"
            );
            let seated = cartridge.overall_length - bullet.length;
            assert!(
                seated > case.body_start && seated < case.length,
                "{name}: the bullet's base is inside the case"
            );

            let outline = points(&cartridge.outline());
            assert_eq!(
                outline.first(),
                Some(&[0.0, 0.0]),
                "{name}: starts on the axis"
            );
            let last = outline.last().unwrap();
            assert_eq!(
                *last,
                [cartridge.overall_length, 0.0],
                "{name}: ends on the axis at the tip"
            );
            for pair in outline.windows(2) {
                assert!(
                    pair[1][0] >= pair[0][0] - 1e-9,
                    "{name}: goes forward: {pair:?}"
                );
                assert!(pair[1][1] >= 0.0, "{name}: never inside the axis: {pair:?}");
            }
            let widest = outline.iter().map(|p| p[1]).fold(0.0, f64::max);
            assert!(
                widest <= case.rim_diameter.max(case.body_diameter) / 2.0 + 1e-9,
                "{name}"
            );

            // The drawing's measurements are where the drawing puts them.
            let at = |along: f64| radius_at(&outline, along) * 2.0;
            assert!(
                // Just past E: on a rimfire case the rim steps down there.
                (at(case.body_start + 1e-6) - case.body_diameter).abs() < 1e-4,
                "{name}: P1 at E"
            );
            // The head: its bevel, f long at β to the face, then the rim.
            let head = outline
                .iter()
                .filter(|p| p[0] == 0.0)
                .map(|p| p[1])
                .fold(0.0, f64::max);
            let (inset, bevel_length) = case
                .head_bevel
                .as_ref()
                .map_or((0.0, 0.0), |bevel| (bevel.inset(), bevel.length));
            let rounded = case.rim_radius.unwrap_or(0.0);
            assert!(
                (head * 2.0 - (case.rim_diameter - 2.0 * (inset + rounded))).abs() < 1e-9,
                "{name}: the head's bevel or rounding"
            );
            assert!(
                rounded <= case.rim_thickness / 2.0 + 1e-9,
                "{name}: the rim's rounding fits it"
            );
            assert!(
                bevel_length < case.rim_thickness,
                "{name}: f inside the rim"
            );
            assert!(
                (at((bevel_length + case.rim_thickness) / 2.0) - case.rim_diameter).abs() < 1e-9,
                "{name}: R1 up to R"
            );
            if let Some(groove) = &case.groove {
                // The groove's floor (e), then its bevel at δ to the axis up
                // to P1 at E.
                let bevel = (case.body_diameter - groove.diameter)
                    / 2.0
                    / groove.bevel_degrees.to_radians().tan();
                let floor_end = case.body_start - bevel;
                // The measurements give the floor; the drawing's own e min
                // checks them (and how its angles were read).
                let floor = floor_end - case.rim_thickness;
                assert!(
                    floor >= groove.floor_min - 0.02 && floor < groove.floor_min + 0.1,
                    "{name}: the groove's floor {floor:.3} against e min {}",
                    groove.floor_min
                );
                assert!(
                    (at((case.rim_thickness + floor_end) / 2.0) - groove.diameter).abs() < 1e-9,
                    "{name}: E1 in the groove"
                );
                let half_way = floor_end + bevel / 2.0;
                assert!(
                    (at(half_way) - (groove.diameter + case.body_diameter) / 2.0).abs() < 1e-9,
                    "{name}: the groove's bevel"
                );
            }
            if let Some(shoulder) = &case.shoulder {
                // Half way along the shoulder it is straight, between the
                // rounded corners: with the body and the neck, that puts the
                // corners at (L1, P2) and (L2, H1).
                let middle = (shoulder.start + shoulder.neck_start) / 2.0;
                assert!(
                    (at(middle) - (shoulder.diameter + shoulder.neck_diameter) / 2.0).abs() < 1e-6,
                    "{name}: the shoulder's line"
                );
                // The corners are rounded, so measure just off them.
                let body_end = shoulder.start - shoulder.radius;
                let expected = case.body_diameter
                    + (shoulder.diameter - case.body_diameter) * (body_end - case.body_start)
                        / (shoulder.start - case.body_start);
                assert!(
                    (at(body_end) - expected).abs() < 1e-6,
                    "{name}: the body's taper"
                );
                let neck = shoulder.neck_start + shoulder.neck_radius;
                let expected = shoulder.neck_diameter
                    + (case.mouth_diameter - shoulder.neck_diameter) * (neck - shoulder.neck_start)
                        / (case.length - shoulder.neck_start);
                assert!(
                    (at(neck) - expected).abs() < 1e-6,
                    "{name}: the neck: {}",
                    at(neck)
                );
            }
        }
        assert!(drawn > 0);
    }

    #[test]
    fn the_solid_is_closed_faces_out_and_has_the_outlines_volume() {
        for caliber in &Catalog::builtin().calibers {
            let Some(cartridge) = &caliber.cartridge else {
                continue;
            };
            let name = &caliber.name;
            let solid = cartridge.solid(128);
            // Closed: each edge is used once each way.
            let mut edges = std::collections::HashMap::new();
            for t in &solid.triangles {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    assert_ne!(a, b, "{name}: no flat triangles");
                    *edges.entry((a, b)).or_insert(0) += 1;
                }
            }
            for (&(a, b), &count) in &edges {
                assert_eq!(count, 1, "{name}: edge {a}-{b} used once");
                assert_eq!(
                    edges.get(&(b, a)),
                    Some(&1),
                    "{name}: edge {a}-{b} has its twin"
                );
            }
            // The outline turned round its axis (Pappus), as cones' frustums.
            let points = points(&cartridge.outline());
            let exact: f64 = points
                .windows(2)
                .map(|p| {
                    let (h, r1, r2) = (p[1][0] - p[0][0], p[0][1], p[1][1]);
                    std::f64::consts::PI * h * (r1 * r1 + r1 * r2 + r2 * r2) / 3.0
                })
                .sum();
            let volume = solid.volume();
            assert!(volume > 0.0, "{name}: faces out");
            assert!(
                (volume / exact - 1.0).abs() < 0.001,
                "{name}: {volume} against {exact}"
            );
            // Stands on its head, as tall as the cartridge.
            let lowest = solid.vertices.iter().map(|v| v[2]).fold(f64::MAX, f64::min);
            let highest = solid.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
            assert_eq!(lowest, 0.0);
            assert!(
                (highest - cartridge.overall_length).abs() < 1e-9,
                "{name}: {highest}"
            );

            let stl = solid.stl(name);
            assert_eq!(stl.len(), 84 + 50 * solid.triangles.len());
            assert_eq!(
                u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize,
                solid.triangles.len()
            );
        }
    }

    #[test]
    fn a_rounded_corner_meets_both_lines() {
        let mut points = vec![[0.0, 4.5]];
        fillet(&mut points, [10.0, 4.5], [13.0, 3.0], 2.0);
        let first = points[1];
        let last = *points.last().unwrap();
        assert!((first[1] - 4.5).abs() < 1e-9 && first[0] < 10.0);
        // On the shoulder's line, past the corner.
        let slope = -1.5 / 3.0;
        assert!((last[1] - (4.5 + slope * (last[0] - 10.0))).abs() < 1e-9 && last[0] > 10.0);
        // The arc bulges no further out than the corner.
        assert!(points.iter().all(|p| p[1] <= 4.5 + 1e-9));
    }
}
