//! "Find the grid": the four crossings and the squares between them, found
//! in the photo for the grid-squares scale. The square's size can't be seen
//! in pixels, so a found grid counts only once the shooter has confirmed it.

use eframe::egui::{self, RichText};
use groups_vision::grid::{self as vision, NoGrid};
use serde::{Deserialize, Serialize};

use super::scale::{MAX_SQUARES, ScaleDraft, ScaleMode, square_name};
use crate::i18n::{tr, trf};
use crate::theme;

/// Grid square sizes remembered on this device, mm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridSizes {
    /// The square last scaled by.
    pub last_mm: Option<f64>,
    /// On a grid with heavier lines: the big square and the small one, and
    /// how many small squares a big one's side holds.
    pub big_mm: Option<f64>,
    pub small_mm: Option<f64>,
    pub every: Option<u32>,
}

/// A confidence below this gets a caution line. In testing, right grids
/// came out from about 0.3 (fine, faint or halftone lines) to 1; the
/// lowest, 0.29, was a wrong grid from a photographed screen.
const CAUTION_BELOW: f64 = 0.35;

/// Which squares are counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Big,
    Small,
}

/// One way to count a grid found: four crossings (photo px), the squares
/// between them, and the size those squares are taken to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Count {
    pub corners: [[f64; 2]; 4],
    pub across: u32,
    pub down: u32,
    pub square_mm: f64,
}

impl Count {
    fn of(
        squares: &vision::Squares,
        square_mm: f64,
        to_photo: &impl Fn([f64; 2]) -> [f64; 2],
    ) -> Option<Count> {
        (squares.across <= MAX_SQUARES && squares.down <= MAX_SQUARES).then(|| Count {
            corners: squares.corners.map(to_photo),
            across: squares.across,
            down: squares.down,
            square_mm,
        })
    }

    /// Puts the crossings, counts and size in the draft, as if tapped.
    fn place(&self, draft: &mut ScaleDraft) {
        draft.mode = ScaleMode::GridSquares;
        draft.corners = self.corners.to_vec();
        draft.squares_across = self.across;
        draft.squares_down = self.down;
        draft.square_mm = self.square_mm;
    }

    /// Whether the draft still counts these squares (the corners may have
    /// been nudged).
    fn counted_in(&self, draft: &ScaleDraft) -> bool {
        draft.mode == ScaleMode::GridSquares
            && draft.corners.len() == 4
            && (draft.squares_across, draft.squares_down) == (self.across, self.down)
    }
}

/// A grid found and placed.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub big: Option<Count>,
    pub small: Option<Count>,
    /// Small squares to a big one's side, when both were found.
    pub every: u32,
    pub level: Level,
    /// The shooter has said the square's size.
    pub confirmed: bool,
    pub confidence: f64,
    /// Only one size of square was found, while the last grid had two:
    /// which one this is must be asked.
    pub which: bool,
}

impl Found {
    fn count(&self, level: Level) -> Option<&Count> {
        match level {
            Level::Big => self.big.as_ref(),
            Level::Small => self.small.as_ref(),
        }
    }

    fn current(&self) -> Option<&Count> {
        self.count(self.level)
    }
}

/// Where the search is.
#[derive(Clone, Debug, PartialEq)]
pub enum Search {
    Idle,
    /// Asked for, over the whole photo or the part on screen; `ready` once
    /// "Looking for the grid…" has been shown for a frame.
    Looking {
        whole: bool,
        ready: bool,
    },
    Found(Found),
    /// Not found, with the crossings the draft had then: the reason shows
    /// until they change.
    Failed {
        reason: NoGrid,
        corners: Vec<[f64; 2]>,
    },
}

/// The grid search of the scale step.
#[derive(Clone, Debug, PartialEq)]
pub struct Finder {
    pub search: Search,
    /// Still to try once on the whole photo by itself.
    pub auto: bool,
    /// The sizes remembered, updated as the shooter confirms.
    pub sizes: GridSizes,
}

impl Finder {
    pub fn new(auto: bool) -> Self {
        Finder {
            search: Search::Idle,
            auto,
            sizes: GridSizes::default(),
        }
    }

