//! Air density and speed of sound from what a hunter can measure: temperature,
//! station (absolute) pressure and relative humidity.

const DRY_AIR_GAS_CONSTANT: f64 = 287.058;
const WATER_VAPOUR_GAS_CONSTANT: f64 = 461.495;
const KELVIN_OFFSET: f64 = 273.15;
pub const STANDARD_SEA_LEVEL_PRESSURE_HPA: f64 = 1013.25;
const LAPSE_RATE_K_PER_M: f64 = 0.0065;
/// g·M / (R·L) for the troposphere.
const BAROMETRIC_EXPONENT: f64 = 5.25588;
const STANDARD_GRAVITY: f64 = 9.806_65;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub temperature_c: f64,
    /// Station pressure, the absolute pressure where you stand (not a
    /// sea-level value from a forecast or an airport).
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

    /// Station pressure at an altitude from the sea-level pressure a weather
    /// forecast gives (MSLP), with the air temperature where you stand.
    /// Forecasts reduce their pressure to sea level through the real air
    /// column, not the standard one, so this undoes it the same way: the
    /// hypsometric formula with the column's mean temperature, the station's
    /// plus half the standard lapse over the height. An aviation QNH is
    /// reduced through the standard atmosphere instead; on a standard day the
    /// two agree.
    pub fn station_pressure(sea_level_hpa: f64, altitude_m: f64, temperature_c: f64) -> f64 {
        let mean_temperature_k =
            temperature_c + KELVIN_OFFSET + LAPSE_RATE_K_PER_M * altitude_m / 2.0;
        sea_level_hpa
            * (-STANDARD_GRAVITY * altitude_m / (DRY_AIR_GAS_CONSTANT * mean_temperature_k)).exp()
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
    /// Water vapour is lighter than dry air, so humid air carries sound a
    /// little faster: the mixture's gas constant and heat-capacity ratio,
    /// with vapour's molar heat capacity 4R against dry air's 3.5R.
    pub fn speed_of_sound(&self) -> f64 {
        let total = self.pressure_hpa * 100.0;
        let vapour =
            (self.humidity.clamp(0.0, 1.0) * self.saturation_vapour_pressure_pa()).min(total);
        let x = if total > 0.0 { vapour / total } else { 0.0 };
        let gas_constant = 1.0 / ((1.0 - x) / DRY_AIR_GAS_CONSTANT + x / WATER_VAPOUR_GAS_CONSTANT);
        let heat_capacity = 3.5 * (1.0 - x) + 4.0 * x;
        let heat_capacity_ratio = heat_capacity / (heat_capacity - 1.0);
        (heat_capacity_ratio * gas_constant * self.temperature_k()).sqrt()
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
    fn humid_air_carries_sound_faster() {
        let dry = Atmosphere {
            temperature_c: 30.0,
            ..Atmosphere::standard()
        };
        let humid = Atmosphere {
            humidity: 1.0,
            ..dry
        };
        // About +0.7 % at 30 °C and saturation (Cramer 1993 gives the same).
        let gain = humid.speed_of_sound() / dry.speed_of_sound() - 1.0;
        assert!((gain - 0.0068).abs() < 0.0005, "{gain}");
    }

    #[test]
    fn sea_level_pressure_is_reduced_through_the_real_air() {
        // A standard day: the standard atmosphere's own pressure.
        assert!((Atmosphere::station_pressure(1013.25, 0.0, 15.0) - 1013.25).abs() < 1e-9);
        assert!((Atmosphere::station_pressure(1013.25, 1000.0, 8.5) - 898.75).abs() < 0.1);
        // Cold air is denser, so pressure falls faster with height: a
        // −15 °C day at 1000 m is about 1 % below the standard reduction.
        let cold = Atmosphere::station_pressure(1013.25, 1000.0, -15.0);
        assert!((cold / 898.75 - 1.0 + 0.0109).abs() < 0.001, "{cold}");
    }

    #[test]
    fn altitude_pressure_follows_standard_atmosphere() {
        let up = Atmosphere::standard().at_height(1000.0);
        assert!((up.pressure_hpa - 898.75).abs() < 0.1);
        assert!((up.temperature_c - 8.5).abs() < 1e-9);
    }
}
