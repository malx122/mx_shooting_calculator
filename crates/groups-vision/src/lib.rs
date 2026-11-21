//! Target photos for shot-group analysis: reading a photo the hunter
//! picked, turning it upright, reducing it once to the size kept, and
//! thumbnails for the library.
//!
//! Photos are read from JPEG, PNG or WebP and kept as JPEG, without the
//! metadata cameras write (time and place, camera and owner). A photo is
//! known by its id, the lowercase hex SHA-256 of the kept bytes, so the
//! same photo imported twice is kept once.

mod jpeg;
mod resample;

use std::fmt;
use std::io::Cursor;

pub use ballistics_store::photo_id;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, RgbImage};

/// The longest side a photo is reduced to on import unless the hunter
/// keeps originals: the largest texture many phones can show, which also
/// leaves a 12-megapixel phone photo (4032 px) as it is.
pub const DEFAULT_MAX_LONG_EDGE_PX: u32 = 4096;
/// The JPEG quality a reduced or turned photo is written with.
pub const DEFAULT_JPEG_QUALITY: u8 = 85;
/// The longest side of the thumbnail made on import.
pub const THUMBNAIL_LONG_EDGE_PX: u32 = 320;
const THUMBNAIL_JPEG_QUALITY: u8 = 80;
/// The type of every kept photo and thumbnail.
pub const MIME: &str = "image/jpeg";
/// The longest side a photo may have to be opened.
pub const MAX_SIDE_PX: u32 = 24_000;
/// The most pixels a photo may have to be opened: each takes 3 to 4 bytes
/// once decoded, and the browser's memory is the tighter.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_PIXELS: u64 = 100_000_000;
#[cfg(target_arch = "wasm32")]
pub const MAX_PIXELS: u64 = 50_000_000;

/// How a photo is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportOptions {
    /// The longest side the kept photo may have, in pixels; `None` keeps the
    /// photo's own size.
    pub max_long_edge_px: Option<u32>,
    /// The JPEG quality (1–100) when the photo has to be written again.
    pub jpeg_quality: u8,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions {
            max_long_edge_px: Some(DEFAULT_MAX_LONG_EDGE_PX),
            jpeg_quality: DEFAULT_JPEG_QUALITY,
        }
    }
}

/// A photo ready to keep: upright, reduced, as JPEG, with a thumbnail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedPhoto {
    /// The JPEG to keep. A JPEG that was already upright and small enough
    /// keeps its picture data unchanged; only its metadata is left out.
    pub jpeg: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    /// The photo's id: the lowercase hex SHA-256 of `jpeg`.
    pub id: String,
    /// A JPEG of the photo with its longest side `THUMBNAIL_LONG_EDGE_PX`
    /// (or less, for a smaller photo).
    pub thumbnail: Vec<u8>,
    /// When the camera says the photo was taken, if it says.
    pub taken: Option<CaptureTime>,
}

/// When a camera says a photo was taken (EXIF `DateTimeOriginal`, else
/// `DateTimeDigitized`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureTime {
    /// The camera clock's reading, as milliseconds since 1970 as if that
    /// clock showed UTC.
    pub local_ms: i64,
    /// How far the camera clock was ahead of UTC, in minutes, when the photo
    /// says (EXIF `OffsetTimeOriginal`).
    pub offset_minutes: Option<i32>,
}

impl CaptureTime {
    /// Milliseconds since 1970 (UTC), or `None` for a time before 1970 (a
    /// camera clock never set). `assumed_offset_minutes` stands in for an
    /// offset the photo doesn't give: usually the device's own.
    pub fn unix_ms(self, assumed_offset_minutes: i32) -> Option<i64> {
        let offset = self.offset_minutes.unwrap_or(assumed_offset_minutes);
        Some(self.local_ms - i64::from(offset) * 60_000).filter(|ms| *ms >= 0)
    }
}

/// A small upright copy of a photo, as RGBA pixels ready for a texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    pub width_px: u32,
    pub height_px: u32,
    /// Four bytes a pixel, row by row from the top left, not premultiplied.
    pub rgba: Vec<u8>,
}

