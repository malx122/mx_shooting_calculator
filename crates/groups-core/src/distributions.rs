//! The probability distributions the statistics need: χ², F, Student's t
//! and the normal, with the gamma and beta functions under them.
//!
//! Degrees of freedom may be any positive number, not only whole ones.
//! Every function gives `NaN` for an argument outside its domain (a
//! probability outside 0 to 1, degrees of freedom that are not positive),
//! and for shapes too large to converge (beyond about 10¹¹).
//! The CDFs and quantiles are good to 10 significant digits or better,
//! the tails included.

use std::f64::consts::PI;

const EPSILON: f64 = 1e-15;
const TINY: f64 = 1e-300;
const MAX_ITERATIONS: usize = 10_000;

/// Enough iterations for a series or continued fraction in a shape of
/// `size`, which need about √size of them.
fn iterations(size: f64) -> usize {
    MAX_ITERATIONS + (20.0 * size.sqrt()).min(1e7) as usize
}

/// Lanczos's approximation with g = 7 and nine terms.
const LANCZOS_G: f64 = 7.0;
const LANCZOS: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// ln Γ(x), the natural logarithm of the gamma function, for x > 0.
pub fn ln_gamma(x: f64) -> f64 {
    if x.is_nan() || x <= 0.0 {
        return f64::NAN;
    }
    if x < 0.5 {
        // Γ(x) Γ(1 − x) = π / sin(πx)
        return (PI / (PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let sum = LANCZOS
        .iter()
        .enumerate()
        .skip(1)
        .fold(LANCZOS[0], |sum, (i, &c)| sum + c / (x + i as f64));
    let t = x + LANCZOS_G + 0.5;
    0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + sum.ln()
}

/// ln B(a, b), the logarithm of the beta function.
fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// The regularised lower incomplete gamma function P(a, x) =
/// γ(a, x) / Γ(a), for a > 0 and x ≥ 0: the chance that a gamma variable
/// of shape `a` and scale 1 is below `x`.
pub fn incomplete_gamma_p(a: f64, x: f64) -> f64 {
    if a.is_nan() || x.is_nan() || a <= 0.0 || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        0.0
    } else if x.is_infinite() {
        1.0
    } else if x < a + 1.0 {
        gamma_series(a, x)
    } else {
        1.0 - gamma_continued_fraction(a, x)
    }
}

/// The regularised upper incomplete gamma function Q(a, x) = 1 − P(a, x),
/// computed directly so that small tails keep their precision.
pub fn incomplete_gamma_q(a: f64, x: f64) -> f64 {
    if a.is_nan() || x.is_nan() || a <= 0.0 || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        1.0
    } else if x.is_infinite() {
        0.0
    } else if x < a + 1.0 {
        1.0 - gamma_series(a, x)
    } else {
        gamma_continued_fraction(a, x)
    }
}

/// xᵃ e⁻ˣ / Γ(a).
fn gamma_front(a: f64, x: f64) -> f64 {
    (a * x.ln() - x - ln_gamma(a)).exp()
}

/// P(a, x) from its power series, for x < a + 1.
fn gamma_series(a: f64, x: f64) -> f64 {
    let mut shape = a;
    let mut term = 1.0 / a;
    let mut sum = term;
    for _ in 0..iterations(a) {
        shape += 1.0;
        term *= x / shape;
        sum += term;
        if term.abs() < sum.abs() * EPSILON {
            return sum * gamma_front(a, x);
        }
    }
    f64::NAN
}

/// Q(a, x) from its continued fraction (modified Lentz), for x ≥ a + 1.
fn gamma_continued_fraction(a: f64, x: f64) -> f64 {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..iterations(a) {
        let i = i as f64;
        let an = -i * (i - a);
        b += 2.0;
        d = nonzero(an * d + b);
        c = nonzero(b + an / c);
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPSILON {
            return gamma_front(a, x) * h;
        }
    }
    f64::NAN
}

fn nonzero(value: f64) -> f64 {
    if value.abs() < TINY { TINY } else { value }
}

/// The regularised incomplete beta function Iₓ(a, b), for 0 ≤ x ≤ 1 and
/// a, b > 0: the chance that a beta(a, b) variable is below `x`.
pub fn incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if x.is_nan() || a.is_nan() || b.is_nan() || a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    if !(0.0..=1.0).contains(&x) {
        return f64::NAN;
    }
    if x == 0.0 || x == 1.0 {
        return x;
    }
    let front = (a * x.ln() + b * (-x).ln_1p() - ln_beta(a, b)).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_continued_fraction(x, a, b) / a
    } else {
        1.0 - front * beta_continued_fraction(1.0 - x, b, a) / b
    }
}

