//! Gyroscopic stability (Miller twist rule) and the spin drift it causes
//! (Litz's empirical fit). Both formulas are defined in imperial units, so
//! the inputs are converted here and nowhere else.

use super::atmosphere::Atmosphere;

const GRAINS_PER_KG: f64 = 15_432.358;
const METRES_PER_INCH: f64 = 0.0254;
const FPS_PER_MPS: f64 = 3.280_84;
const INHG_PER_HPA: f64 = 0.029_53;

/// Miller gyroscopic stability factor Sg. Below 1.0 the bullet tumbles,
/// 1.0–1.4 is marginal, and 1.4 or above is fully stable.
pub fn miller_stability(
    mass_kg: f64,
    diameter_m: f64,
    length_m: f64,
    twist_m: f64,
    velocity: f64,
    atmosphere: &Atmosphere,
) -> f64 {
    let mass_grains = mass_kg * GRAINS_PER_KG;
    let diameter_in = diameter_m / METRES_PER_INCH;
    let length_calibres = length_m / diameter_m;
    let twist_calibres = twist_m / diameter_m;
    let base = 30.0 * mass_grains
        / (twist_calibres.powi(2)
            * diameter_in.powi(3)
            * length_calibres
            * (1.0 + length_calibres.powi(2)));
    let velocity_correction = (velocity * FPS_PER_MPS / 2800.0).cbrt();
    let temperature_f = atmosphere.temperature_c * 9.0 / 5.0 + 32.0;
    let pressure_inhg = atmosphere.pressure_hpa * INHG_PER_HPA;
    let air_correction = (temperature_f + 460.0) / (59.0 + 460.0) * (29.92 / pressure_inhg);
    base * velocity_correction * air_correction
}

/// Lateral spin drift in metres after the given time of flight. Positive is
/// to the right, which is where a right-hand twist drifts.
pub fn spin_drift(stability: f64, time_of_flight: f64, right_hand_twist: bool) -> f64 {
    if stability <= 0.0 || time_of_flight <= 0.0 {
        return 0.0;
    }
    let drift = 1.25 * (stability + 1.2) * time_of_flight.powf(1.83) * METRES_PER_INCH;
    if right_hand_twist { drift } else { -drift }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faster_twist_is_more_stable() {
        let air = Atmosphere::standard();
        let slow = miller_stability(0.0117, 0.00782, 0.031, 12.0 * 0.0254, 790.0, &air);
        let fast = miller_stability(0.0117, 0.00782, 0.031, 10.0 * 0.0254, 790.0, &air);
        assert!(fast > slow);
        assert!(slow > 1.0);
    }

    #[test]
    fn thin_air_raises_stability() {
        let sea_level = Atmosphere::standard();
        let mountain = Atmosphere {
            pressure_hpa: 800.0,
            ..sea_level
        };
        let at = |air: &Atmosphere| miller_stability(0.0091, 0.00671, 0.034, 0.2, 800.0, air);
        assert!(at(&mountain) > at(&sea_level));
    }

    #[test]
    fn spin_drift_follows_twist_direction() {
        assert!(spin_drift(1.8, 0.8, true) > 0.0);
        assert_eq!(spin_drift(1.8, 0.8, false), -spin_drift(1.8, 0.8, true));
        assert_eq!(spin_drift(1.8, 0.0, true), 0.0);
    }
}