/// Why a photo couldn't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// An HEIC or HEIF photo, the iPhone's own format, which can't be read
    /// here. The iPhone's browser hands photos over as JPEG; on a computer
    /// the photo can be exported as JPEG first.
    Heic,
    /// Neither JPEG, PNG nor WebP.
    Unsupported,
    /// The picture is larger than can be opened (`MAX_SIDE_PX`,
    /// `MAX_PIXELS`).
    TooLarge,
    /// The file is damaged, with the decoder's words.
    Damaged(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Heic => formatter.write_str(
                "HEIC photos can't be read yet: export the photo as JPEG, or pick it from the \
                 phone's browser, which hands it over as JPEG",
            ),
            ImportError::Unsupported => {
                formatter.write_str("this isn't a photo the app reads: use JPEG, PNG or WebP")
            }
            ImportError::TooLarge => formatter.write_str("the photo is too large to open"),
            ImportError::Damaged(reason) => write!(formatter, "the photo is damaged: {reason}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<ImageError> for ImportError {
    fn from(error: ImageError) -> Self {
        match error {
            ImageError::Limits(_) => ImportError::TooLarge,
            ImageError::Unsupported(_) => ImportError::Unsupported,
            other => ImportError::Damaged(other.to_string()),
        }
    }
}

/// Whether the bytes are an HEIF file (HEIC, the iPhone's format): an ISO
/// media file of an HEIF brand that isn't AVIF.
pub fn is_heif(bytes: &[u8]) -> bool {
    const HEIF: [&[u8; 4]; 8] = [
        b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"msf1",
    ];
    const AVIF: [&[u8; 4]; 2] = [b"avif", b"avis"];
    let (Some(size), Some(b"ftyp")) = (bytes.get(..4), bytes.get(4..8)) else {
        return false;
    };
    let size = u32::from_be_bytes(size.try_into().expect("four bytes")) as usize;
    // The major brand, the minor version, then the compatible brands.
    let end = size.min(bytes.len()).min(256);
    let brands: Vec<&[u8; 4]> = bytes
        .get(8..end)
        .unwrap_or_default()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, brand)| brand)
        .collect();
    !brands.iter().any(|brand| AVIF.contains(brand))
        && brands.iter().any(|brand| HEIF.contains(brand))
}

/// Reads a picked photo and makes it ready to keep: turned upright as its
/// EXIF orientation says, reduced so its longest side is at most
/// `max_long_edge_px` (Lanczos-3), and written as JPEG without metadata. A
/// JPEG that is already upright and small enough keeps its picture data
/// unchanged.
pub fn import_photo(bytes: &[u8], options: ImportOptions) -> Result<ImportedPhoto, ImportError> {
    import_upright(bytes, options).map(|(photo, _)| photo)
}

/// [`import_photo`], and the upright picture as RGBA at most `long_edge_px`
/// on its longest side, to show at once without decoding the JPEG again.
pub fn import_photo_with_pixels(
    bytes: &[u8],
    options: ImportOptions,
    long_edge_px: u32,
) -> Result<(ImportedPhoto, Thumbnail), ImportError> {
    let (photo, upright) = import_upright(bytes, options)?;
    let (width, height) = reduced(upright.width(), upright.height(), long_edge_px.max(1));
    let shown = if (width, height) == upright.dimensions() {
        DynamicImage::ImageRgb8(upright).into_rgba8()
    } else {
        DynamicImage::ImageRgb8(image::imageops::thumbnail(&upright, width, height)).into_rgba8()
    };
    Ok((
        photo,
        Thumbnail {
            width_px: shown.width(),
            height_px: shown.height(),
            rgba: shown.into_raw(),
        },
    ))
}

fn import_upright(
    bytes: &[u8],
    options: ImportOptions,
) -> Result<(ImportedPhoto, RgbImage), ImportError> {
    let (format, mut decoder) = open(bytes)?;
    let exif = decoder.exif_metadata().ok().flatten();
    let orientation = exif
        .as_deref()
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    let taken = exif.as_deref().and_then(capture_time);
    let (width, height) = decoder.dimensions();
    let target = options
        .max_long_edge_px
        .map(|limit| reduced(width, height, limit.max(1)))
        .unwrap_or((width, height));
    let kept =
        if format == ImageFormat::Jpeg
            && orientation == Orientation::NoTransforms
            && target == (width, height)
        {
            Some(jpeg::without_metadata(bytes).ok_or_else(|| {
                ImportError::Damaged("the file ends before the picture does".into())
            })?)
        } else {
            None
        };
    let sized = {
        let picture = rgb(DynamicImage::from_decoder(decoder)?);
        if picture.dimensions() == target {
            picture
        } else {
            resample::resize(&picture, target.0, target.1)
        }
    };
    let mut upright = DynamicImage::ImageRgb8(sized);
    upright.apply_orientation(orientation);
    let upright = upright.into_rgb8();
    let (thumbnail_width, thumbnail_height) =
        reduced(upright.width(), upright.height(), THUMBNAIL_LONG_EDGE_PX);
    let thumbnail = encode_jpeg(
        &image::imageops::thumbnail(&upright, thumbnail_width, thumbnail_height),
        THUMBNAIL_JPEG_QUALITY,
    )?;
    let jpeg = match kept {
        Some(kept) => kept,
        None => encode_jpeg(&upright, options.jpeg_quality)?,
    };
    Ok((
        ImportedPhoto {
            id: photo_id(&jpeg),
            width_px: upright.width(),
            height_px: upright.height(),
            jpeg,
            thumbnail,
            taken,
        },
        upright,
    ))
}

/// An upright copy of a photo with its longest side `long_edge_px`
/// (never larger than the photo), for lists of photos.
pub fn thumbnail(bytes: &[u8], long_edge_px: u32) -> Result<Thumbnail, ImportError> {
    let (_, mut decoder) = open(bytes)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let picture = rgb(DynamicImage::from_decoder(decoder)?);
    let (width, height) = reduced(picture.width(), picture.height(), long_edge_px.max(1));
    let mut small = DynamicImage::ImageRgb8(image::imageops::thumbnail(&picture, width, height));
    drop(picture);
    small.apply_orientation(orientation);
    let small = small.into_rgba8();
    Ok(Thumbnail {
        width_px: small.width(),
        height_px: small.height(),
        rgba: small.into_raw(),
    })
}