/// The continued fraction of Iₓ(a, b) (modified Lentz).
fn beta_continued_fraction(x: f64, a: f64, b: f64) -> f64 {
    let mut c = 1.0;
    let mut d = 1.0 / nonzero(1.0 - (a + b) * x / (a + 1.0));
    let mut h = d;
    for m in 1..iterations(a.max(b)) {
        let m = m as f64;
        let even = m * (b - m) * x / ((a - 1.0 + 2.0 * m) * (a + 2.0 * m));
        d = 1.0 / nonzero(1.0 + even * d);
        c = nonzero(1.0 + even / c);
        h *= d * c;
        let odd = -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 1.0 + 2.0 * m));
        d = 1.0 / nonzero(1.0 + odd * d);
        c = nonzero(1.0 + odd / c);
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPSILON {
            return h;
        }
    }
    f64::NAN
}

/// The standard normal CDF Φ(z).
pub fn normal_cdf(z: f64) -> f64 {
    if z.is_nan() {
        return f64::NAN;
    }
    // P(Z > |z|) = Q(1/2, z²/2) / 2
    let tail = 0.5 * incomplete_gamma_q(0.5, 0.5 * z * z);
    if z < 0.0 { tail } else { 1.0 - tail }
}

/// The standard normal density.
pub fn normal_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * PI).sqrt()
}

/// The standard normal quantile Φ⁻¹(p): Acklam's rational approximation,
/// refined with Halley's method.
pub fn normal_quantile(p: f64) -> f64 {
    if p.is_nan() || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if p > 0.5 {
        // 1 − p is exact for p in [0.5, 1].
        return -normal_quantile(1.0 - p);
    }
    let mut x = acklam(p);
    // Below about 1e-305 the density underflows; Acklam alone is good to
    // 1e-9 there.
    if 0.5 * x * x > 700.0 {
        return x;
    }
    for _ in 0..3 {
        let error = normal_cdf(x) - p;
        let u = error * (2.0 * PI).sqrt() * (0.5 * x * x).exp();
        let step = u / (1.0 + 0.5 * x * u);
        x -= step;
        if step.abs() <= 1e-15 * x.abs() {
            break;
        }
    }
    x
}

/// Acklam's approximation of Φ⁻¹(p) for p ≤ 0.5, to about 1e-9.
fn acklam(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let polynomial = |coefficients: &[f64], x: f64| {
        coefficients
            .iter()
            .fold(0.0, |value, &coefficient| value * x + coefficient)
    };
    if p < 0.02425 {
        let q = (-2.0 * p.ln()).sqrt();
        polynomial(&C, q) / (polynomial(&D, q) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        polynomial(&A, r) * q / (polynomial(&B, r) * r + 1.0)
    }
}

/// The χ² CDF with `k` degrees of freedom.
pub fn chi_squared_cdf(x: f64, k: f64) -> f64 {
    if x.is_nan() || k.is_nan() || k <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    incomplete_gamma_p(0.5 * k, 0.5 * x)
}

fn chi_squared_survival(x: f64, k: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    incomplete_gamma_q(0.5 * k, 0.5 * x)
}

fn chi_squared_pdf(x: f64, k: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let half = 0.5 * k;
    ((half - 1.0) * x.ln() - 0.5 * x - half * 2f64.ln() - ln_gamma(half)).exp()
}

/// The χ² quantile: the x with `chi_squared_cdf(x, k) = p`.
pub fn chi_squared_quantile(p: f64, k: f64) -> f64 {
    if p.is_nan() || k.is_nan() || k <= 0.0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    // Wilson–Hilferty, or the small-x form P ≈ (x/2)^(k/2) / Γ(k/2 + 1).
    let h = 2.0 / (9.0 * k);
    let wilson_hilferty = k * (1.0 - h + normal_quantile(p) * h.sqrt()).powi(3);
    let start = if wilson_hilferty > 0.0 {
        wilson_hilferty
    } else {
        2.0 * (p * ln_gamma(0.5 * k + 1.0).exp()).powf(2.0 / k)
    };
    invert_positive(
        p,
        1.0 - p,
        start,
        |x| chi_squared_cdf(x, k),
        |x| chi_squared_survival(x, k),
        |x| chi_squared_pdf(x, k),
    )
}

/// The CDF of the F distribution with `d1` and `d2` degrees of freedom.
pub fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x.is_nan() || d1.is_nan() || d2.is_nan() || d1 <= 0.0 || d2 <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x.is_infinite() {
        return 1.0;
    }
    incomplete_beta(1.0 / (1.0 + d2 / d1 / x), 0.5 * d1, 0.5 * d2)
}

