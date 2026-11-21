//! A JPEG without what it says about the camera, its owner and where it
//! was taken. The picture's own data is copied unchanged.

/// The JPEG up to its end, with only the segments that decoding needs and
/// JFIF's (APP0), the colour profile (APP2 `ICC_PROFILE`) and Adobe's
/// colour transform (APP14): no Exif or XMP (APP1), Photoshop or IPTC
/// (APP13), Ducky (APP12), comments, or other application segments.
/// `None` when the bytes aren't a whole JPEG: a frame, a scan and the end.
pub(crate) fn without_metadata(jpeg: &[u8]) -> Option<Vec<u8>> {
    if jpeg.get(..2)? != [0xff, 0xd8] {
        return None;
    }
    let mut out = Vec::with_capacity(jpeg.len());
    out.extend_from_slice(&[0xff, 0xd8]);
    let mut at = 2;
    let mut framed = false;
    let mut scanned = false;
    loop {
        if *jpeg.get(at)? != 0xff {
            return None;
        }
        while *jpeg.get(at + 1)? == 0xff {
            at += 1;
        }
        let marker = jpeg[at + 1];
        match marker {
            0xd9 => {
                out.extend_from_slice(&[0xff, 0xd9]);
                return scanned.then_some(out);
            }
            0x01 | 0xd0..=0xd7 => {
                out.extend_from_slice(&jpeg[at..at + 2]);
                at += 2;
                continue;
            }
            _ => {}
        }
        let length = usize::from(u16::from_be_bytes([*jpeg.get(at + 2)?, *jpeg.get(at + 3)?]));
        if length < 2 {
            return None;
        }
        let segment = jpeg.get(at..at + 2 + length)?;
        let keep = match marker {
            0xe0 | 0xee => true,
            0xe2 => segment[4..].starts_with(b"ICC_PROFILE\0"),
            0xe1..=0xef | 0xfe => false,
            _ => true,
        };
        if keep {
            out.extend_from_slice(segment);
        }
        framed |= matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
        at += segment.len();
        if marker == 0xda {
            if !framed {
                return None;
            }
            scanned = true;
            // The coded picture runs to the next marker that is neither a
            // stuffed 0xff nor a restart.
            let start = at;
            loop {
                if *jpeg.get(at)? == 0xff {
                    match *jpeg.get(at + 1)? {
                        0x00 | 0xd0..=0xd7 => at += 2,
                        _ => break,
                    }
                } else {
                    at += 1;
                }
            }
            out.extend_from_slice(&jpeg[start..at]);
        }
    }
}