/// The thumbnail JPEG `import_photo` makes, from a photo already kept (a
/// restored backup's), without writing the photo again.
pub fn thumbnail_jpeg(bytes: &[u8]) -> Result<Vec<u8>, ImportError> {
    let small = thumbnail(bytes, THUMBNAIL_LONG_EDGE_PX)?;
    let rgba = image::RgbaImage::from_raw(small.width_px, small.height_px, small.rgba)
        .ok_or_else(|| ImportError::Damaged("the thumbnail's size".into()))?;
    encode_jpeg(
        &DynamicImage::ImageRgba8(rgba).into_rgb8(),
        THUMBNAIL_JPEG_QUALITY,
    )
}

/// A decoder for a JPEG, PNG or WebP file no larger than can be opened.
fn open(bytes: &[u8]) -> Result<(ImageFormat, impl ImageDecoder + '_), ImportError> {
    if is_heif(bytes) {
        return Err(ImportError::Heic);
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ImportError::Damaged(error.to_string()))?;
    let format = reader
        .format()
        .filter(|format| {
            matches!(
                format,
                ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
            )
        })
        .ok_or(ImportError::Unsupported)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE_PX);
    limits.max_image_height = Some(MAX_SIDE_PX);
    limits.max_alloc = Some(MAX_PIXELS * 4 + (64 << 20));
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if width > MAX_SIDE_PX
        || height > MAX_SIDE_PX
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(ImportError::TooLarge);
    }
    Ok((format, decoder))
}

/// The size of a `width` × `height` picture with its longest side at most
/// `limit`, keeping its shape.
fn reduced(width: u32, height: u32, limit: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= limit {
        return (width, height);
    }
    let scale = f64::from(limit) / f64::from(long);
    let side = |length: u32| ((f64::from(length) * scale).round() as u32).clamp(1, limit);
    (side(width), side(height))
}

/// The picture as 8-bit RGB; transparent parts show as white, as on
/// paper.
fn rgb(picture: DynamicImage) -> RgbImage {
    if !picture.color().has_alpha() {
        return picture.into_rgb8();
    }
    let rgba = picture.into_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [red, green, blue, alpha] = rgba.get_pixel(x, y).0;
        let over_white = |value: u8| {
            let alpha = u16::from(alpha);
            ((u16::from(value) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        };
        image::Rgb([over_white(red), over_white(green), over_white(blue)])
    })
}

fn encode_jpeg(picture: &RgbImage, quality: u8) -> Result<Vec<u8>, ImportError> {
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality.clamp(1, 100))
        .encode_image(picture)?;
    Ok(jpeg)
}

/// The capture time in an EXIF block (a TIFF structure).
fn capture_time(exif: &[u8]) -> Option<CaptureTime> {
    use exif::{In, Tag, Value};
    let fields = exif::Reader::new().read_raw(exif.to_vec()).ok()?;
    let ascii = |tag: Tag| match fields.get_field(tag, In::PRIMARY).map(|field| &field.value) {
        Some(Value::Ascii(parts)) => parts.first().cloned(),
        _ => None,
    };
    let (time, offset, subsecond) = [
        (
            Tag::DateTimeOriginal,
            Tag::OffsetTimeOriginal,
            Tag::SubSecTimeOriginal,
        ),
        (
            Tag::DateTimeDigitized,
            Tag::OffsetTimeDigitized,
            Tag::SubSecTimeDigitized,
        ),
    ]
    .into_iter()
    .find_map(|(time, offset, subsecond)| Some((ascii(time)?, offset, subsecond)))?;
    let mut reading = exif::DateTime::from_ascii(&time).ok()?;
    if let Some(subsecond) = ascii(subsecond) {
        let _ = reading.parse_subsec(&subsecond);
    }
    if let Some(offset) = ascii(offset) {
        let _ = reading.parse_offset(&offset);
    }
    if !(1..=12).contains(&reading.month)
        || !(1..=31).contains(&reading.day)
        || reading.hour > 23
        || reading.minute > 59
        || reading.second > 60
    {
        return None;
    }
    let days = days_from_civil(
        i64::from(reading.year),
        i64::from(reading.month),
        i64::from(reading.day),
    );
    let seconds = days * 86_400
        + i64::from(reading.hour) * 3600
        + i64::from(reading.minute) * 60
        + i64::from(reading.second);
    let milliseconds = reading
        .nanosecond
        .map_or(0, |nanoseconds| i64::from(nanoseconds / 1_000_000));
    Some(CaptureTime {
        local_ms: seconds * 1000 + milliseconds,
        offset_minutes: reading.offset.map(i32::from),
    })
}

/// Days from 1970-01-01 to a date of the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests;
