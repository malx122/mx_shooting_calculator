//! The MX target: a sheet to shoot at, with a mark near each corner that
//! the app finds on a photo of it to set the scale, straighten the photo
//! and place the aim points by itself. The layout and the marks come from
//! `groups_vision::marks`, which reads them back; the format is in
//! `docs/mx-targets.md`.

use eframe::egui;
use groups_vision::marks::{
    AimPoints, AimShape, AimSize, Corner, FORMAT, GridSpacing, Layout, MARK_MODULES, Paper,
};
use serde::{Deserialize, Serialize};

use super::sheet::{Align, Ink, Sheet, fit};
use crate::i18n::{tr, trf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MxPaper {
    #[default]
    A4,
    A3,
    Letter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MxGrid {
    Off,
    #[default]
    Mm10,
    Inch,
}

/// The hunter's MX target choices, kept on this device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MxTargetOptions {
    pub paper: MxPaper,
    pub landscape: bool,
    /// 1, 2, 4 or 6.
    pub aims: u8,
    pub grid: MxGrid,
    /// A square aim mark; a diamond otherwise.
    pub square: bool,
    /// The aim mark's width: 25, 40, 60 or 80 mm.
    pub size_mm: u8,
}

impl MxPaper {
    /// Letter where it is the usual paper (the United States and Canada),
    /// A4 elsewhere.
    pub fn for_locale(locale: Option<&str>) -> MxPaper {
        let region = locale
            .and_then(|locale| locale.split(['-', '_']).nth(1))
            .map(str::to_ascii_uppercase);
        match region.as_deref() {
            Some("US" | "CA") => MxPaper::Letter,
            _ => MxPaper::A4,
        }
    }
}

impl Default for MxTargetOptions {
    fn default() -> Self {
        MxTargetOptions {
            paper: MxPaper::for_locale(sys_locale::get_locale().as_deref()),
            landscape: false,
            aims: 4,
            grid: MxGrid::Mm10,
            square: false,
            size_mm: 40,
        }
    }
}

impl MxTargetOptions {
    pub fn layout(&self) -> Layout {
        Layout {
            paper: match self.paper {
                MxPaper::A4 => Paper::A4,
                MxPaper::A3 => Paper::A3,
                MxPaper::Letter => Paper::Letter,
            },
            landscape: self.landscape,
            aims: match self.aims {
                0 | 1 => AimPoints::One,
                2 | 3 => AimPoints::Two,
                4 | 5 => AimPoints::Four,
                _ => AimPoints::Six,
            },
            grid: match self.grid {
                MxGrid::Off => GridSpacing::Off,
                MxGrid::Mm10 => GridSpacing::Mm10,
                MxGrid::Inch => GridSpacing::Inch,
            },
            shape: if self.square {
                AimShape::Square
            } else {
                AimShape::Diamond
            },
            size: *AimSize::ALL
                .iter()
                .min_by_key(|size| (size.mm() as i32 - i32::from(self.size_mm)).abs())
                .unwrap_or(&AimSize::Mm40),
        }
    }

    /// The PDF's file name.
    pub fn file_name(&self) -> String {
        let layout = self.layout();
        format!(
            "mx-target-{}-{}-{}.pdf",
            layout.paper.name().to_lowercase(),
            if layout.landscape {
                "landscape"
            } else {
                "portrait"
            },
            layout.aims.count()
        )
    }
}

/// The grid's light blue: light grey on a black-and-white printer, and too
/// light for the mark search to take for ink.
const GRID_INK: Ink = Ink::Rgb([0xB8, 0xD4, 0xEE]);

/// The layout in words, as printed under the title.
pub fn words(layout: &Layout) -> String {
    let count = layout.aims.count();
    let size = layout.aim_size_mm();
    let aims = if count == 1 {
        tr("1 aim point").to_owned()
    } else {
        trf!("{} aim points", count)
    };
    let mark = match (layout.shape, count == 1) {
        (AimShape::Diamond, true) => trf!("{} mm diamond", size),
        (AimShape::Diamond, false) => trf!("{} mm diamonds", size),
        (AimShape::Square, true) => trf!("{} mm square", size),
        (AimShape::Square, false) => trf!("{} mm squares", size),
    };
    let grid = match layout.grid {
        GridSpacing::Off => tr("no grid"),
        GridSpacing::Mm10 => tr("10 mm grid"),
        GridSpacing::Inch => tr("1 inch grid"),
    };
    let orientation = if layout.landscape {
        tr("landscape")
    } else {
        tr("portrait")
    };
    format!(
        "{aims} · {mark} · {grid} · {} {orientation}",
        layout.paper.name()
    )
}

/// The sheet as printed.
pub fn page(layout: &Layout) -> Sheet {
    let [width, height] = layout.sheet_mm();
    let mut sheet = Sheet::new(width as f32, height as f32);
    let f = |v: f64| v as f32;

    if let Some(step) = layout.grid.spacing_mm() {
        let [x0, y0, x1, y1] = layout.field_mm();
        let lines = |from: f64, to: f64, middle: f64| {
            let first = ((from - middle) / step).ceil() as i64;
            let last = ((to - middle) / step).floor() as i64;
            (first..=last).map(move |k| middle + k as f64 * step)
        };
        for x in lines(x0, x1, width / 2.0) {
            sheet.line([f(x), f(y0)], [f(x), f(y1)], 0.15, GRID_INK);
        }
        for y in lines(y0, y1, height / 2.0) {
            sheet.line([f(x0), f(y)], [f(x1), f(y)], 0.15, GRID_INK);
        }
    }

    let size = layout.aim_size_mm();
    for (number, &[cx, cy]) in layout.aim_points_mm().iter().enumerate() {
        aim_mark(&mut sheet, layout.shape, [f(cx), f(cy)], f(size));
        sheet.text(
            [f(cx - size / 2.0), f(cy - size / 2.0 - 1.2)],
            (number + 1).to_string(),
            7.0,
            true,
            Align::Left,
            Ink::Grey,
        );
    }

    for corner in Corner::ALL {
        corner_mark(&mut sheet, layout, corner);
    }

    // The title and the layout between the top marks, the print advice and
    // the check ruler between the bottom ones, each a module and a half
    // clear of the marks.
    let inset = layout.paper.inset_mm();
    let room = f(width - 2.0 * (inset + layout.mark_mm() / 2.0 + 1.5 * layout.module_mm()));
    let middle = f(width / 2.0);
    sheet.text(
        [middle, f(inset - 0.5)],
        fit(tr("MX target"), 16.0, true, room),
        16.0,
        true,
        Align::Center,
        Ink::Black,
    );
    sheet.text(
        [middle, f(inset + 5.5)],
        fit(&words(layout), 8.0, false, room),
        8.0,
        false,
        Align::Center,
        Ink::Grey,
    );
    let ruler_y = f(height - inset + 1.0);
    sheet.text(
        [middle, ruler_y - 6.0],
        fit(tr("Print at 100% (no scaling)"), 9.0, true, room),
        9.0,
        true,
        Align::Center,
        Ink::Black,
    );
    ruler(&mut sheet, [middle - 50.0, ruler_y]);
    sheet.text(
        [middle, ruler_y + 7.0],
        fit(
            &trf!(
                "MX target format {format}, layout {code}",
                format = FORMAT,
                code = layout.code()
            ),
            6.0,
            false,
            room,
        ),
        6.0,
        false,
        Align::Center,
        Ink::Grey,
    );
    sheet
}

/// An aim mark `size` mm across at `centre`: an outline a tenth of its
/// width thick (across the corners for a diamond), with a fine cross
/// through the middle. Each side is a rectangle overlapping its
/// neighbours at the corners, so the corners print without a seam.
fn aim_mark(sheet: &mut Sheet, shape: AimShape, centre: [f32; 2], size: f32) {
    let [cx, cy] = centre;
    let (outer, inner) = (size / 2.0, size * 0.4);
    let out = match shape {
        AimShape::Diamond => [
            [cx, cy - outer],
            [cx + outer, cy],
            [cx, cy + outer],
            [cx - outer, cy],
        ],
        AimShape::Square => [
            [cx - outer, cy - outer],
            [cx + outer, cy - outer],
            [cx + outer, cy + outer],
            [cx - outer, cy + outer],
        ],
    };
    let band = match shape {
        AimShape::Diamond => (outer - inner) / std::f32::consts::SQRT_2,
        AimShape::Square => outer - inner,
    };
    for i in 0..4 {
        let (a, b) = (out[i], out[(i + 1) % 4]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dy);
        // Clockwise on the page (y down), the inside is to the right.
        let (nx, ny) = (-dy / length * band, dx / length * band);
        sheet.fill(
            vec![a, b, [b[0] + nx, b[1] + ny], [a[0] + nx, a[1] + ny]],
            Ink::Black,
        );
    }
    sheet.line([cx - inner, cy], [cx + inner, cy], 0.25, Ink::Black);
    sheet.line([cx, cy - inner], [cx, cy + inner], 0.25, Ink::Black);
}

/// The corner mark: its black modules drawn as runs along the rows and
/// down the columns, so neighbouring modules print without a seam.
fn corner_mark(sheet: &mut Sheet, layout: &Layout, corner: Corner) {
    let modules = layout.modules(corner);
    let u = layout.module_mm() as f32;
    let [cx, cy] = layout.mark_centres_mm()[corner.index()];
    let half = MARK_MODULES as f32 / 2.0;
    let (left, top) = (cx as f32 - half * u, cy as f32 - half * u);
    let columns: [[bool; MARK_MODULES]; MARK_MODULES] =
        std::array::from_fn(|j| std::array::from_fn(|i| modules[i][j]));
    for (across, lines) in [(true, modules), (false, columns)] {
        for (line, cells) in lines.iter().enumerate() {
            let mut k = 0;
            while k < MARK_MODULES {
                if !cells[k] {
                    k += 1;
                    continue;
                }
                let start = k;
                while k < MARK_MODULES && cells[k] {
                    k += 1;
                }
                let run = (k - start) as f32 * u;
                let (a, b) = (start as f32 * u, line as f32 * u);
                if across {
                    sheet.rect(left + a, top + b, run, u, Ink::Key);
                } else {
                    sheet.rect(left + b, top + a, u, run, Ink::Key);
                }
            }
        }
    }
}

/// A 100 mm ruler from `start` (its left end on the baseline), ticks up.
fn ruler(sheet: &mut Sheet, start: [f32; 2]) {
    let [x0, y] = start;
    sheet.line([x0, y], [x0 + 100.0, y], 0.3, Ink::Key);
    for mm in 0..=100 {
        let x = x0 + mm as f32;
        let length = if mm % 10 == 0 {
            3.0
        } else if mm % 5 == 0 {
            2.0
        } else {
            1.2
        };
        sheet.line([x, y], [x, y - length], 0.15, Ink::Key);
    }
    sheet.text([x0, y + 3.2], "0", 7.0, false, Align::Center, Ink::Black);
    sheet.text(
        [x0 + 50.0, y + 3.2],
        "50",
        7.0,
        false,
        Align::Center,
        Ink::Black,
    );
    sheet.text(
        [x0 + 100.0, y + 3.2],
        "100 mm",
        7.0,
        false,
        Align::Center,
        Ink::Black,
    );
}

/// The controls for the MX target.
pub fn controls(ui: &mut egui::Ui, options: &mut MxTargetOptions) {
    ui.weak(tr(
        "A target to print. A photo of it with all four corner marks in view sets the scale, \
         straightens the photo and places the aim points by itself.",
    ));
    ui.add_space(6.0);
    crate::widgets::grid(ui, "mx-target", |ui| {
        ui.label(tr("Paper"));
        ui.horizontal_wrapped(|ui| {
            for (paper, name, hint) in [
                (MxPaper::A4, "A4", "210 × 297 mm"),
                (
                    MxPaper::A3,
                    "A3",
                    tr("297 × 420 mm: room for bigger aim marks"),
                ),
                (MxPaper::Letter, "Letter", tr("8.5 × 11 inches")),
            ] {
                ui.selectable_value(&mut options.paper, paper, name)
                    .on_hover_text(hint);
            }
        });
        ui.end_row();
        ui.label("");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut options.landscape, false, tr("Portrait"));
            ui.selectable_value(&mut options.landscape, true, tr("Landscape"));
        });
        ui.end_row();
        ui.label(tr("Aim points"));
        ui.horizontal(|ui| {
            for count in [1u8, 2, 4, 6] {
                ui.selectable_value(&mut options.aims, count, count.to_string());
            }
        });
        ui.end_row();
        ui.label(tr("Aim mark"));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut options.square, false, tr("Diamond"));
            ui.selectable_value(&mut options.square, true, tr("Upright square"));
        });
        ui.end_row();
        ui.label(tr("Size"));
        ui.horizontal_wrapped(|ui| {
            for size in AimSize::ALL {
                let mm = size.mm() as u8;
                ui.selectable_value(&mut options.size_mm, mm, format!("{mm} mm"))
                    .on_hover_text(trf!("For about {} m", size.range_m()));
            }
        });
        ui.end_row();
        ui.label(tr("Grid"));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut options.grid, MxGrid::Off, tr("None"));
            ui.selectable_value(&mut options.grid, MxGrid::Mm10, "10 mm");
            ui.selectable_value(&mut options.grid, MxGrid::Inch, tr("1 inch"));
        });
        ui.end_row();
    });
    let layout = options.layout();
    let printed = layout.aim_size_mm();
    if printed < layout.size.mm() {
        ui.weak(trf!(
            "The aim marks are {} mm so that they fit: fewer aim points or bigger paper \
             leave room for {} mm.",
            printed,
            layout.size.mm()
        ));
    }
    ui.add_space(4.0);
    ui.weak(tr(
        "Print at 100% (no scaling) and check the ruler at the foot of the sheet: it should \
         measure 100 mm. Keep the corner marks clean and flat; a hole through one is fine.",
    ));
}

#[cfg(test)]
mod tests;
