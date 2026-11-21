use std::io::Cursor;

use image::{ImageEncoder, Rgb, RgbImage};

use super::*;

const RED: [u8; 3] = [230, 20, 20];
const GREEN: [u8; 3] = [20, 200, 20];
const BLUE: [u8; 3] = [20, 20, 230];
const WHITE: [u8; 3] = [250, 250, 250];

/// A picture whose quarters have different colours, so turning it shows:
/// red top left, green top right, blue bottom left, white bottom right.
fn quarters(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        Rgb(match (x < width / 2, y < height / 2) {
            (true, true) => RED,
            (false, true) => GREEN,
            (true, false) => BLUE,
            (false, false) => WHITE,
        })
    })
}

/// An EXIF block (big-endian TIFF) with an orientation and a capture time
/// with its UTC offset.
fn exif_block(orientation: Option<u16>, taken: Option<&str>, offset: Option<&str>) -> Vec<u8> {
    let ascii = |text: &str| [text.as_bytes(), &[0]].concat();
    let entry = |out: &mut Vec<u8>, tag: u16, kind: u16, count: usize, value: [u8; 4]| {
        out.extend(tag.to_be_bytes());
        out.extend(kind.to_be_bytes());
        out.extend((count as u32).to_be_bytes());
        out.extend(value);
    };
    let primary = usize::from(orientation.is_some()) + usize::from(taken.is_some());
    let sub = usize::from(taken.is_some()) * (1 + usize::from(offset.is_some()));
    let sub_at = 8 + 2 + 12 * primary + 4;
    let data_at = sub_at + if sub > 0 { 2 + 12 * sub + 4 } else { 0 };
    let mut out = b"MM\0\x2a\0\0\0\x08".to_vec();
    let mut data = Vec::new();
    out.extend((primary as u16).to_be_bytes());
    if let Some(orientation) = orientation {
        let [high, low] = orientation.to_be_bytes();
        entry(&mut out, 0x0112, 3, 1, [high, low, 0, 0]);
    }
    if taken.is_some() {
        entry(&mut out, 0x8769, 4, 1, (sub_at as u32).to_be_bytes());
    }
    out.extend(0u32.to_be_bytes());
    if let Some(taken) = taken {
        out.extend((sub as u16).to_be_bytes());
        for (tag, text) in [(0x9003, Some(taken)), (0x9011, offset)] {
            if let Some(text) = text {
                let text = ascii(text);
                let at = ((data_at + data.len()) as u32).to_be_bytes();
                entry(&mut out, tag, 2, text.len(), at);
                data.extend(text);
            }
        }
        out.extend(0u32.to_be_bytes());
    }
    out.extend(data);
    out
}

fn jpeg(picture: &RgbImage, exif: Option<Vec<u8>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95);
    if let Some(exif) = exif {
        encoder.set_exif_metadata(exif).unwrap();
    }
    encoder
        .write_image(
            picture.as_raw(),
            picture.width(),
            picture.height(),
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    bytes
}

fn png(picture: &RgbImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            picture.as_raw(),
            picture.width(),
            picture.height(),
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    bytes
}

/// The colour near a point given as fractions of the picture's size.
fn colour_at(picture: &RgbImage, x: f64, y: f64) -> [u8; 3] {
    let x = (x * f64::from(picture.width())) as u32;
    let y = (y * f64::from(picture.height())) as u32;
    picture.get_pixel(x, y).0
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 24)
}

fn decoded(photo: &ImportedPhoto) -> RgbImage {
    image::load_from_memory_with_format(&photo.jpeg, ImageFormat::Jpeg)
        .unwrap()
        .into_rgb8()
}