fn f_survival(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    let ratio = d2 / d1 / x;
    incomplete_beta(
        if ratio.is_infinite() {
            1.0
        } else {
            ratio / (1.0 + ratio)
        },
        0.5 * d2,
        0.5 * d1,
    )
}

fn f_pdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let ln_d1x = d1.ln() + x.ln();
    let ln_sum = ln_d1x.max(d2.ln()) + (-(ln_d1x - d2.ln()).abs()).exp().ln_1p();
    let ln = 0.5 * (d1 * ln_d1x + d2 * d2.ln() - (d1 + d2) * ln_sum)
        - x.ln()
        - ln_beta(0.5 * d1, 0.5 * d2);
    ln.exp()
}

/// The F quantile: the x with `f_cdf(x, d1, d2) = p`.
pub fn f_quantile(p: f64, d1: f64, d2: f64) -> f64 {
    if p.is_nan() || d1.is_nan() || d2.is_nan() || d1 <= 0.0 || d2 <= 0.0 {
        return f64::NAN;
    }
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    invert_positive(
        p,
        1.0 - p,
        1.0,
        |x| f_cdf(x, d1, d2),
        |x| f_survival(x, d1, d2),
        |x| f_pdf(x, d1, d2),
    )
}

/// The CDF of Student's t distribution with `df` degrees of freedom.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    if t.is_nan() || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if t.is_infinite() {
        return if t > 0.0 { 1.0 } else { 0.0 };
    }
    let t2 = t * t;
    if t2 < 1.0 {
        // P(|T| < |t|) = I_{t²/(df + t²)}(1/2, df/2)
        let inner = 0.5 * incomplete_beta(t2 / (df + t2), 0.5, 0.5 * df);
        if t < 0.0 { 0.5 - inner } else { 0.5 + inner }
    } else {
        // P(T > |t|) = I_{df/(df + t²)}(df/2, 1/2) / 2
        let tail = 0.5 * incomplete_beta(df / (df + t2), 0.5 * df, 0.5);
        if t < 0.0 { tail } else { 1.0 - tail }
    }
}

fn t_pdf(t: f64, df: f64) -> f64 {
    let ln = ln_gamma(0.5 * (df + 1.0))
        - ln_gamma(0.5 * df)
        - 0.5 * (df * PI).ln()
        - 0.5 * (df + 1.0) * (t * t / df).ln_1p();
    ln.exp()
}

/// The t quantile: the t with `t_cdf(t, df) = p`.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if p.is_nan() || df.is_nan() || df <= 0.0 || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -invert_positive(
            1.0 - p,
            p,
            start_t(1.0 - p, df),
            |t| t_cdf(t, df),
            |t| t_cdf(-t, df),
            |t| t_pdf(t, df),
        );
    }
    invert_positive(
        p,
        1.0 - p,
        start_t(p, df),
        |t| t_cdf(t, df),
        |t| t_cdf(-t, df),
        |t| t_pdf(t, df),
    )
}

/// A first guess at the t quantile for p > 0.5 (Cornish–Fisher, first term).
fn start_t(p: f64, df: f64) -> f64 {
    let z = normal_quantile(p);
    (z + (z * z * z + z) / (4.0 * df)).max(1e-3)
}

