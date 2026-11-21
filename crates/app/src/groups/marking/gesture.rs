//! Telling a tap from a hold, a drag that moves a mark from one that pans.
//!
//! - A press that lifts within [`MOVE_THRESHOLD`] of where it went down is a
//!   tap, and lands where it lifts.
//! - A press that stays put for [`HOLD_SECONDS`] is a hold: on a mark it
//!   selects it; off one it lets the finger steer the loupe before lifting.
//! - A press that moves further first pans the canvas, unless it started on
//!   a mark that may be dragged straight away; a held mark is dragged too.
//! - Two taps within [`DOUBLE_TAP_SECONDS`] and [`DOUBLE_TAP_DISTANCE`] are a
//!   double tap.

use eframe::egui::{Pos2, Vec2};

/// How far (points) a finger may wander and still tap.
pub const MOVE_THRESHOLD: f32 = 8.0;

/// How long (seconds) a still press takes to become a hold.
pub const HOLD_SECONDS: f64 = 0.4;

/// The longest gap (seconds) between the taps of a double tap.
pub const DOUBLE_TAP_SECONDS: f64 = 0.3;

/// The farthest apart (points) the taps of a double tap may be.
pub const DOUBLE_TAP_DISTANCE: f32 = 12.0;

/// What a press went down on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Under<H> {
    /// Nothing that can be moved.
    Nothing,
    /// A mark, and whether dragging it straight away moves it (otherwise it
    /// must be held first).
    Mark { mark: H, drag_at_once: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Down, not yet told apart.
    Pending,
    /// Held still long enough.
    Held,
    Panning,
    Moving,
    Cancelled,
}

/// What a frame of the press does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step<H> {
    Nothing,
    /// Became a hold on this mark (select it), or off any mark.
    Held(Option<H>),
    /// Pan the canvas by this many points.
    Pan(Vec2),
    /// Move this mark: it is now `offset` points from where the press began.
    Move {
        mark: H,
        offset: Vec2,
    },
}

/// How the press ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release<H> {
    /// A quick tap here, on this mark or on nothing.
    Tap { at: Pos2, on: Option<H> },
    /// A hold off any mark lifted here: place a mark here.
    Place { at: Pos2 },
    /// A mark was held and let go without moving.
    Selected(H),
    /// A mark was dragged.
    Moved(H),
    /// The canvas was panned, or the press was cancelled.
    Nothing,
}

/// One press of a finger or the mouse on the canvas.
#[derive(Clone, Copy, Debug)]
pub struct Gesture<H> {
    origin: Pos2,
    started: f64,
    last: Pos2,
    under: Under<H>,
    phase: Phase,
}

impl<H: Copy> Gesture<H> {
    /// A press going down at `at`, at `time` seconds. With `pan` (space
    /// held, or another button) it pans from the start.
    pub fn new(at: Pos2, time: f64, under: Under<H>, pan: bool) -> Self {
        Gesture {
            origin: at,
            started: time,
            last: at,
            under,
            phase: if pan { Phase::Panning } else { Phase::Pending },
        }
    }

    /// Where the press is now.
    pub fn position(&self) -> Pos2 {
        self.last
    }

    /// Whether the press is still undecided or steering a placement, so a
    /// loupe should show where it would land.
    pub fn is_placing(&self) -> bool {
        matches!(self.phase, Phase::Pending | Phase::Held)
            && !matches!((self.phase, self.under), (Phase::Held, Under::Mark { .. }))
    }

    /// Whether a mark is being dragged, and which.
    pub fn moving(&self) -> Option<H> {
        match (self.phase, self.under) {
            (Phase::Moving, Under::Mark { mark, .. }) => Some(mark),
            _ => None,
        }
    }

    /// How long until a still press becomes a hold, for a repaint.
    pub fn hold_in(&self, time: f64) -> Option<f64> {
        (self.phase == Phase::Pending).then(|| (self.started + HOLD_SECONDS - time).max(0.0))
    }

    /// Stops the press doing anything: a second finger came down.
    pub fn cancel(&mut self) {
        self.phase = Phase::Cancelled;
    }

    /// The press is at `at` at `time` seconds.
    pub fn update(&mut self, at: Pos2, time: f64) -> Step<H> {
        let delta = at - self.last;
        self.last = at;
        let wandered = (at - self.origin).length() > MOVE_THRESHOLD;
        match self.phase {
            Phase::Pending if wandered => match self.under {
                Under::Mark {
                    mark,
                    drag_at_once: true,
                } => {
                    self.phase = Phase::Moving;
                    Step::Move {
                        mark,
                        offset: at - self.origin,
                    }
                }
                _ => {
                    self.phase = Phase::Panning;
                    Step::Pan(at - self.origin)
                }
            },
            Phase::Pending if time - self.started >= HOLD_SECONDS => {
                self.phase = Phase::Held;
                Step::Held(match self.under {
                    Under::Mark { mark, .. } => Some(mark),
                    Under::Nothing => None,
                })
            }
            Phase::Held => match self.under {
                Under::Mark { mark, .. } if wandered => {
                    self.phase = Phase::Moving;
                    Step::Move {
                        mark,
                        offset: at - self.origin,
                    }
                }
                _ => Step::Nothing,
            },
            Phase::Panning if delta != Vec2::ZERO => Step::Pan(delta),
            Phase::Moving => match self.under {
                Under::Mark { mark, .. } => Step::Move {
                    mark,
                    offset: at - self.origin,
                },
                Under::Nothing => Step::Nothing,
            },
            _ => Step::Nothing,
        }
    }

    /// The press lifted at `at`.
    pub fn release(mut self, at: Pos2, time: f64) -> Release<H> {
        // A last movement in the lifting frame counts.
        let _ = self.update(at, time);
        let on = match self.under {
            Under::Mark { mark, .. } => Some(mark),
            Under::Nothing => None,
        };
        match (self.phase, on) {
            (Phase::Pending, on) => Release::Tap { at, on },
            (Phase::Held, None) => Release::Place { at },
            (Phase::Held, Some(mark)) => Release::Selected(mark),
            (Phase::Moving, Some(mark)) => Release::Moved(mark),
            _ => Release::Nothing,
        }
    }
}

/// Whether a tap at `at`, `time` makes a double tap with the one before.
pub fn is_double_tap(before: Option<(Pos2, f64)>, at: Pos2, time: f64) -> bool {
    before.is_some_and(|(where_, when)| {
        time - when <= DOUBLE_TAP_SECONDS && (at - where_).length() <= DOUBLE_TAP_DISTANCE
    })
}