    pub fn ask(&mut self, whole: bool) {
        self.search = Search::Looking {
            whole,
            ready: false,
        };
    }

    /// The search to run now, whole photo or not, once its notice has
    /// shown.
    pub fn due(&self) -> Option<bool> {
        match self.search {
            Search::Looking { whole, ready: true } => Some(whole),
            _ => None,
        }
    }

    /// After a frame: a search asked for runs on the next one.
    pub fn after_frame(&mut self, context: &egui::Context) {
        if let Search::Looking { ready, .. } = &mut self.search {
            *ready = true;
            context.request_repaint();
        }
    }

    /// The grid found and still counted in the draft.
    fn found_in(&self, draft: &ScaleDraft) -> Option<&Found> {
        match &self.search {
            Search::Found(found)
                if [found.big, found.small]
                    .iter()
                    .flatten()
                    .any(|count| count.counted_in(draft)) =>
            {
                Some(found)
            }
            _ => None,
        }
    }

    /// Whether the draft holds a grid found whose square size the shooter
    /// hasn't confirmed yet: the scale doesn't count until they do.
    pub fn awaits_size(&self, draft: &ScaleDraft) -> bool {
        self.found_in(draft).is_some_and(|found| !found.confirmed)
    }

    /// Whether the draft's counts came from a search (so across runs along
    /// the true top edge and Swap would undo a right count).
    pub fn counted(&self, draft: &ScaleDraft) -> bool {
        self.found_in(draft).is_some()
    }

    /// The shooter tapped or typed a square size: the found grid counts.
    pub fn confirm(&mut self, draft: &ScaleDraft) {
        if let Search::Found(found) = &mut self.search {
            found.confirmed = true;
            found.which = false;
            if let Some(count) = match found.level {
                Level::Big => &mut found.big,
                Level::Small => &mut found.small,
            } {
                count.square_mm = draft.square_mm;
            }
            let every = f64::from(found.every);
            if let (Some(big_count), Some(small_count)) = (&mut found.big, &mut found.small) {
                let (big, small) = match found.level {
                    Level::Big => (draft.square_mm, draft.square_mm / every),
                    Level::Small => (draft.square_mm * every, draft.square_mm),
                };
                big_count.square_mm = big;
                small_count.square_mm = small;
                self.sizes.big_mm = Some(big);
                self.sizes.small_mm = Some(small);
                self.sizes.every = Some(found.every);
            }
            self.sizes.last_mm = Some(draft.square_mm);
        }
    }

    /// Counts the big squares or the small ones, each with its own size.
    pub fn choose(&mut self, level: Level, draft: &mut ScaleDraft) {
        if let Search::Found(found) = &mut self.search
            && let Some(count) = found.count(level).copied()
        {
            found.level = level;
            count.place(draft);
        }
    }

    /// Takes the search's result, placing the crossings it found with the
    /// remembered size for their level.
    pub fn finish(
        &mut self,
        result: Result<vision::Grid, NoGrid>,
        to_photo: impl Fn([f64; 2]) -> [f64; 2],
        draft: &mut ScaleDraft,
    ) {
        self.search = match result {
            Ok(grid) => {
                let sizes = self.sizes;
                let found = match &grid.major {
                    Some(major) => {
                        let every = major.every;
                        let big_mm = (sizes.every == Some(every))
                            .then(|| {
                                sizes
                                    .big_mm
                                    .or(sizes.small_mm.map(|small| small * f64::from(every)))
                            })
                            .flatten()
                            .unwrap_or(if every % 4 == 0 || every == 2 {
                                25.4
                            } else {
                                10.0
                            });
                        let small_mm = big_mm / f64::from(every);
                        Found {
                            big: Count::of(&major.squares, big_mm, &to_photo),
                            small: Count::of(&grid.minor, small_mm, &to_photo),
                            every,
                            level: Level::Big,
                            confirmed: false,
                            confidence: grid.confidence,
                            which: false,
                        }
                    }
                    None => Found {
                        big: None,
                        small: Count::of(
                            &grid.minor,
                            sizes.last_mm.unwrap_or(draft.square_mm),
                            &to_photo,
                        ),
                        every: 1,
                        level: Level::Small,
                        confirmed: false,
                        confidence: grid.confidence,
                        which: sizes.every.is_some(),
                    },
                };
                let mut found = found;
                if found.big.is_none() {
                    found.level = Level::Small;
                }
                match found.current() {
                    Some(count) => {
                        count.place(draft);
                        Search::Found(found)
                    }
                    None => Search::Failed {
                        reason: NoGrid::TooFewLines,
                        corners: draft.corners.clone(),
                    },
                }
            }
            Err(reason) => Search::Failed {
                reason,
                corners: draft.corners.clone(),
            },
        };
    }

