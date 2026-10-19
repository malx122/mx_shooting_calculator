//! Air density and speed of sound from what a hunter can measure: temperature,
//! station (absolute) pressure and relative humidity.

const DRY_AIR_GAS_CONSTANT: f64 = 287.058;
const WATER_VAPOUR_GAS_CONSTANT: f64 = 461.495;
const HEAT_CAPACITY_RATIO: f64 = 1.4;
const KELVIN_OFFSET: f64 = 273.15;
pub const STANDARD_SEA_LEVEL_PRESSURE_HPA: f64 = 1013.25;
const LAPSE_RATE_K_PER_M: f64 = 0.0065;
/// g·M / (R·L) for the troposphere.
const BAROMETRIC_EXPONENT: f64 = 5.25588;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub temperature_c: f64,
    /// Station pressure, the absolute pressure where you stand (not the
    /// sea-level QNH value from weather reports).
    pub pressure_hpa: f64,
    /// Relative humidity, 0.0–1.0.
    pub humidity: f64,
}

impl Atmosphere {
    /// ICAO standard day at sea level: 15 °C, 1013.25 hPa, dry air.
    pub fn standard() -> Self {
        Self {
            temperature_c: 15.0,
            pressure_hpa: STANDARD_SEA_LEVEL_PRESSURE_HPA,
            humidity: 0.0,
        }
    }

    /// Station pressure at an altitude from the sea-level pressure (QNH) a
    /// weather report gives, or from 1013.25 hPa on a standard day.
    pub fn station_pressure(sea_level_hpa: f64, altitude_m: f64) -> f64 {
        sea_level_hpa * (1.0 - 2.25577e-5 * altitude_m).powf(BAROMETRIC_EXPONENT)
    }

    /// The same air `height_m` higher up (or lower, if negative), following
    /// the standard lapse rate. Matters for steep uphill and downhill shots.
    pub fn at_height(&self, height_m: f64) -> Self {
        let base_k = self.temperature_k();
        let temperature_k = (base_k - LAPSE_RATE_K_PER_M * height_m).max(1.0);
        Self {
            temperature_c: temperature_k - KELVIN_OFFSET,
            pressure_hpa: self.pressure_hpa * (temperature_k / base_k).powf(BAROMETRIC_EXPONENT),
            humidity: self.humidity,
        }
    }

    fn temperature_k(&self) -> f64 {
        self.temperature_c + KELVIN_OFFSET
    }

    /// Saturation vapour pressure over water (Arden Buck), in Pa.
    fn saturation_vapour_pressure_pa(&self) -> f64 {
        let t = self.temperature_c;
        611.21 * ((18.678 - t / 234.5) * (t / (257.14 + t))).exp()
    }

    /// Air density in kg/m³. Humid air is lighter than dry air.
    pub fn density(&self) -> f64 {
        let total = self.pressure_hpa * 100.0;
        let vapour =
            (self.humidity.clamp(0.0, 1.0) * self.saturation_vapour_pressure_pa()).min(total);
        let temperature = self.temperature_k();
        (total - vapour) / (DRY_AIR_GAS_CONSTANT * temperature)
            + vapour / (WATER_VAPOUR_GAS_CONSTANT * temperature)
    }

    /// Speed of sound in m/s; sets the Mach number the drag tables use.
    pub fn speed_of_sound(&self) -> f64 {
        (HEAT_CAPACITY_RATIO * DRY_AIR_GAS_CONSTANT * self.temperature_k()).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_day_matches_icao() {
        let air = Atmosphere::standard();
        assert!((air.density() - 1.2250).abs() < 0.0005);
        assert!((air.speed_of_sound() - 340.29).abs() < 0.05);
    }

    #[test]
    fn humidity_and_heat_thin_the_air() {
        let dry = Atmosphere::standard();
        let humid = Atmosphere {
            humidity: 1.0,
            ..dry
        };
        let cold = Atmosphere {
            temperature_c: -20.0,
            ..dry
        };
        assert!(humid.density() < dry.density());
        assert!(cold.density() > dry.density());
    }

    #[test]
    fn altitude_pressure_follows_standard_atmosphere() {
        assert!((Atmosphere::station_pressure(1013.25, 0.0) - 1013.25).abs() < 1e-9);
        assert!((Atmosphere::station_pressure(1013.25, 1000.0) - 898.75).abs() < 0.1);
        let up = Atmosphere::standard().at_height(1000.0);
        assert!((up.pressure_hpa - 898.75).abs() < 0.1);
        assert!((up.temperature_c - 8.5).abs() < 1e-9);
    }
}
