//! The grid as a homography from pixels to grid units, fitted to points
//! seen on its lines.

/// A 3 × 3 matrix acting on `[x, y, 1]`.
pub(super) type Matrix = [[f64; 3]; 3];

/// A homography from pixels to grid units `(u, v)`: line `n` of the first
/// family is `u = n`, line `m` of the second `v = m`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Model {
    /// Normalised pixels (`(p − centre) / scale`) to grid units.
    to_grid: Matrix,
    /// Grid units to normalised pixels.
    to_norm: Matrix,
    centre: [f64; 2],
    scale: f64,
}

/// A point found on a grid line.
#[derive(Clone, Copy, Debug)]
pub(super) struct Seen {
    pub at: [f64; 2],
    /// 0 for the lines `u = n`, 1 for `v = m`.
    pub family: usize,
    pub index: f64,
}

pub(super) fn apply(m: &Matrix, x: f64, y: f64) -> Option<[f64; 2]> {
    let w = m[2][0] * x + m[2][1] * y + m[2][2];
    if w.abs() < 1e-12 || !w.is_finite() {
        return None;
    }
    Some([
        (m[0][0] * x + m[0][1] * y + m[0][2]) / w,
        (m[1][0] * x + m[1][1] * y + m[1][2]) / w,
    ])
}

fn invert(m: &Matrix) -> Option<Matrix> {
    let cofactor =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let adjugate = [
        [
            cofactor(1, 2, 1, 2),
            -cofactor(0, 2, 1, 2),
            cofactor(0, 1, 1, 2),
        ],
        [
            -cofactor(1, 2, 0, 2),
            cofactor(0, 2, 0, 2),
            -cofactor(0, 1, 0, 2),
        ],
        [
            cofactor(1, 2, 0, 1),
            -cofactor(0, 2, 0, 1),
            cofactor(0, 1, 0, 1),
        ],
    ];
    let det = m[0][0] * adjugate[0][0] + m[0][1] * adjugate[1][0] + m[0][2] * adjugate[2][0];
    if det.abs() < 1e-15 || !det.is_finite() {
        return None;
    }
    Some(adjugate.map(|row| row.map(|value| value / det)))
}

