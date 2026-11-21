//! How muzzle velocity changes with powder temperature, from sessions
//! shot at different temperatures.
//!
//! A straight line v = a + b T is fitted by weighted least squares to the
//! sessions' mean velocities, each weighted by its number of shots (as
//! if every shot were measured: one SD for all, since a session's own SD
//! from a few shots is too uncertain to weight by). The slope b is in
//! m/s per °C, the unit MXSC stores a load's sensitivity in.

use crate::error::{AnalysisError, t95};
use crate::group::{Estimate, Interval, StatsError};
use crate::velocity::is_plausible;

/// The fewest sessions a fit needs.
pub const MIN_SESSIONS: usize = 3;

/// The temperature span, °C, the sessions need between them.
pub const MIN_SPAN_C: f64 = 10.0;

/// The temperature the percentage sensitivity is relative to, °C.
pub const REFERENCE_C: f64 = 15.0;

/// One session at the chronograph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemperatureSession {
    /// The powder's temperature, °C.
    pub temperature_c: f64,
    /// The session's mean muzzle velocity, m/s.
    pub mean_mps: f64,
    pub shots: usize,
    /// The session's sample SD (n − 1), m/s; ignored for a single shot.
    pub sd_mps: f64,
}

/// The fitted sensitivity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemperatureSensitivity {
    pub sessions: usize,
    pub shots: usize,
    /// Warmest less coldest session, °C.
    pub span_c: f64,
    /// The change of velocity per °C, m/s, with its 95% range from
    /// Student's t (see `degrees_of_freedom`).
    pub per_degree: Estimate,
    /// The slope's standard error, m/s per °C: from the sessions'
    /// scatter about the line, or from their shots alone
    /// (`within_standard_error`) when that is larger.
    pub standard_error: f64,
    /// For Student's t: sessions − 2 with the error from the scatter
    /// about the line, Σ(nᵢ − 1) with the error from the shots.
    pub degrees_of_freedom: f64,
    /// The slope's standard error from the shots alone, from the pooled
    /// SD; `None` when every session is one shot. A `standard_error` well
    /// above it means the sessions differ by more than temperature and
    /// shot-to-shot scatter explain (another lot, chronograph placement).
    pub within_standard_error: Option<f64>,
    /// The line's velocity at [`REFERENCE_C`], m/s.
    pub velocity_at_reference: f64,
    /// `per_degree` as a percentage of `velocity_at_reference` per °C.
    pub percent_per_degree: Estimate,
    /// The shot-weighted mean temperature, °C, through which the line is
    /// best known.
    pub mean_temperature_c: f64,
    /// The shot-weighted mean velocity, m/s.
    pub mean_velocity_mps: f64,
}

