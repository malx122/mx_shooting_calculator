//! Sizes on the target as angles, the way scopes and dope cards read.
//!
//! A milliradian (mrad, "mil") is taken as 1 mm at 1 m, as scope
//! reticles and turrets are made; the true angle differs by less than one
//! part in a million at these sizes. A minute of angle (MOA) is a true
//! 1/60 degree. "cm per 100 m" is the same angle as a length at 100 m.

/// Minutes of angle in a milliradian: 0.001 rad × 180/π × 60 =
/// 3.437746771.
pub const MOA_PER_MRAD: f64 = 10.8 / std::f64::consts::PI;

/// Centimetres at 100 m in a milliradian.
pub const CM_PER_100M_PER_MRAD: f64 = 10.0;

/// The angle (mrad) of `mm` seen from `distance_m` metres.
pub fn mrad(mm: f64, distance_m: f64) -> f64 {
    mm / distance_m
}

/// The length (mm) at `distance_m` metres of an angle of `mrad`.
pub fn mm_at(mrad: f64, distance_m: f64) -> f64 {
    mrad * distance_m
}

pub fn moa_from_mrad(mrad: f64) -> f64 {
    mrad * MOA_PER_MRAD
}

pub fn mrad_from_moa(moa: f64) -> f64 {
    moa / MOA_PER_MRAD
}

pub fn cm_per_100m_from_mrad(mrad: f64) -> f64 {
    mrad * CM_PER_100M_PER_MRAD
}

pub fn mrad_from_cm_per_100m(cm: f64) -> f64 {
    cm / CM_PER_100M_PER_MRAD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_agree() {
        assert!((MOA_PER_MRAD - 3.437_746_771).abs() < 1e-9);
        // 1 MOA is 29.09 mm at 100 m; 1 mrad is 100 mm, 10 cm per 100 m.
        assert!((mm_at(mrad_from_moa(1.0), 100.0) - 29.088_820_866).abs() < 1e-8);
        assert_eq!(mrad(100.0, 100.0), 1.0);
        assert_eq!(cm_per_100m_from_mrad(1.0), 10.0);
        assert_eq!(mrad_from_cm_per_100m(25.0), 2.5);
        assert!((mrad_from_moa(moa_from_mrad(0.7)) - 0.7).abs() < 1e-15);
        // 25 mm at 250 m: 0.1 mrad, 0.344 MOA, 1 cm per 100 m.
        let angle = mrad(25.0, 250.0);
        assert!((moa_from_mrad(angle) - 0.343_774_677).abs() < 1e-9);
        assert!((cm_per_100m_from_mrad(angle) - 1.0).abs() < 1e-12);
    }
}
