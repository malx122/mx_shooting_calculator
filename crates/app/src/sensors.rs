//! The device's position and compass, where there are any: in the browser
//! (a phone's GPS and compass, after the hunter allows them). The desktop
//! has neither, so places are searched for by name there.

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