#[test]
fn an_upright_jpeg_small_enough_keeps_its_picture_without_its_metadata() {
    let picture = quarters(64, 48);
    let original = jpeg(&picture, Some(exif_block(Some(1), None, None)));
    let photo = import_photo(&original, ImportOptions::default()).unwrap();
    assert_eq!(photo.jpeg, jpeg(&picture, None));
    assert_eq!((photo.width_px, photo.height_px), (64, 48));
    assert_eq!(photo.id, photo_id(&photo.jpeg));
    assert_eq!(photo.id.len(), 64);
    assert!(
        photo
            .id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

#[test]
fn keeping_originals_keeps_an_upright_jpeg_of_any_size() {
    let original = jpeg(&quarters(300, 20), None);
    let options = ImportOptions {
        max_long_edge_px: None,
        ..ImportOptions::default()
    };
    let photo = import_photo(&original, options).unwrap();
    assert_eq!(photo.jpeg, original);
    assert_eq!((photo.width_px, photo.height_px), (300, 20));
    // With the limit, the same photo is reduced.
    let reduced = import_photo(
        &original,
        ImportOptions {
            max_long_edge_px: Some(150),
            ..options
        },
    )
    .unwrap();
    assert_eq!((reduced.width_px, reduced.height_px), (150, 10));
    assert_ne!(reduced.id, photo.id);
}

#[test]
fn photos_are_turned_upright_as_their_exif_says() {
    let picture = quarters(80, 40);
    // 6: turn 90° clockwise; 3: turn 180°; 8: turn 90° anticlockwise.
    for (orientation, size, top_left, top_right) in [
        (6, (40, 80), BLUE, RED),
        (3, (80, 40), WHITE, BLUE),
        (8, (40, 80), GREEN, WHITE),
        (2, (80, 40), GREEN, RED),
    ] {
        let original = jpeg(&picture, Some(exif_block(Some(orientation), None, None)));
        for limit in [None, Some(4000)] {
            let photo = import_photo(
                &original,
                ImportOptions {
                    max_long_edge_px: limit,
                    ..ImportOptions::default()
                },
            )
            .unwrap();
            assert_ne!(photo.jpeg, original, "orientation {orientation} is written");
            assert_eq!((photo.width_px, photo.height_px), size);
            let upright = decoded(&photo);
            assert_eq!(upright.dimensions(), size);
            assert!(
                close(colour_at(&upright, 0.2, 0.2), top_left),
                "orientation {orientation}: {:?}",
                colour_at(&upright, 0.2, 0.2)
            );
            assert!(close(colour_at(&upright, 0.8, 0.2), top_right));
        }
    }
}

#[test]
fn turned_photos_are_reduced_on_their_long_edge() {
    let original = jpeg(&quarters(400, 200), Some(exif_block(Some(6), None, None)));
    let photo = import_photo(
        &original,
        ImportOptions {
            max_long_edge_px: Some(100),
            jpeg_quality: 90,
        },
    )
    .unwrap();
    assert_eq!((photo.width_px, photo.height_px), (50, 100));
    let upright = decoded(&photo);
    assert!(close(colour_at(&upright, 0.25, 0.25), BLUE));
    assert!(close(colour_at(&upright, 0.75, 0.75), GREEN));
}

#[test]
fn png_and_webp_are_read_and_kept_as_jpeg() {
    let picture = quarters(120, 90);
    let mut webp = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut webp)
        .write_image(picture.as_raw(), 120, 90, image::ExtendedColorType::Rgb8)
        .unwrap();
    for file in [png(&picture), webp] {
        let photo = import_photo(&file, ImportOptions::default()).unwrap();
        assert_eq!(&photo.jpeg[..2], &[0xff, 0xd8], "a JPEG");
        assert_eq!((photo.width_px, photo.height_px), (120, 90));
        let kept = decoded(&photo);
        assert!(close(colour_at(&kept, 0.25, 0.25), RED));
        assert!(close(colour_at(&kept, 0.75, 0.75), WHITE));
        assert_eq!(photo.taken, None);
    }
}

#[test]
fn the_same_photo_imported_twice_gets_the_same_id() {
    let file = png(&quarters(120, 90));
    let first = import_photo(&file, ImportOptions::default()).unwrap();
    let again = import_photo(&file, ImportOptions::default()).unwrap();
    assert_eq!(first, again);
    assert_eq!(first.id, photo_id(&first.jpeg));
    let other = import_photo(&png(&quarters(121, 90)), ImportOptions::default()).unwrap();
    assert_ne!(other.id, first.id);
}

#[test]
fn the_capture_time_comes_from_exif() {
    let with_offset = jpeg(
        &quarters(32, 32),
        Some(exif_block(
            None,
            Some("2026:11:12 14:30:05"),
            Some("+01:00"),
        )),
    );
    let taken = import_photo(&with_offset, ImportOptions::default())
        .unwrap()
        .taken
        .unwrap();
    assert_eq!(taken.local_ms, 1_794_493_805_000);
    assert_eq!(taken.offset_minutes, Some(60));
    assert_eq!(taken.unix_ms(-300), Some(1_794_490_205_000));

    let without_offset = jpeg(
        &quarters(32, 32),
        Some(exif_block(Some(6), Some("1999:02:28 23:59:59"), None)),
    );
    let taken = import_photo(&without_offset, ImportOptions::default())
        .unwrap()
        .taken
        .unwrap();
    assert_eq!(taken.offset_minutes, None);
    assert_eq!(taken.unix_ms(0), Some(920_246_399_000));
    assert_eq!(taken.unix_ms(120), Some(920_246_399_000 - 2 * 3_600_000));
    // A camera whose clock was never set.
    let unset = CaptureTime {
        local_ms: -3_600_000,
        offset_minutes: None,
    };
    assert_eq!(unset.unix_ms(0), None);

    let blank = jpeg(
        &quarters(32, 32),
        Some(exif_block(None, Some("    :  :     :  :  "), None)),
    );
    assert_eq!(
        import_photo(&blank, ImportOptions::default())
            .unwrap()
            .taken,
        None
    );
}

#[test]
fn heic_is_named_and_other_files_are_refused() {
    let heic = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic\0\0\0\x08mdat";
    assert!(is_heif(heic));
    assert_eq!(
        import_photo(heic, ImportOptions::default()),
        Err(ImportError::Heic)
    );
    assert_eq!(thumbnail(heic, 100), Err(ImportError::Heic));
    // An HEIF photo that names its brand among the compatible ones.
    assert!(is_heif(b"\0\0\0\x18ftypmif1\0\0\0\0mif1heic"));
    // AVIF shares the container, but isn't HEIC.
    assert!(!is_heif(b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf"));
    assert!(!is_heif(b"\0\0\0"));
    assert!(!is_heif(&jpeg(&quarters(8, 8), None)));

    let avif = b"\0\0\0\x18ftypavif\0\0\0\0avifmiaf\0\0\0\x08mdat";
    assert_eq!(
        import_photo(avif, ImportOptions::default()),
        Err(ImportError::Unsupported)
    );
    assert_eq!(
        import_photo(b"GIF89a\x01\0\x01\0", ImportOptions::default()),
        Err(ImportError::Unsupported)
    );
    let mut cut = jpeg(&quarters(64, 64), None);
    cut.truncate(40);
    assert!(matches!(
        import_photo(&cut, ImportOptions::default()),
        Err(ImportError::Damaged(_))
    ));
}

#[test]
fn thumbnails_are_small_and_upright() {
    let original = jpeg(&quarters(400, 200), Some(exif_block(Some(6), None, None)));
    let small = thumbnail(&original, 64).unwrap();
    assert_eq!((small.width_px, small.height_px), (32, 64));
    assert_eq!(small.rgba.len(), 32 * 64 * 4);
    let pixel = |x: usize, y: usize| {
        let at = (y * 32 + x) * 4;
        [small.rgba[at], small.rgba[at + 1], small.rgba[at + 2]]
    };
    assert!(close(pixel(8, 16), BLUE));
    assert!(close(pixel(24, 48), GREEN));
    assert_eq!(small.rgba[3], 255);
    // Never enlarged.
    let tiny = thumbnail(&png(&quarters(20, 10)), 64).unwrap();
    assert_eq!((tiny.width_px, tiny.height_px), (20, 10));
}

#[test]
fn transparent_parts_become_white() {
    let mut picture = image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 0, 0, 0]));
    picture.put_pixel(0, 0, image::Rgba([0, 0, 255, 255]));
    let mut file = Vec::new();
    image::codecs::png::PngEncoder::new(&mut file)
        .write_image(picture.as_raw(), 16, 16, image::ExtendedColorType::Rgba8)
        .unwrap();
    let kept = decoded(&import_photo(&file, ImportOptions::default()).unwrap());
    assert!(close(kept.get_pixel(12, 12).0, [255, 255, 255]));
}