/// Finds the x > 0 where `cdf(x) = p` for a continuous distribution on
/// (0, ∞), by Newton's method kept inside a shrinking bracket. Above the
/// median it matches the upper tail `survival(x) = q` instead, where `q`
/// is 1 − p given at full precision, so quantiles near 1 keep theirs.
fn invert_positive(
    p: f64,
    q: f64,
    start: f64,
    cdf: impl Fn(f64) -> f64,
    survival: impl Fn(f64) -> f64,
    pdf: impl Fn(f64) -> f64,
) -> f64 {
    let upper = p > 0.5;
    // Increasing in x, zero at the quantile.
    let miss = |x: f64| if upper { q - survival(x) } else { cdf(x) - p };
    let mut low = 0.0;
    let mut high = if start.is_finite() && start > 0.0 {
        start
    } else {
        1.0
    };
    for _ in 0..2_000 {
        if miss(high) >= 0.0 {
            break;
        }
        low = high;
        high *= 2.0;
    }
    let mut x = high;
    for _ in 0..200 {
        let error = miss(x);
        if error == 0.0 {
            return x;
        }
        if error < 0.0 {
            low = x;
        } else {
            high = x;
        }
        let newton = x - error / pdf(x);
        let next = if newton.is_finite() && newton > low && newton < high {
            newton
        } else {
            0.5 * (low + high)
        };
        if (next - x).abs() <= 1e-15 * next.abs() || high - low <= 1e-15 * high {
            return next;
        }
        x = next;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64, relative: f64) {
        assert!(
            (actual - expected).abs() <= relative * expected.abs().max(1e-300),
            "{actual} is not within {relative:e} of {expected}"
        );
    }

    /// Q(n, x) for whole n is the Poisson chance of fewer than n events.
    fn poisson_below(n: u32, x: f64) -> f64 {
        let mut term = (-x).exp();
        let mut sum = 0.0;
        for j in 0..n {
            if j > 0 {
                term *= x / j as f64;
            }
            sum += term;
        }
        sum
    }

    #[test]
    fn ln_gamma_matches_known_values() {
        close(ln_gamma(0.5), 0.572_364_942_924_700, 1e-13);
        close(ln_gamma(1.5), -0.120_782_237_635_245, 1e-12);
        close(ln_gamma(0.1), 2.252_712_651_734_206, 1e-13);
        close(ln_gamma(5.0), 24f64.ln(), 1e-14);
        close(ln_gamma(10.0), 362_880f64.ln(), 1e-14);
        close(ln_gamma(100.0), 359.134_205_369_575_4, 1e-14);
        assert!(ln_gamma(1.0).abs() < 1e-14);
        assert!(ln_gamma(2.0).abs() < 1e-14);
        assert!(ln_gamma(0.0).is_nan());
        assert!(ln_gamma(-1.0).is_nan());
    }

    #[test]
    fn incomplete_gamma_matches_closed_forms() {
        for x in [0.01, 0.3, 1.0, 2.5, 7.0, 30.0] {
            close(incomplete_gamma_p(1.0, x), 1.0 - (-x).exp(), 1e-13);
            close(incomplete_gamma_q(1.0, x), (-x).exp(), 1e-12);
        }
        // P(1/2, x) = erf(√x); erf(1) and erf(2).
        close(incomplete_gamma_p(0.5, 1.0), 0.842_700_792_949_715, 1e-13);
        close(incomplete_gamma_p(0.5, 4.0), 0.995_322_265_018_953, 1e-13);
        for n in [2, 3, 10, 40] {
            for x in [0.5, 2.0, 5.0, 12.0, 60.0] {
                close(incomplete_gamma_q(n as f64, x), poisson_below(n, x), 1e-11);
            }
        }
        assert_eq!(incomplete_gamma_p(2.0, 0.0), 0.0);
        assert_eq!(incomplete_gamma_q(2.0, 0.0), 1.0);
        // Huge shapes need far more terms; near the mean P is a shade
        // under ½ (Edgeworth: Φ(z) + φ(z)/(3√a)).
        close(incomplete_gamma_p(1e7, 1e7 - 0.5), 0.499_979, 2e-5);
        close(incomplete_gamma_p(1e9, 1e9 - 1.0), 0.499_992, 2e-5);
        assert!(incomplete_gamma_p(0.0, 1.0).is_nan());
        assert!(incomplete_gamma_p(1.0, -1.0).is_nan());
    }

    #[test]
    fn incomplete_beta_matches_closed_forms() {
        // I_0.4(2, 3) = Σ_{j=2..4} C(4, j) 0.4^j 0.6^(4−j)
        close(incomplete_beta(0.4, 2.0, 3.0), 0.5248, 1e-13);
        for x in [0.001, 0.1, 0.37, 0.5, 0.9, 0.999] {
            close(incomplete_beta(x, 3.5, 1.0), x.powf(3.5), 1e-12);
            close(
                incomplete_beta(x, 1.0, 2.5),
                1.0 - (1.0 - x).powf(2.5),
                1e-12,
            );
            close(
                incomplete_beta(x, 0.5, 0.5),
                2.0 / PI * x.sqrt().asin(),
                1e-12,
            );
        }
        for a in [0.5, 3.0, 17.0, 250.0] {
            close(incomplete_beta(0.5, a, a), 0.5, 1e-12);
        }
        assert_eq!(incomplete_beta(0.0, 2.0, 3.0), 0.0);
        assert_eq!(incomplete_beta(1.0, 2.0, 3.0), 1.0);
        assert!(incomplete_beta(1.5, 2.0, 3.0).is_nan());
    }

    #[test]
    fn normal_matches_tables() {
        close(normal_cdf(0.0), 0.5, 1e-15);
        close(normal_cdf(1.0), 0.841_344_746_068_543, 1e-13);
        close(normal_cdf(1.96), 0.975_002_104_851_780, 1e-13);
        close(normal_cdf(-3.0), 1.349_898_031_630_095e-3, 1e-12);
        close(normal_cdf(-6.0), 9.865_876_450_376_98e-10, 1e-11);
        close(normal_quantile(0.975), 1.959_963_984_540_054, 1e-13);
        close(normal_quantile(0.995), 2.575_829_303_548_901, 1e-13);
        close(normal_quantile(0.9), 1.281_551_565_544_601, 1e-13);
        close(normal_quantile(0.999), 3.090_232_306_167_814, 1e-13);
        close(normal_quantile(1e-10), -6.361_340_902_404_056, 1e-12);
        close(normal_quantile(0.3), -0.524_400_512_708_041, 1e-12);
        assert_eq!(normal_quantile(0.5), 0.0);
        assert_eq!(normal_quantile(0.0), f64::NEG_INFINITY);
        // Subnormal probabilities, where the density underflows.
        let tiny = normal_quantile(1e-320);
        assert!(tiny < normal_quantile(1e-310) && tiny > -39.0);
        assert!(normal_quantile(5e-324).is_finite());
        assert!(normal_quantile(1.5).is_nan());
        for p in [1e-12, 0.001, 0.02, 0.2, 0.5, 0.77, 0.999_999] {
            close(normal_cdf(normal_quantile(p)), p, 1e-12);
        }
    }

    #[test]
    fn chi_squared_matches_tables() {
        // Even degrees of freedom have a closed form: Q(k/2, x/2).
        for k in [2u32, 4, 8, 20] {
            for x in [0.3, 2.0, 9.0, 31.0] {
                close(
                    1.0 - chi_squared_cdf(x, k as f64),
                    poisson_below(k / 2, x / 2.0),
                    1e-10,
                );
            }
        }
        // χ²(2) is exponential: the quantile is −2 ln(1 − p).
        close(chi_squared_quantile(0.5, 2.0), 2.0 * 2f64.ln(), 1e-13);
        // χ²(1) is Z²: the quantile is Φ⁻¹((1 + p)/2)².
        close(
            chi_squared_quantile(0.95, 1.0),
            1.959_963_984_540_054f64.powi(2),
            1e-12,
        );
        // Critical values as tabulated in the NIST/SEMATECH e-Handbook
        // (1.3.6.7.4), to more digits.
        close(chi_squared_quantile(0.99, 1.0), 6.634_896_601, 1e-9);
        close(chi_squared_quantile(0.025, 4.0), 0.484_418_6, 1e-6);
        close(chi_squared_quantile(0.975, 4.0), 11.143_287, 1e-6);
        close(chi_squared_quantile(0.025, 8.0), 2.179_730_7, 1e-6);
        close(chi_squared_quantile(0.975, 8.0), 17.534_546, 1e-6);
        close(chi_squared_quantile(0.05, 10.0), 3.940_299, 1e-6);
        close(chi_squared_quantile(0.95, 10.0), 18.307_038, 1e-6);
        close(chi_squared_quantile(0.99, 30.0), 50.892_181, 1e-6);
        close(chi_squared_quantile(0.05, 100.0), 77.929_465, 1e-6);
        close(chi_squared_quantile(0.95, 100.0), 124.342_113, 1e-6);
        close(chi_squared_quantile(0.025, 1.0), 9.820_691e-4, 1e-5);
        for k in [0.7, 1.0, 1.74, 3.0, 18.0, 98.0, 2_000.0] {
            for p in [1e-9, 0.025, 0.5, 0.975, 1.0 - 1e-9] {
                let x = chi_squared_quantile(p, k);
                if p < 0.5 {
                    close(chi_squared_cdf(x, k), p, 1e-10);
                } else {
                    close(chi_squared_survival(x, k), 1.0 - p, 1e-7);
                }
            }
        }
        assert_eq!(chi_squared_quantile(0.0, 3.0), 0.0);
        assert!(chi_squared_quantile(0.5, 0.0).is_nan());
    }

    #[test]
    fn student_t_matches_tables() {
        // t(1) is Cauchy, t(2) has a closed form.
        for p in [0.6, 0.9, 0.975, 0.999] {
            close(t_quantile(p, 1.0), (PI * (p - 0.5)).tan(), 1e-12);
            let two = (2.0 * p - 1.0) / (2.0 * p * (1.0 - p)).sqrt();
            close(t_quantile(p, 2.0), two, 1e-12);
        }
        // Critical values as tabulated in the NIST/SEMATECH e-Handbook
        // (1.3.6.7.2), to more digits.
        close(t_quantile(0.975, 1.0), 12.706_204_736, 1e-9);
        close(t_quantile(0.975, 3.0), 3.182_446_305, 1e-9);
        close(t_quantile(0.975, 5.0), 2.570_581_836, 1e-9);
        close(t_quantile(0.975, 10.0), 2.228_138_852, 1e-9);
        close(t_quantile(0.95, 10.0), 1.812_461_123, 1e-9);
        close(t_quantile(0.995, 20.0), 2.845_339_710, 1e-9);
        close(t_quantile(0.975, 30.0), 2.042_272_456, 1e-9);
        close(t_quantile(0.975, 100.0), 1.983_971_519, 1e-9);
        close(t_quantile(0.025, 10.0), -2.228_138_852, 1e-9);
        close(t_cdf(2.228_138_852, 10.0), 0.975, 1e-9);
        // t(4): F(t) = 1/2 + (3/8) u (1 − u²/12) with u = t / √(1 + t²/4).
        for t in [-3.0, 0.1, 0.8, 6.0] {
            let u: f64 = t / (1.0 + t * t / 4.0_f64).sqrt();
            close(t_cdf(t, 4.0), 0.5 + 0.375 * u * (1.0 - u * u / 12.0), 1e-12);
        }
        assert_eq!(t_quantile(0.5, 7.0), 0.0);
        for df in [0.5, 1.0, 3.3, 12.0, 1e4] {
            for p in [1e-8, 0.01, 0.3, 0.51, 0.9, 0.99999] {
                close(t_cdf(t_quantile(p, df), df), p, 1e-9);
            }
        }
    }

    #[test]
    fn f_matches_tables() {
        // F(2, 2) has CDF x / (1 + x); F(1, ν) is t(ν)².
        for x in [0.01, 0.5, 1.0, 7.0, 300.0] {
            close(f_cdf(x, 2.0, 2.0), x / (1.0 + x), 1e-12);
        }
        // Where d₁x overflows or underflows.
        assert_eq!(f_cdf(1e308, 10.0, 10.0), 1.0);
        assert_eq!(f_cdf(5e-324, 10.0, 10.0), 0.0);
        assert_eq!(f_survival(1e308, 10.0, 10.0), 0.0);
        close(f_quantile(0.95, 1.0, 10.0), 2.228_138_852f64.powi(2), 1e-8);
        // Critical values as tabulated in the NIST/SEMATECH e-Handbook
        // (1.3.6.7.3), to more digits.
        close(f_quantile(0.95, 1.0, 1.0), 161.447_638_8, 1e-8);
        close(f_quantile(0.95, 4.0, 4.0), 6.388_232_6, 1e-6);
        close(f_quantile(0.975, 4.0, 4.0), 9.604_530, 1e-6);
        close(f_quantile(0.95, 8.0, 8.0), 3.438_101, 1e-6);
        close(f_quantile(0.975, 9.0, 9.0), 4.025_994, 1e-6);
        close(f_quantile(0.95, 10.0, 20.0), 2.347_878, 1e-6);
        close(f_quantile(0.95, 5.0, 10.0), 3.325_835, 1e-6);
        close(f_quantile(0.99, 5.0, 10.0), 5.636_326, 1e-6);
        close(f_quantile(0.99, 10.0, 10.0), 4.849_147, 1e-6);
        // The lower quantile is the reciprocal of the upper one with the
        // degrees of freedom swapped.
        close(
            f_quantile(0.05, 10.0, 20.0),
            1.0 / f_quantile(0.95, 20.0, 10.0),
            1e-11,
        );
        for (d1, d2) in [(1.0, 1.0), (3.0, 7.5), (8.0, 8.0), (40.0, 120.0)] {
            for p in [1e-6, 0.025, 0.5, 0.975, 0.999_99] {
                close(f_cdf(f_quantile(p, d1, d2), d1, d2), p, 1e-9);
            }
        }
    }
}
