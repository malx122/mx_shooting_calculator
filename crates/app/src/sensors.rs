//! The device's position and compass, where there are any: in the browser
//! (a phone's GPS and compass, after the hunter allows them). The desktop
//! has neither, so places are searched for by name, and positions and
//! bearings typed, there.

use eframe::egui;

use crate::weather::Place;

/// Whether the device can say where it is.
pub fn can_locate() -> bool {
    #[cfg(target_arch = "wasm32")]
    return crate::web::can_locate();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Finds where the device is; `done` gets the place (or why not).
pub fn locate(done: impl Fn(Result<Place, String>) + 'static) {
    #[cfg(target_arch = "wasm32")]
    crate::web::locate(move |position| {
        done(position.map(|(latitude, longitude)| Place {
            name: crate::i18n::trf!(
                "Your position ({latitude:.2}° N, {longitude:.2}° E)",
                latitude = latitude,
                longitude = longitude
            ),
            latitude,
            longitude,
            region: None,
            country_code: None,
        }))
    });
    #[cfg(not(target_arch = "wasm32"))]
    done(Err(crate::i18n::tr(
        "this device can't tell where it is; search for a place",
    )
    .into()));
}

/// Where the device is, and how far off that may be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    pub latitude: f64,
    pub longitude: f64,
    /// Metres, as the device says; `None` when it doesn't.
    pub accuracy_m: Option<f64>,
}

#[cfg(target_arch = "wasm32")]
fn fix_of((latitude, longitude, accuracy_m): crate::web::Fix) -> Fix {
    Fix {
        latitude,
        longitude,
        accuracy_m,
    }
}

/// Finds where the device is, afresh and as exactly as it can: for marking
/// a spot. `done` gets the position (or why not).
pub fn fix(done: impl Fn(Result<Fix, String>) + 'static) {
    #[cfg(target_arch = "wasm32")]
    crate::web::position(true, move |fix| done(fix.map(fix_of)));
    #[cfg(not(target_arch = "wasm32"))]
    done(Err(crate::i18n::tr(
        "this device can't tell where it is; type the position",
    )
    .into()));
}

/// Starts following where the device is as it moves (the browser may ask
/// the hunter first); the answers come in `followed`.
pub fn follow(context: egui::Context) {
    #[cfg(target_arch = "wasm32")]
    crate::web::follow(context);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = context;
}

/// The latest position while following, or why there is none.
pub fn followed() -> Option<Result<Fix, String>> {
    #[cfg(target_arch = "wasm32")]
    return crate::web::followed().map(|fix| fix.map(fix_of));
    #[cfg(not(target_arch = "wasm32"))]
    None
}

/// Stops following where the device is.
pub fn stop_following() {
    #[cfg(target_arch = "wasm32")]
    crate::web::stop_following();
}

/// How many minutes the device's clock is ahead of UTC at a moment, where
/// the device says (in the browser).
pub fn utc_offset_minutes(unix_ms: i64) -> Option<f64> {
    #[cfg(target_arch = "wasm32")]
    return Some(crate::web::utc_offset_minutes(unix_ms));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = unix_ms;
        None
    }
}

/// Whether the device has a compass (a phone in the browser).
pub fn has_compass() -> bool {
    #[cfg(target_arch = "wasm32")]
    return crate::web::tilt_supported();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Starts reading the compass (the browser may ask the hunter first).
pub fn start_compass(context: egui::Context) {
    #[cfg(target_arch = "wasm32")]
    crate::web::start_tilt(context);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = context;
}

/// Where the phone's top edge points, degrees from north, once known.
pub fn heading() -> Option<f64> {
    #[cfg(target_arch = "wasm32")]
    return crate::web::heading();
    #[cfg(not(target_arch = "wasm32"))]
    None
}
