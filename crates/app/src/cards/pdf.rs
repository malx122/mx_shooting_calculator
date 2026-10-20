//! Sheets as a PDF: vector lines, fills and Helvetica text (one of PDF's
//! standard fonts, so nothing is embedded), each sheet one page at its
//! exact size, so cards print true to size.

use std::fmt::Write;

use super::sheet::{Align, Ink, Mark, PT, Sheet, text_width, win_ansi};

/// Points per millimetre.
const MM: f32 = 72.0 / 25.4;

pub fn write(pages: &[Sheet], title: &str) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    // 1 catalog, 2 page tree, 3 and 4 the fonts, 5 the document info; then
    // a page and its contents for each sheet.
    let first_page = 6;
    let kids: Vec<String> = (0..pages.len())
        .map(|index| format!("{} 0 R", first_page + 2 * index))
        .collect();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .into_bytes(),
    );
    for font in ["Helvetica", "Helvetica-Bold"] {
        objects.push(
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{font} /Encoding /WinAnsiEncoding >>"
            )
            .into_bytes(),
        );
    }
    let mut info = b"<< /Title ".to_vec();
    info.extend(string(title));
    info.extend(b" /Producer ");
    info.extend(string(&format!("{} {}", crate::APP_NAME, crate::VERSION)));
    info.extend(b" >>");
    objects.push(info);
    for (index, sheet) in pages.iter().enumerate() {
        let contents = first_page + 2 * index + 1;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {contents} 0 R >>",
                sheet.width * MM,
                sheet.height * MM
            )
            .into_bytes(),
        );
        let stream = content(sheet);
        let mut object = format!("<< /Length {} >>\nstream\n", stream.len()).into_bytes();
        object.extend(stream);
        object.extend(b"\nendstream");
        objects.push(object);
    }

    // Plain ASCII throughout: text outside it is written as octal escapes.
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n", index + 1).into_bytes());
        pdf.extend(object);
        pdf.extend(b"\nendobj\n");
    }
    let xref = pdf.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(table, "{offset:010} 00000 n ");
    }
    let _ = write!(
        table,
        "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    pdf.extend(table.into_bytes());
    pdf
}

/// The page's drawing operators, in millimetres with y down.
fn content(sheet: &Sheet) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let put = |out: &mut Vec<u8>, text: String| out.extend(text.into_bytes());
    // Millimetres, y down from the top edge; round joins and caps.
    put(
        &mut out,
        format!(
            "{MM:.5} 0 0 {:.5} 0 {:.3} cm 1 j 1 J\n",
            -MM,
            sheet.height * MM
        ),
    );
    let colour = |ink: Ink| {
        let [r, g, b] = ink.rgb();
        format!(
            "{:.3} {:.3} {:.3}",
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0
        )
    };
    let path = |points: &[[f32; 2]]| {
        let mut text = String::new();
        for (index, [x, y]) in points.iter().enumerate() {
            let op = if index == 0 { "m" } else { "l" };
            let _ = write!(text, "{x:.3} {y:.3} {op} ");
        }
        text
    };
    for mark in &sheet.marks {
        match mark {
            Mark::Line {
                points,
                width,
                ink,
                dashed,
                closed,
            } => {
                let dash = if *dashed { "[1.2 1] 0 d" } else { "[] 0 d" };
                let close = if *closed { "h " } else { "" };
                put(
                    &mut out,
                    format!(
                        "{} RG {width:.3} w {dash} {}{close}S\n",
                        colour(*ink),
                        path(points)
                    ),
                );
            }
            Mark::Fill { points, ink, .. } => {
                put(
                    &mut out,
                    format!("{} rg {}h f\n", colour(*ink), path(points)),
                );
            }
            Mark::Text {
                at,
                text,
                size,
                bold,
                align,
                ink,
            } => {
                let width = text_width(text, *size, *bold);
                let x = match align {
                    Align::Left => at[0],
                    Align::Center => at[0] - width / 2.0,
                    Align::Right => at[0] - width,
                };
                // The text matrix flips y back, so the letters stand upright.
                let mut op = format!(
                    "BT {} rg /{} {:.3} Tf 1 0 0 -1 {x:.3} {:.3} Tm ",
                    colour(*ink),
                    if *bold { "F2" } else { "F1" },
                    size * PT,
                    at[1]
                )
                .into_bytes();
                op.extend(string(text));
                op.extend(b" Tj ET\n");
                out.extend(op);
            }
        }
    }
    out
}

/// A PDF string in WinAnsiEncoding.
fn string(text: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for c in text.chars() {
        let code = win_ansi(c).unwrap_or(b'?');
        match code {
            b'(' | b')' | b'\\' => out.extend([b'\\', code]),
            32..=126 => out.push(code),
            _ => out.extend(format!("\\{code:03o}").into_bytes()),
        }
    }
    out.push(b')');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_a_valid_pdf() {
        let mut sheet = Sheet::new(85.6, 54.0);
        sheet.frame(1.0, 1.0, 83.6, 52.0, 0.2, Ink::Black);
        sheet.text(
            [3.0, 8.0],
            "Älg (moose) 3 °C",
            8.0,
            true,
            Align::Left,
            Ink::Black,
        );
        let pdf = write(&[sheet.clone(), sheet], "Test");
        let text = String::from_utf8(pdf).unwrap();
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 2"));
        assert!(text.contains("(\\304lg \\(moose\\) 3 \\260C) Tj"));
        // The cross-reference table points at each object.
        let xref: usize = text
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(text[xref..].starts_with("xref"));
        for line in text[xref..]
            .lines()
            .skip(3)
            .take_while(|line| line.ends_with(" n "))
        {
            let offset: usize = line[..10].parse().unwrap();
            assert!(text[offset..].split_whitespace().nth(2) == Some("obj"));
        }
    }
}
