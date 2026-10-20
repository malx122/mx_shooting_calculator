//! Renders the app icon for the desktop packages (see scripts/package.sh):
//! PNGs for Linux, an iconset for macOS's `iconutil`, and an .ico for
//! Windows, all from assets/mxsc-icon.svg.
//!
//! cargo run -p mxsc --example icons -- <folder>

use std::path::Path;

const ICON_SVG: &[u8] = include_bytes!("../assets/mxsc-icon.svg");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("usage: icons <folder>")?;
    let out = Path::new(&out);
    let iconset = out.join("mxsc.iconset");
    std::fs::create_dir_all(&iconset)?;
    for size in [16, 32, 48, 64, 128, 256, 512, 1024] {
        std::fs::write(out.join(format!("mxsc-{size}.png")), png(size)?)?;
    }
    // macOS wants each size at 1× and 2×, the artwork 824 of 1024 wide
    // with a margin round it, like the system's own icons.
    for size in [16, 32, 128, 256, 512] {
        std::fs::write(
            iconset.join(format!("icon_{size}x{size}.png")),
            padded_png(size)?,
        )?;
        std::fs::write(
            iconset.join(format!("icon_{size}x{size}@2x.png")),
            padded_png(size * 2)?,
        )?;
    }
    std::fs::write(out.join("mxsc.ico"), ico(&[16, 24, 32, 48, 64, 256])?)?;
    Ok(())
}

/// The icon as a PNG, `size` pixels square.
fn png(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    encode(size, &rgba(size)?)
}

/// The icon in the middle of a transparent square, as macOS draws icons.
fn padded_png(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let inner = (size * 824).div_ceil(1024);
    let margin = (size - inner) / 2;
    let icon = rgba(inner)?;
    let mut canvas = vec![0u8; (size * size * 4) as usize];
    for row in 0..inner {
        let from = (row * inner * 4) as usize;
        let to = (((row + margin) * size + margin) * 4) as usize;
        canvas[to..to + (inner * 4) as usize]
            .copy_from_slice(&icon[from..from + (inner * 4) as usize]);
    }
    encode(size, &canvas)
}

/// The icon's pixels, `size` square, as RGBA.
fn rgba(size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let image = egui_extras::image::load_svg_bytes_with_size(
        ICON_SVG,
        Some(eframe::egui::SizeHint::Size(size, size)),
    )?;
    if image.size != [size as usize, size as usize] {
        return Err(format!("the icon came out {:?}, not {size} square", image.size).into());
    }
    Ok(image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect())
}

/// RGBA pixels, `size` square, as a PNG.
fn encode(size: u32, rgba: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(bytes)
}

/// A Windows icon holding a PNG for each size.
fn ico(sizes: &[u32]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let images = sizes
        .iter()
        .map(|&size| png(size))
        .collect::<Result<Vec<_>, _>>()?;
    let mut bytes = Vec::new();
    // The header: reserved, type 1 (icon), the number of images.
    for value in [0u16, 1, sizes.len() as u16] {
        bytes.extend(value.to_le_bytes());
    }
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, image) in sizes.iter().zip(&images) {
        // 256 is written as 0.
        let side = if size >= 256 { 0 } else { size as u8 };
        bytes.extend([side, side, 0, 0]);
        bytes.extend(1u16.to_le_bytes()); // colour planes
        bytes.extend(32u16.to_le_bytes()); // bits per pixel
        bytes.extend((image.len() as u32).to_le_bytes());
        bytes.extend(offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in images {
        bytes.extend(image);
    }
    Ok(bytes)
}