    /// The button, what came of the search, and the big/small choice.
    pub fn ui(&mut self, ui: &mut egui::Ui, draft: &mut ScaleDraft, can_search: bool) {
        let palette = theme::palette(ui.ctx());
        let looking = matches!(self.search, Search::Looking { .. });
        let found = self.found_in(draft).cloned();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(can_search && !looking, egui::Button::new(tr("Find the grid")))
                .on_hover_text(tr("Look for the grid in the part of the photo on screen"))
                .clicked()
            {
                self.ask(false);
            }
            match (&self.search, &found) {
                (Search::Looking { .. }, _) => {
                    ui.label(RichText::new(tr("Looking for the grid…")).color(palette.text_muted));
                }
                (Search::Found(_), Some(found)) => {
                    let (width, height) = draft.sides_mm();
                    let text = if found.confirmed {
                        RichText::new(trf!(
                            "Found a grid: {} × {} squares of {}.",
                            draft.squares_across,
                            draft.squares_down,
                            square_name(draft.square_mm)
                        ))
                        .color(palette.good)
                    } else if found.which {
                        RichText::new(trf!(
                            "Found {} × {} squares of one size. Tap their size: big or small?",
                            draft.squares_across,
                            draft.squares_down
                        ))
                        .color(palette.warning)
                    } else {
                        RichText::new(trf!(
                            "Found {} × {} squares of {} = {} × {} mm. Tap the square size to confirm it.",
                            draft.squares_across,
                            draft.squares_down,
                            square_name(draft.square_mm),
                            super::tenths(width),
                            super::tenths(height)
                        ))
                        .color(palette.warning)
                    };
                    ui.label(text);
                }
                (Search::Failed { reason, corners }, _) if draft.corners == *corners => {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            failure(*reason),
                            tr("Tap four crossings instead.")
                        ))
                        .color(palette.warning),
                    );
                }
                _ => {}
            }
        });
        let Some(found) = found else {
            return;
        };
        if found.which
            && let (Some(big), Some(small)) = (self.sizes.big_mm, self.sizes.small_mm)
        {
            ui.horizontal_wrapped(|ui| {
                for mm in [big, small] {
                    if ui.button(square_name(mm)).clicked() {
                        draft.square_mm = mm;
                        self.confirm(draft);
                    }
                }
            });
        }
        if found.confidence < CAUTION_BELOW {
            ui.label(
                RichText::new(tr(
                    "The grid was hard to make out: check that the crossings sit on it.",
                ))
                .color(palette.warning),
            );
        }
        if found.big.is_some() && found.small.is_some() {
            ui.horizontal_wrapped(|ui| {
                let mut level = found.level;
                ui.selectable_value(&mut level, Level::Big, tr("Big squares"));
                ui.selectable_value(&mut level, Level::Small, tr("Small squares"));
                if level != found.level {
                    self.choose(level, draft);
                }
            });
        }
    }
}

/// Why no grid was found, for the shooter.
pub fn failure(reason: NoGrid) -> &'static str {
    match reason {
        NoGrid::TooSmall => tr("This part of the photo is too small to search. Zoom out."),
        NoGrid::NoLines => tr("No grid found here."),
        NoGrid::TooFewLines => tr("Too few grid lines show. Zoom out to show more squares."),
        NoGrid::TooBlurry => tr("The grid is too blurred to find."),
    }
}