/// The JPEG with the size its frame header claims changed.
fn claiming(mut jpeg: Vec<u8>, width: u16, height: u16) -> Vec<u8> {
    let frame = jpeg
        .windows(2)
        .position(|pair| pair[0] == 0xff && matches!(pair[1], 0xc0 | 0xc2))
        .expect("a frame header");
    jpeg[frame + 5..frame + 7].copy_from_slice(&height.to_be_bytes());
    jpeg[frame + 7..frame + 9].copy_from_slice(&width.to_be_bytes());
    jpeg
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[test]
fn a_small_file_claiming_a_huge_picture_is_refused_before_decoding() {
    let started = std::time::Instant::now();
    let bomb = claiming(jpeg(&quarters(16, 16), None), 20_000, 20_000);
    assert!(bomb.len() < 1000);
    assert_eq!(
        import_photo(&bomb, ImportOptions::default()),
        Err(ImportError::TooLarge)
    );
    let keep = ImportOptions {
        max_long_edge_px: None,
        ..ImportOptions::default()
    };
    assert_eq!(import_photo(&bomb, keep), Err(ImportError::TooLarge));
    assert_eq!(thumbnail(&bomb, 64), Err(ImportError::TooLarge));
    // Long and thin: under the pixel budget, over the side.
    let strip = claiming(jpeg(&quarters(16, 16), None), 30_000, 100);
    assert_eq!(import_photo(&strip, keep), Err(ImportError::TooLarge));

    let mut file = png(&quarters(16, 16));
    // IHDR: length, "IHDR", width, height, …, then its CRC.
    file[16..20].copy_from_slice(&20_000u32.to_be_bytes());
    file[20..24].copy_from_slice(&20_000u32.to_be_bytes());
    let crc = crc32(&file[12..29]);
    file[29..33].copy_from_slice(&crc.to_be_bytes());
    assert_eq!(
        import_photo(&file, ImportOptions::default()),
        Err(ImportError::TooLarge)
    );
    assert_eq!(thumbnail(&file, 64), Err(ImportError::TooLarge));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

/// An EXIF block naming the owner and the camera's serial, with a GPS
/// position.
fn revealing_exif() -> Vec<u8> {
    let mut out = b"MM\0\x2a\0\0\0\x08".to_vec();
    let artist = b"Jane Hunter\0";
    let serial = b"SN-0042-7731\0";
    // IFD0 at 8: artist, serial (in IFD0 for brevity) and the GPS pointer.
    let entries = 3u16;
    let gps_at = 8 + 2 + 12 * usize::from(entries) + 4;
    let gps_len = 2 + 12 * 2 + 4;
    let data_at = gps_at + gps_len;
    let entry = |out: &mut Vec<u8>, tag: u16, kind: u16, count: u32, value: [u8; 4]| {
        out.extend(tag.to_be_bytes());
        out.extend(kind.to_be_bytes());
        out.extend(count.to_be_bytes());
        out.extend(value);
    };
    out.extend(entries.to_be_bytes());
    entry(
        &mut out,
        0x013b,
        2,
        artist.len() as u32,
        (data_at as u32).to_be_bytes(),
    );
    entry(&mut out, 0x8825, 4, 1, (gps_at as u32).to_be_bytes());
    entry(
        &mut out,
        0xa431,
        2,
        serial.len() as u32,
        ((data_at + artist.len()) as u32).to_be_bytes(),
    );
    out.extend(0u32.to_be_bytes());
    // GPS IFD: latitude N, 65° 49' 12".
    out.extend(2u16.to_be_bytes());
    entry(&mut out, 0x0001, 2, 2, *b"N\0\0\0");
    entry(
        &mut out,
        0x0002,
        5,
        3,
        ((data_at + artist.len() + serial.len()) as u32).to_be_bytes(),
    );
    out.extend(0u32.to_be_bytes());
    out.extend(artist);
    out.extend(serial);
    for (numerator, denominator) in [(65u32, 1u32), (49, 1), (12, 1)] {
        out.extend(numerator.to_be_bytes());
        out.extend(denominator.to_be_bytes());
    }
    out
}

/// The JPEG with these segments (marker, payload) after its start.
fn with_segments(jpeg: &[u8], segments: &[(u8, &[u8])]) -> Vec<u8> {
    let mut out = jpeg[..2].to_vec();
    for (marker, payload) in segments {
        out.extend([0xff, *marker]);
        out.extend(((payload.len() + 2) as u16).to_be_bytes());
        out.extend(*payload);
    }
    out.extend(&jpeg[2..]);
    out
}

#[test]
fn a_kept_jpeg_loses_its_gps_position_owner_and_comments() {
    let picture = quarters(64, 48);
    let plain = jpeg(&picture, None);
    let original = with_segments(
        &jpeg(&picture, Some(revealing_exif())),
        &[
            (
                0xe1,
                b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta>Jane Hunter</x:xmpmeta>",
            ),
            (0xed, b"Photoshop 3.0\0IPTC Jane Hunter"),
            (0xec, b"Ducky\0Jane Hunter"),
            (0xe2, b"ICC_PROFILE\0\x01\x01not a real profile"),
            (0xe2, b"MPF\0secondary images"),
            (0xee, b"Adobe\0\x64\0\0\0\0\x01"),
            (0xfe, b"Shot by Jane Hunter at the club"),
        ],
    );
    let read = |bytes: &[u8]| {
        exif::Reader::new()
            .read_from_container(&mut Cursor::new(bytes))
            .ok()
    };
    let before = read(&original).expect("the original has EXIF");
    assert!(
        before
            .get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
            .is_some()
    );

    let photo = import_photo(&original, ImportOptions::default()).unwrap();
    assert!(read(&photo.jpeg).is_none(), "no EXIF is left");
    let text = String::from_utf8_lossy(&photo.jpeg);
    for gone in [
        "Jane",
        "SN-0042",
        "MPF",
        "xap",
        "Photoshop",
        "Ducky",
        "Exif",
    ] {
        assert!(!text.contains(gone), "{gone} is left");
    }
    for kept in ["ICC_PROFILE", "Adobe", "JFIF"] {
        assert!(text.contains(kept), "{kept} is lost");
    }
    assert_eq!(photo.id, photo_id(&photo.jpeg));
    let pixels = |bytes: &[u8]| {
        image::load_from_memory_with_format(bytes, ImageFormat::Jpeg)
            .unwrap()
            .into_rgb8()
    };
    assert_eq!(
        pixels(&photo.jpeg),
        pixels(&plain),
        "the picture is the same"
    );
    // Trailing bytes after the end, such as a second image, are left too.
    let mut trailing = original.clone();
    trailing.extend_from_slice(&plain);
    assert_eq!(
        import_photo(&trailing, ImportOptions::default())
            .unwrap()
            .jpeg,
        photo.jpeg
    );
}

#[test]
fn a_jpeg_cut_short_is_refused() {
    let whole = jpeg(&quarters(64, 48), None);
    let scan = whole
        .windows(2)
        .position(|pair| pair == [0xff, 0xda])
        .unwrap();
    for cut in [scan, scan + 20, whole.len() - 2] {
        assert!(
            matches!(
                import_photo(&whole[..cut], ImportOptions::default()),
                Err(ImportError::Damaged(_))
            ),
            "cut at {cut} of {}",
            whole.len()
        );
    }
}

#[test]
fn import_makes_a_small_upright_thumbnail() {
    let original = jpeg(&quarters(800, 400), Some(exif_block(Some(6), None, None)));
    let photo = import_photo(&original, ImportOptions::default()).unwrap();
    let small = image::load_from_memory_with_format(&photo.thumbnail, ImageFormat::Jpeg)
        .unwrap()
        .into_rgb8();
    assert_eq!(small.dimensions(), (160, 320));
    assert!(close(colour_at(&small, 0.25, 0.25), BLUE));
    // A kept photo gets one too, and a small photo's is its own size.
    let kept = import_photo(&jpeg(&quarters(64, 48), None), ImportOptions::default()).unwrap();
    let small = image::load_from_memory(&kept.thumbnail).unwrap();
    assert_eq!((small.width(), small.height()), (64, 48));
}

#[test]
fn a_twelve_megapixel_phone_photo_is_not_reduced() {
    assert_eq!(reduced(4032, 3024, DEFAULT_MAX_LONG_EDGE_PX), (4032, 3024));
    assert_eq!(reduced(5712, 4284, DEFAULT_MAX_LONG_EDGE_PX), (4096, 3072));
}