impl TemperatureSensitivity {
    /// Fits the sessions: at least [`MIN_SESSIONS`] spanning at least
    /// [`MIN_SPAN_C`].
    pub fn new(sessions: &[TemperatureSession]) -> Result<Self, AnalysisError> {
        if sessions.is_empty() {
            return Err(AnalysisError::Empty);
        }
        for (index, session) in sessions.iter().enumerate() {
            let error = |error: StatsError| Err(AnalysisError::Input { index, error });
            if session.shots == 0 {
                return error(StatsError::NoShots);
            }
            if !session.temperature_c.is_finite()
                || !session.mean_mps.is_finite()
                || (session.shots >= 2 && !(session.sd_mps.is_finite() && session.sd_mps >= 0.0))
            {
                return error(StatsError::NotFinite);
            }
            if !is_plausible(session.mean_mps) {
                return Err(AnalysisError::ImplausibleMean {
                    index,
                    mps: session.mean_mps,
                });
            }
        }
        if sessions.len() < MIN_SESSIONS {
            return Err(AnalysisError::TooFew {
                what: "sessions",
                need: MIN_SESSIONS,
                have: sessions.len(),
            });
        }
        let (coldest, warmest) = sessions
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), s| {
                (low.min(s.temperature_c), high.max(s.temperature_c))
            });
        let span_c = warmest - coldest;
        if span_c.is_nan() || span_c < MIN_SPAN_C {
            return Err(AnalysisError::TooNarrow {
                need_c: MIN_SPAN_C,
                have_c: span_c,
            });
        }
        let shots: usize = sessions.iter().map(|s| s.shots).sum();
        let total = shots as f64;
        let weight = |s: &TemperatureSession| s.shots as f64;
        let mean_t = sessions
            .iter()
            .map(|s| weight(s) * s.temperature_c)
            .sum::<f64>()
            / total;
        let mean_v = sessions.iter().map(|s| weight(s) * s.mean_mps).sum::<f64>() / total;
        let (mut tt, mut tv) = (0.0, 0.0);
        for s in sessions {
            let dt = s.temperature_c - mean_t;
            tt += weight(s) * dt * dt;
            tv += weight(s) * dt * (s.mean_mps - mean_v);
        }
        let slope = tv / tt;
        let residual: f64 = sessions
            .iter()
            .map(|s| weight(s) * (s.mean_mps - mean_v - slope * (s.temperature_c - mean_t)).powi(2))
            .sum();
        let degrees = (sessions.len() - 2) as f64;
        let scatter_error = (residual / degrees / tt).sqrt();
        let within_degrees: usize = sessions.iter().map(|s| s.shots - 1).sum();
        let within_standard_error = (within_degrees > 0).then(|| {
            let squares: f64 = sessions
                .iter()
                .filter(|s| s.shots >= 2)
                .map(|s| (s.shots - 1) as f64 * s.sd_mps * s.sd_mps)
                .sum();
            (squares / within_degrees as f64 / tt).sqrt()
        });
        // The larger of the two errors, with its own degrees of freedom.
        let (standard_error, degrees) = match within_standard_error {
            Some(within) if within >= scatter_error => (within, within_degrees as f64),
            _ => (scatter_error, degrees),
        };
        let half = t95(degrees) * standard_error;
        let velocity_at_reference = mean_v + slope * (REFERENCE_C - mean_t);
        let per_degree = Estimate {
            value: slope,
            range: Interval {
                low: slope - half,
                high: slope + half,
            },
        };
        let percent = 100.0 / velocity_at_reference;
        let fitted = TemperatureSensitivity {
            sessions: sessions.len(),
            shots,
            span_c,
            per_degree,
            standard_error,
            degrees_of_freedom: degrees,
            within_standard_error,
            velocity_at_reference,
            percent_per_degree: Estimate {
                value: slope * percent,
                range: per_degree.range.scaled(percent),
            },
            mean_temperature_c: mean_t,
            mean_velocity_mps: mean_v,
        };
        let numbers = [
            slope,
            half,
            velocity_at_reference,
            fitted.percent_per_degree.range.low,
            fitted.percent_per_degree.range.high,
        ];
        if numbers.iter().any(|n| !n.is_finite()) || velocity_at_reference <= 0.0 {
            return Err(AnalysisError::NotFinite);
        }
        Ok(fitted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Rng;

    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} is not within {tolerance} of {expected}"
        );
    }

    fn session(temperature_c: f64, mean_mps: f64, shots: usize, sd_mps: f64) -> TemperatureSession {
        TemperatureSession {
            temperature_c,
            mean_mps,
            shots,
            sd_mps,
        }
    }

    #[test]
    fn three_sessions_by_hand() {
        // Weights 5, 10, 5 at 0, 10, 20 °C: mean 10 °C; Σw dt² = 1000.
        let fit = TemperatureSensitivity::new(&[
            session(0.0, 790.0, 5, 4.0),
            session(10.0, 801.0, 10, 4.0),
            session(20.0, 810.0, 5, 4.0),
        ])
        .unwrap();
        close(fit.mean_temperature_c, 10.0, 1e-12);
        close(
            fit.mean_velocity_mps,
            (3950.0 + 8010.0 + 4050.0) / 20.0,
            1e-12,
        );
        // Σw dt dv = 5 (−10)(790 − 800.5) + 5 (10)(810 − 800.5) = 1000.
        close(fit.per_degree.value, 1.0, 1e-12);
        // Residuals −0.5, 0.5, −0.5 m/s: Σw r² = 1.25 + 2.5 + 1.25 = 5.
        let scatter = (5.0f64 / 1.0 / 1000.0).sqrt();
        // The shots alone: pooled SD 4 over Σw dt²: 4 / √1000 is more.
        let within = 4.0 / 1000f64.sqrt();
        close(fit.within_standard_error.unwrap(), within, 1e-12);
        assert!(within > scatter);
        close(fit.standard_error, within, 1e-12);
        // So it has the shots' 4 + 9 + 4 degrees of freedom.
        assert_eq!(fit.degrees_of_freedom, 17.0);
        close(fit.per_degree.range.high, 1.0 + t95(17.0) * within, 1e-12);
        // At 15 °C: 800.5 + 5 = 805.5 m/s; 1 m/s per °C is 0.124% per °C.
        close(fit.velocity_at_reference, 805.5, 1e-12);
        close(fit.percent_per_degree.value, 100.0 / 805.5, 1e-12);
        close(fit.span_c, 20.0, 1e-12);
    }

    #[test]
    fn a_perfect_line_and_its_invariances() {
        let sessions: Vec<TemperatureSession> = [(-5.0, 6), (8.0, 10), (21.0, 4), (30.0, 8)]
            .iter()
            .map(|&(t, n)| session(t, 850.0 + 1.5 * t, n, 3.0))
            .collect();
        let fit = TemperatureSensitivity::new(&sessions).unwrap();
        close(fit.per_degree.value, 1.5, 1e-12);
        close(fit.velocity_at_reference, 850.0 + 22.5, 1e-9);
        // On the line exactly, only the shots' SD sets the range.
        close(
            fit.standard_error,
            fit.within_standard_error.unwrap(),
            1e-15,
        );
        // Order doesn't matter; a constant on every velocity moves only
        // the intercept.
        let reversed: Vec<TemperatureSession> = sessions.iter().rev().copied().collect();
        let other = TemperatureSensitivity::new(&reversed).unwrap();
        close(other.per_degree.range.low, fit.per_degree.range.low, 1e-9);
        let faster: Vec<TemperatureSession> = sessions
            .iter()
            .map(|s| TemperatureSession {
                mean_mps: s.mean_mps + 40.0,
                ..*s
            })
            .collect();
        let faster = TemperatureSensitivity::new(&faster).unwrap();
        close(faster.per_degree.value, 1.5, 1e-9);
        close(
            faster.velocity_at_reference,
            fit.velocity_at_reference + 40.0,
            1e-9,
        );
    }

    #[test]
    fn reasons_for_no_fit() {
        let good = [
            session(0.0, 800.0, 5, 3.0),
            session(10.0, 810.0, 5, 3.0),
            session(20.0, 820.0, 5, 3.0),
        ];
        assert_eq!(TemperatureSensitivity::new(&[]), Err(AnalysisError::Empty));
        assert_eq!(
            TemperatureSensitivity::new(&good[..2]),
            Err(AnalysisError::TooFew {
                what: "sessions",
                need: 3,
                have: 2
            })
        );
        let narrow = [good[0], good[0], session(9.5, 805.0, 5, 3.0)];
        assert_eq!(
            TemperatureSensitivity::new(&narrow),
            Err(AnalysisError::TooNarrow {
                need_c: 10.0,
                have_c: 9.5
            })
        );
        for bad in [
            session(f64::NAN, 800.0, 5, 3.0),
            session(5.0, f64::INFINITY, 5, 3.0),
            session(5.0, 800.0, 5, f64::NAN),
            session(5.0, 800.0, 0, 3.0),
        ] {
            assert!(matches!(
                TemperatureSensitivity::new(&[good[0], bad, good[2]]),
                Err(AnalysisError::Input { index: 1, .. })
            ));
        }
        let implausible = TemperatureSensitivity::new(&[good[0], session(5.0, 3000.0, 5, 3.0)]);
        assert_eq!(
            implausible,
            Err(AnalysisError::ImplausibleMean {
                index: 1,
                mps: 3000.0
            })
        );
        assert_eq!(
            implausible.unwrap_err().to_string(),
            "session 2: a mean of 3000 m/s is not a velocity"
        );
        // Single shots: no SD needed, no within error.
        let singles = [
            session(0.0, 800.0, 1, f64::NAN),
            session(10.0, 811.0, 1, 0.0),
            session(20.0, 819.0, 1, 0.0),
        ];
        let fit = TemperatureSensitivity::new(&singles).unwrap();
        assert_eq!(fit.within_standard_error, None);
        assert!(fit.standard_error > 0.0);
        // Overflowing temperatures are refused, not hung on.
        let huge = [
            session(-1e308, 800.0, 5, 3.0),
            session(0.0, 800.0, 5, 3.0),
            session(1e308, 800.0, 5, 3.0),
        ];
        assert!(TemperatureSensitivity::new(&huge).is_err());
    }

    /// The share of ranges covering the true slope, for sessions at
    /// `temperatures` of `shots` readings with SD 4 m/s and session means
    /// shifted by a normal of SD `between` beyond that.
    fn coverage(
        rng: &mut Rng,
        temperatures: &[f64],
        shots: usize,
        between: f64,
        trials: usize,
    ) -> f64 {
        let mut covered = 0;
        for _ in 0..trials {
            let sessions: Vec<TemperatureSession> = temperatures
                .iter()
                .map(|&t| {
                    let true_mean = 800.0 + 1.2 * t + between * rng.normal();
                    let readings: Vec<f64> =
                        (0..shots).map(|_| true_mean + 4.0 * rng.normal()).collect();
                    let stats = crate::VelocityStats::new(&readings).unwrap();
                    session(t, stats.average, shots, stats.sd.unwrap().value)
                })
                .collect();
            let fit = TemperatureSensitivity::new(&sessions).unwrap();
            if fit.per_degree.range.contains(1.2) {
                covered += 1;
            }
        }
        covered as f64 / trials as f64
    }

    /// The numbers in `docs/group-statistics.md`: `cargo test --release
    /// -p groups-core -- --ignored --nocapture temperature_table`.
    #[test]
    #[ignore]
    fn temperature_table() {
        let mut rng = Rng::new(2026);
        let three: &[f64] = &[0.0, 10.0, 20.0];
        let five: &[f64] = &[-5.0, 5.0, 15.0, 25.0, 35.0];
        for (temperatures, shots) in [(three, 5), (three, 10), (five, 10)] {
            for between in [0.0, 1.0, 2.0, 4.0] {
                println!(
                    "{} sessions of {shots}, between-session SD {between} m/s: coverage {:.1}%",
                    temperatures.len(),
                    100.0 * coverage(&mut rng, temperatures, shots, between, 20_000)
                );
            }
        }
    }

    #[test]
    fn the_range_covers_the_true_slope() {
        let mut rng = Rng::new(9);
        let trials = 2_500;
        // (temperatures, shots, SD between sessions beyond the shots).
        let cases: [(&[f64], usize, f64); 3] = [
            (&[0.0, 10.0, 20.0], 5, 0.0),
            (&[-5.0, 5.0, 15.0, 25.0, 35.0], 10, 0.0),
            (&[-5.0, 5.0, 15.0, 25.0, 35.0], 10, 2.0),
        ];
        for (temperatures, shots, between) in cases {
            let rate = coverage(&mut rng, temperatures, shots, between, trials);
            assert!(
                (0.935..=0.98).contains(&rate),
                "{temperatures:?}, {between}: {rate}"
            );
        }
    }
}