/// Solves `a x = b` by Gaussian elimination with partial pivoting.
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for column in 0..N {
        let pivot =
            (column..N).max_by(|&i, &j| a[i][column].abs().total_cmp(&a[j][column].abs()))?;
        if a[pivot][column].abs() < 1e-14 {
            return None;
        }
        a.swap(column, pivot);
        b.swap(column, pivot);
        for row in column + 1..N {
            let factor = a[row][column] / a[column][column];
            for k in column..N {
                a[row][k] -= factor * a[column][k];
            }
            b[row] -= factor * b[column];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let sum: f64 = (row + 1..N).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

impl Model {
    pub fn new(to_grid: Matrix, centre: [f64; 2], scale: f64) -> Option<Model> {
        let to_grid = to_grid.map(|row| row.map(|v| v / to_grid[2][2]));
        Some(Model {
            to_norm: invert(&to_grid)?,
            to_grid,
            centre,
            scale,
        })
    }

    /// Parallel lines: family `f` has unit normal `normals[f]`, lines
    /// `periods[f]` px apart, line 0 `offsets[f]` px from `centre` along
    /// the normal.
    pub fn parallel(
        normals: [[f64; 2]; 2],
        periods: [f64; 2],
        offsets: [f64; 2],
        centre: [f64; 2],
        scale: f64,
    ) -> Option<Model> {
        let row = |f: usize| {
            [
                scale * normals[f][0] / periods[f],
                scale * normals[f][1] / periods[f],
                -offsets[f] / periods[f],
            ]
        };
        Model::new([row(0), row(1), [0.0, 0.0, 1.0]], centre, scale)
    }

    fn norm(&self, p: [f64; 2]) -> [f64; 2] {
        [
            (p[0] - self.centre[0]) / self.scale,
            (p[1] - self.centre[1]) / self.scale,
        ]
    }

    pub fn to_grid(&self, p: [f64; 2]) -> Option<[f64; 2]> {
        let [x, y] = self.norm(p);
        apply(&self.to_grid, x, y)
    }

    pub fn to_px(&self, [u, v]: [f64; 2]) -> Option<[f64; 2]> {
        let [x, y] = apply(&self.to_norm, u, v)?;
        Some([
            self.centre[0] + self.scale * x,
            self.centre[1] + self.scale * y,
        ])
    }

    /// The gradient of grid coordinate `family` at `p`, per pixel: normal
    /// to the family's lines, as long as one over their spacing.
    pub fn gradient(&self, family: usize, p: [f64; 2]) -> [f64; 2] {
        let [x, y] = self.norm(p);
        let row = self.to_grid[family];
        let w = self.to_grid[2];
        let a = row[0] * x + row[1] * y + row[2];
        let d = w[0] * x + w[1] * y + w[2];
        let k = 1.0 / (d * d * self.scale);
        [(row[0] * d - a * w[0]) * k, (row[1] * d - a * w[1]) * k]
    }

    /// The distance between neighbouring lines of `family` at `p`, px.
    pub fn spacing(&self, family: usize, p: [f64; 2]) -> f64 {
        let [gx, gy] = self.gradient(family, p);
        1.0 / gx.hypot(gy)
    }

    /// The projective denominator at `p`: 1 where the grid looks as it
    /// does at the centre, more where it looks smaller.
    pub fn depth(&self, p: [f64; 2]) -> f64 {
        let [x, y] = self.norm(p);
        let w = self.to_grid[2];
        w[0] * x + w[1] * y + w[2]
    }

    /// Fits the model to points seen on the lines, each with a weight, by
    /// least squares on `u·w(p) = n` (linear in the homography). `regular`
    /// pulls the perspective terms towards none.
    pub fn fit(&self, seen: &[Seen], weights: &[f64], regular: f64) -> Option<Model> {
        let mut a = [[0.0f64; 8]; 8];
        let mut b = [0.0f64; 8];
        let mut total = 0.0;
        for (point, &weight) in seen.iter().zip(weights) {
            if weight <= 0.0 {
                continue;
            }
            let [x, y] = self.norm(point.at);
            let n = point.index;
            let mut row = [0.0; 8];
            let base = 3 * point.family;
            row[base] = x;
            row[base + 1] = y;
            row[base + 2] = 1.0;
            row[6] = -n * x;
            row[7] = -n * y;
            for i in 0..8 {
                if row[i] == 0.0 {
                    continue;
                }
                for j in 0..8 {
                    a[i][j] += weight * row[i] * row[j];
                }
                b[i] += weight * row[i] * n;
            }
            total += weight;
        }
        if total <= 0.0 {
            return None;
        }
        a[6][6] += regular * total;
        a[7][7] += regular * total;
        let t = solve(a, b)?;
        Model::new(
            [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], 1.0]],
            self.centre,
            self.scale,
        )
    }

    /// The same grid counted in steps `1 / k` of the current ones, with
    /// the old line `r[f]` of each family as the new line 0 (`u' = k(u − r)`).
    pub fn rescaled(&self, k: f64, r: [f64; 2]) -> Option<Model> {
        let m = &self.to_grid;
        let row = |f: usize| [0, 1, 2].map(|i| k * (m[f][i] - r[f] * m[2][i]));
        Model::new([row(0), row(1), m[2]], self.centre, self.scale)
    }

    /// The same model with family `f`'s coordinate replaced by
    /// `scale · u + shift`.
    pub fn adjusted(&self, f: usize, scale: f64, shift: f64) -> Option<Model> {
        let mut m = self.to_grid;
        m[f] = [0, 1, 2].map(|i| scale * m[f][i] + shift * m[2][i]);
        Model::new(m, self.centre, self.scale)
    }

    /// The model in another pixel frame: `p' = origin + factor · p`.
    pub fn to_frame(&self, origin: [f64; 2], factor: f64) -> Model {
        Model {
            to_grid: self.to_grid,
            to_norm: self.to_norm,
            centre: [
                origin[0] + factor * self.centre[0],
                origin[1] + factor * self.centre[1],
            ],
            scale: self.scale * factor,
        }
    }
}
