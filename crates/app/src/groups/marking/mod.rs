//! Marking a group: scaling the photo, the aim point and the shots, on a
//! zoomable canvas with a loupe.
//!
//! Marks are always kept in their own units (photo pixels, or a grid's
//! millimetres); the canvas maps them to the screen through
//! [`geometry::Space`], straightening a photo with a four-corner scale.

mod chrome;
mod geometry;
mod gesture;
mod grid;
pub use grid::GridSizes;
mod history;
mod live;
mod paint;
mod scale;
#[cfg(test)]
mod tests;

use std::time::Duration;

use ballistics_store::{Calibration, GridShot, Group, GroupView, Marks, PhotoShot};
use eframe::egui::{self, Pos2, Rect, Sense, Vec2, pos2, vec2};
use groups_core::{Point, Reference};

use geometry::{Bounds, Camera, Space};
use gesture::{Gesture, Release, Under};
use history::History;
use live::LiveStats;
use paint::ShotLook;
use scale::{ScaleDraft, ScaleState};

use crate::i18n::{tr, trf};
use crate::{mobile, theme};

/// A photo ready to draw: its texture and its size in pixels.
pub struct PhotoImage {
    pub id: String,
    pub texture: egui::TextureHandle,
    pub size_px: [u32; 2],
    /// The texture's pixels as ink for the grid search: only for a photo
    /// opened for marking (`Textures::photo` with `ink`).
    pub ink: Option<groups_vision::grid::InkImage>,
}

/// What marking needs to know besides the marks.
pub struct MarkingContext<'a> {
    /// The photo, for a photo group; `None` while it is loading.
    pub photo: Option<&'a PhotoImage>,
    /// The bullet's diameter, for bullet-sized rings and edge-to-edge.
    pub calibre_mm: Option<f64>,
    /// The distance to the target, for angular sizes.
    pub distance_m: Option<f64>,
}

/// What the marking view asks of its caller after a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkingOutcome {
    /// Still marking.
    Working,
    /// The shooter cancelled.
    Cancelled,
    /// The shooter is done; read `calibration`, `marks` and `view`.
    Finished,
}

/// The steps of marking, in order. A grid group has no scale step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Scale,
    Aim,
    Shots,
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Scale => tr("Scale"),
            Step::Aim => tr("Aim"),
            Step::Shots => tr("Shots"),
        }
    }
}

/// Something on the canvas that can be selected, dragged and nudged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    /// A reference point of the scale, by index.
    Scale(usize),
    Aim,
    Shot(usize),
}

/// Everything undo goes back over.
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    scale: ScaleDraft,
    marks: Marks,
}

/// A press on the canvas in progress.
#[derive(Clone, Copy, Debug)]
struct Press {
    gesture: Gesture<Handle>,
    touch: bool,
    /// Where the pressed mark was when the press began, in display units.
    mark_start: Option<Point>,
}

/// The last tap, to tell a double tap.
#[derive(Clone, Copy, Debug)]
struct LastTap {
    at: Pos2,
    time: f64,
    /// The undo depth after the tap's change, when it made one.
    changed_to: Option<usize>,
}

/// A button or a key's request.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Command {
    Undo,
    Redo,
    ColdBore,
    Flier,
    Delete,
    Deselect,
    /// Move the selection one step (ten with `true`) this way on screen.
    Nudge(Vec2, bool),
    Zoom(ZoomTo),
    Next,
    Skip,
    NoAim,
    /// Back to an earlier step.
    GoTo(Step),
    Back,
    Cancel,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZoomTo {
    In,
    Out,
    Fit,
}

/// The statistics as the strip and the canvas show them.
struct LiveView {
    text: String,
    /// The counted shots' centre, mm, from the second shot.
    centre_mm: Option<Point>,
    mean_radius_mm: Option<f64>,
}

/// The state of marking one group, from scaling the photo to the last shot.
pub struct Marking {
    scale: ScaleDraft,
    calibration: Option<Calibration>,
    /// The calibration marking started with, kept while its draft is
    /// unchanged.
    initial_calibration: Option<Calibration>,
    marks: Marks,
    view: Option<GroupView>,
    step: Step,
    history: History<Snapshot>,
    /// The state the last change was recorded at.
    committed: Snapshot,
    initial: Snapshot,
    selected: Option<Handle>,
    press: Option<Press>,
    last_tap: Option<LastTap>,
    live: LiveStats,
    /// Zoom to the aim point on the next frame.
    zoom_to_aim: bool,
    /// Fit the whole photo or grid on the next frame.
    fit: bool,
    /// When Cancel asked whether to discard the changes.
    discard_asked: Option<f64>,
    /// The time of the frame being shown, seconds.
    now: f64,
    finder: grid::Finder,
    /// What the scale step's warnings need: the photo's pixels, the calibre
    /// and the photo's size.
    sight: scale::Sight,
    /// The part of the photo on screen last frame, photo px.
    visible_px: Option<[f64; 4]>,
}

/// How far (points) round a mark a press still takes it.
fn reach(touch: bool) -> f32 {
    if touch { 22.0 } else { 12.0 }
}

/// A shot's ring radius in display units: half the calibre at the photo's
/// scale there. `None` without a calibre or a scale, for a fixed ring.
fn ring_radius(
    space: &Space,
    calibration: Option<&groups_core::Calibration>,
    calibre_mm: Option<f64>,
    mark: [f64; 2],
) -> Option<f64> {
    let calibre = calibre_mm.filter(|calibre| *calibre > 0.0 && calibre.is_finite())?;
    let at = space.to_display(mark)?;
    let mm_per_unit = space.mm_per_unit(calibration, at)?;
    (mm_per_unit > 0.0).then(|| 0.5 * calibre / mm_per_unit)
}

/// Where marks may be, in mark units: on the photo, or on the grid.
fn mark_bounds(space: &Space) -> Bounds {
    match *space {
        Space::Photo { size_px } | Space::Straightened { size_px, .. } => {
            Bounds::new(Point::ORIGIN, Point::new(size_px[0], size_px[1]))
        }
        Space::Grid { size_mm: [w, h] } => {
            Bounds::new(Point::new(-0.5 * w, -0.5 * h), Point::new(0.5 * w, 0.5 * h))
        }
    }
}

/// A length to a tenth, without a trailing ".0".
pub(super) fn tenths(mm: f64) -> String {
    let text = format!("{mm:.1}");
    text.strip_suffix(".0").map_or(text.clone(), str::to_owned)
}

fn xy(p: Point) -> [f64; 2] {
    [p.x, p.y]
}

fn point([x, y]: [f64; 2]) -> Point {
    Point::new(x, y)
}

impl Marking {
    fn start(
        scale: ScaleDraft,
        calibration: Option<Calibration>,
        marks: Marks,
        view: Option<GroupView>,
        step: Step,
    ) -> Self {
        let snapshot = Snapshot {
            scale: scale.clone(),
            marks: marks.clone(),
        };
        Marking {
            scale,
            calibration,
            initial_calibration: calibration,
            marks,
            view,
            step,
            history: History::default(),
            committed: snapshot.clone(),
            initial: snapshot,
            selected: None,
            press: None,
            last_tap: None,
            live: LiveStats::default(),
            zoom_to_aim: false,
            fit: false,
            discard_asked: None,
            now: 0.0,
            finder: grid::Finder::new(false),
            sight: scale::Sight::default(),
            visible_px: None,
        }
    }

    /// Starts a new group on a photo. With no `calibration` it starts at the
    /// scale step, otherwise at the aim point.
    pub fn new_photo(photo_id: String, calibration: Option<Calibration>) -> Self {
        let scale = ScaleDraft::from_store(calibration.as_ref());
        let step = match scale.state() {
            ScaleState::Ready(_) => Step::Aim,
            _ => Step::Scale,
        };
        let mut marking = Marking::start(
            scale,
            calibration,
            Marks::Photo(ballistics_store::PhotoMarks {
                photo_id,
                aim_px: None,
                shots: Vec::new(),
            }),
            None,
            step,
        );
        marking.finder.auto = step == Step::Scale && calibration.is_none();
        marking
    }

    /// Starts with the grid square sizes remembered on this device; a blank
    /// scale takes the last square used.
    pub fn with_grid_sizes(mut self, sizes: grid::GridSizes) -> Self {
        self.finder.sizes = sizes;
        if let Some(mm) = sizes.last_mm.filter(|mm| mm.is_finite() && *mm > 0.0)
            && self.calibration.is_none()
        {
            self.scale.square_mm = mm;
            self.committed.scale.square_mm = mm;
            self.initial.scale.square_mm = mm;
        }
        self
    }

    /// The grid square sizes to remember: as confirmed, and the square the
    /// scale was set with when it was set from grid squares.
    pub fn grid_sizes(&self) -> grid::GridSizes {
        let mut sizes = self.finder.sizes;
        if self.scale.mode == scale::ScaleMode::GridSquares && self.calibration.is_some() {
            sizes.last_mm = Some(self.scale.square_mm);
        }
        sizes
    }

    /// Starts a new group on a blank grid of this size, at the aim point.
    pub fn new_grid(width_mm: f64, height_mm: f64) -> Self {
        Marking::start(
            ScaleDraft::default(),
            None,
            Marks::Grid(ballistics_store::GridMarks {
                width_mm,
                height_mm,
                aim_mm: None,
                shots: Vec::new(),
            }),
            None,
            Step::Aim,
        )
    }

    /// Reopens a saved group's shots for editing, in its saved framing. A
    /// photo without a usable scale opens at the scale step.
    pub fn edit(group: &Group, calibration: Option<&Calibration>, view: Option<GroupView>) -> Self {
        let photo = matches!(group.marks, Marks::Photo(_));
        let calibration = calibration.copied().filter(|_| photo);
        let scale = ScaleDraft::from_store(calibration.as_ref());
        let step = match scale.state() {
            ScaleState::Ready(_) => Step::Shots,
            _ if !photo => Step::Shots,
            _ => Step::Scale,
        };
        Marking::start(scale, calibration, group.marks.clone(), view, step)
    }

    /// Draws the current step and handles input.
    pub fn show(&mut self, ui: &mut egui::Ui, context: &MarkingContext) -> MarkingOutcome {
        self.now = ui.input(|input| input.time);
        self.refresh_calibration();
        self.sight = scale::Sight {
            can_search: context.photo.is_some_and(|photo| photo.ink.is_some()),
            calibre_mm: context.calibre_mm,
            photo_short_px: context
                .photo
                .map(|photo| f64::from(photo.size_px[0].min(photo.size_px[1]))),
        };
        self.find_grid(context);
        let narrow = mobile::is_narrow(ui.ctx());
        let full = ui.available_rect_before_wrap();
        let live = self.live_view(&crate::units::prefs(ui.ctx()), context);
        let mut commands = Vec::new();
        let canvas_top = self.chrome_top(ui, full, narrow, &live, &mut commands);
        let canvas_bottom = if narrow {
            self.chrome_bottom(ui, full, &mut commands)
        } else {
            full.bottom()
        };
        let canvas = Rect::from_min_max(
            pos2(full.left(), canvas_top),
            pos2(full.right(), canvas_bottom.max(canvas_top + 40.0)),
        );
        self.keys(ui, &mut commands);
        let mut outcome = MarkingOutcome::Working;
        let space = self.space(context);
        for command in commands {
            ui.ctx().request_repaint();
            if let Some(ending) = self.run_command(command, space.as_ref()) {
                outcome = ending;
            }
        }
        if let Some(ending) = self.discard_dialog(ui) {
            outcome = ending;
        }
        self.refresh_calibration();
        self.canvas(ui, canvas, context, &live);
        let pointer_down = ui.input(|input| input.pointer.any_down());
        if !pointer_down && self.press.is_none() {
            self.commit();
        }
        self.finder.after_frame(ui.ctx());
        ui.advance_cursor_after_rect(full);
        outcome
    }

    /// Runs the grid search when it is due, or once by itself on a new
    /// photo, and places the crossings it finds.
    fn find_grid(&mut self, context: &MarkingContext) {
        let Some(photo) = context.photo else {
            return;
        };
        let Some(ink) = photo.ink.as_ref() else {
            return;
        };
        if self.step != Step::Scale || self.scale.mode != scale::ScaleMode::GridSquares {
            if self.finder.due().is_some() {
                self.finder.search = grid::Search::Idle;
            }
            return;
        }
        if self.finder.auto && self.scale.corners.is_empty() {
            self.finder.auto = false;
            self.finder.ask(true);
            return;
        }
        let Some(whole) = self.finder.due() else {
            return;
        };
        let to_ink = [
            f64::from(ink.width) / f64::from(photo.size_px[0].max(1)),
            f64::from(ink.height) / f64::from(photo.size_px[1].max(1)),
        ];
        let region = match (whole, self.visible_px) {
            (false, Some([x0, y0, x1, y1])) => [
                x0 * to_ink[0],
                y0 * to_ink[1],
                x1 * to_ink[0],
                y1 * to_ink[1],
            ],
            _ => [0.0, 0.0, f64::from(ink.width), f64::from(ink.height)],
        };
        let result =
            groups_vision::grid::find_grid(ink, region, &groups_vision::grid::Hints::default());
        self.finder.finish(
            result,
            |[x, y]| [x / to_ink[0], y / to_ink[1]],
            &mut self.scale,
        );
        self.selected = None;
    }

    /// The photo's scale, to keep on the photo record.
    pub fn calibration(&self) -> Option<&Calibration> {
        self.calibration.as_ref()
    }

    /// The aim point and shots, to keep on the group.
    pub fn marks(&self) -> &Marks {
        &self.marks
    }

    /// The framing on screen, kept on this device only.
    pub fn view(&self) -> Option<GroupView> {
        self.view
    }

    /// How many views deep marking is, for the browser's Back button.
    pub fn depth(&self) -> usize {
        self.steps()
            .iter()
            .position(|step| *step == self.step)
            .unwrap_or(0)
            + 1
    }

    /// Goes back one step. `false` when already at the first step, so the
    /// caller closes marking (asking first if `unsaved`).
    pub fn back(&mut self) -> bool {
        let index = self.depth() - 1;
        if index == 0 {
            self.press = None;
            return false;
        }
        self.go_to(self.steps()[index - 1]);
        true
    }

    /// Whether anything was marked or changed since marking started.
    pub fn unsaved(&self) -> bool {
        self.marks != self.initial.marks || (self.is_photo() && self.scale != self.initial.scale)
    }

    fn is_photo(&self) -> bool {
        matches!(self.marks, Marks::Photo(_))
    }

    fn steps(&self) -> &'static [Step] {
        if self.is_photo() {
            &[Step::Scale, Step::Aim, Step::Shots]
        } else {
            &[Step::Aim, Step::Shots]
        }
    }

    fn go_to(&mut self, step: Step) {
        self.commit();
        self.press = None;
        self.selected = None;
        self.last_tap = None;
        if step == Step::Shots && self.step == Step::Aim && self.aim().is_some() {
            self.zoom_to_aim = true;
        }
        self.step = step;
    }

    // --- State and undo ---

    fn refresh_calibration(&mut self) {
        self.calibration = if !self.is_photo() {
            None
        } else if self.scale == self.initial.scale && self.initial_calibration.is_some() {
            self.initial_calibration
        } else {
            self.scale.to_store()
        };
    }

    /// The photo's scale as groups-core uses it, when there is one.
    fn core_calibration(&self) -> Option<groups_core::Calibration> {
        self.calibration
            .as_ref()
            .and_then(|calibration| crate::groups::measure::scale(calibration).ok())
    }

    /// Records a change since the last one, if there was one.
    fn commit(&mut self) -> bool {
        if self.scale == self.committed.scale && self.marks == self.committed.marks {
            return false;
        }
        let now = Snapshot {
            scale: self.scale.clone(),
            marks: self.marks.clone(),
        };
        let before = std::mem::replace(&mut self.committed, now);
        self.history.record(before);
        true
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.scale = snapshot.scale.clone();
        self.marks = snapshot.marks.clone();
        self.committed = snapshot;
        self.refresh_calibration();
        if self
            .selected
            .is_some_and(|handle| self.handle_mark(handle).is_none())
        {
            self.selected = None;
        }
    }

    fn undo(&mut self) {
        self.commit();
        if let Some(before) = self.history.undo(self.committed.clone()) {
            self.restore(before);
        }
    }

    fn redo(&mut self) {
        self.commit();
        if let Some(after) = self.history.redo(self.committed.clone()) {
            self.restore(after);
        }
    }

    // --- The marks ---

    fn aim(&self) -> Option<[f64; 2]> {
        match &self.marks {
            Marks::Photo(photo) => photo.aim_px,
            Marks::Grid(grid) => grid.aim_mm,
        }
    }

    fn set_aim(&mut self, aim: Option<[f64; 2]>) {
        match &mut self.marks {
            Marks::Photo(photo) => photo.aim_px = aim,
            Marks::Grid(grid) => grid.aim_mm = aim,
        }
    }

    fn shot_count(&self) -> usize {
        match &self.marks {
            Marks::Photo(photo) => photo.shots.len(),
            Marks::Grid(grid) => grid.shots.len(),
        }
    }

    /// A shot's place and its cold-bore and called-flier flags.
    fn shot(&self, index: usize) -> Option<([f64; 2], bool, bool)> {
        match &self.marks {
            Marks::Photo(photo) => photo
                .shots
                .get(index)
                .map(|shot| ([shot.x_px, shot.y_px], shot.cold_bore, shot.called_flier)),
            Marks::Grid(grid) => grid
                .shots
                .get(index)
                .map(|shot| ([shot.x_mm, shot.y_mm], shot.cold_bore, shot.called_flier)),
        }
    }

    fn add_shot(&mut self, at: [f64; 2]) -> Option<usize> {
        if self.shot_count() >= ballistics_store::groups::MAX_SHOTS {
            return None;
        }
        let [x, y] = at;
        match &mut self.marks {
            Marks::Photo(photo) => photo.shots.push(PhotoShot {
                x_px: x,
                y_px: y,
                cold_bore: false,
                called_flier: false,
                velocity_mps: None,
            }),
            Marks::Grid(grid) => grid.shots.push(GridShot {
                x_mm: x,
                y_mm: y,
                cold_bore: false,
                called_flier: false,
                velocity_mps: None,
            }),
        }
        Some(self.shot_count() - 1)
    }

    /// Changes a shot's flags: `cold` toggles cold bore, otherwise called
    /// flier.
    fn toggle_shot(&mut self, index: usize, cold: bool) {
        let flags = match &mut self.marks {
            Marks::Photo(photo) => photo
                .shots
                .get_mut(index)
                .map(|shot| (&mut shot.cold_bore, &mut shot.called_flier)),
            Marks::Grid(grid) => grid
                .shots
                .get_mut(index)
                .map(|shot| (&mut shot.cold_bore, &mut shot.called_flier)),
        };
        if let Some((cold_bore, called_flier)) = flags {
            let flag = if cold { cold_bore } else { called_flier };
            *flag = !*flag;
        }
    }

    fn remove_shot(&mut self, index: usize) {
        match &mut self.marks {
            Marks::Photo(photo) if index < photo.shots.len() => {
                photo.shots.remove(index);
            }
            Marks::Grid(grid) if index < grid.shots.len() => {
                grid.shots.remove(index);
            }
            _ => {}
        }
    }

    fn handle_mark(&self, handle: Handle) -> Option<[f64; 2]> {
        match handle {
            Handle::Scale(index) => self.scale.handle(index),
            Handle::Aim => self.aim(),
            Handle::Shot(index) => self.shot(index).map(|(at, ..)| at),
        }
    }

    fn set_handle_mark(&mut self, handle: Handle, at: [f64; 2]) {
        match handle {
            Handle::Scale(index) => self.scale.set_handle(index, at),
            Handle::Aim => self.set_aim(Some(at)),
            Handle::Shot(index) => {
                let [x, y] = at;
                match &mut self.marks {
                    Marks::Photo(photo) => {
                        if let Some(shot) = photo.shots.get_mut(index) {
                            (shot.x_px, shot.y_px) = (x, y);
                        }
                    }
                    Marks::Grid(grid) => {
                        if let Some(shot) = grid.shots.get_mut(index) {
                            (shot.x_mm, shot.y_mm) = (x, y);
                        }
                    }
                }
            }
        }
    }

    /// The marks this step lets the shooter select and move.
    fn handles(&self) -> Vec<(Handle, [f64; 2])> {
        match self.step {
            Step::Scale => self
                .scale
                .handles()
                .into_iter()
                .map(|(index, at)| (Handle::Scale(index), at))
                .collect(),
            Step::Aim => self.aim().map(|at| (Handle::Aim, at)).into_iter().collect(),
            Step::Shots => (0..self.shot_count())
                .filter_map(|index| self.shot(index).map(|(at, ..)| (Handle::Shot(index), at)))
                .collect(),
        }
    }

    /// Whether a tap off the marks would place one in this step.
    fn places(&self) -> bool {
        match self.step {
            Step::Scale => matches!(self.scale.state(), ScaleState::ToTap(_)),
            Step::Aim | Step::Shots => true,
        }
    }

    /// Places what this step places at `at` (mark units).
    fn place(&mut self, at: [f64; 2]) {
        self.selected = match self.step {
            Step::Scale => self.scale.place(at).map(Handle::Scale),
            Step::Aim => {
                self.set_aim(Some(at));
                Some(Handle::Aim)
            }
            Step::Shots => self.add_shot(at).map(Handle::Shot),
        };
    }

    /// Carries out a button's or key's command; the outcome when it ends
    /// marking.
    fn run_command(&mut self, command: Command, space: Option<&Space>) -> Option<MarkingOutcome> {
        match command {
            Command::Cancel => self.cancel(),
            Command::Done => {
                self.commit();
                Some(MarkingOutcome::Finished)
            }
            Command::Back if self.back() => None,
            Command::Back => self.cancel(),
            command => {
                self.apply(command, space);
                None
            }
        }
    }

    /// Cancels marking, asking first when that would drop changes.
    fn cancel(&mut self) -> Option<MarkingOutcome> {
        self.commit();
        if self.unsaved() {
            self.discard_asked = Some(self.now);
            None
        } else {
            Some(MarkingOutcome::Cancelled)
        }
    }

    /// "Discard your changes?", while Cancel waits for an answer.
    fn discard_dialog(&mut self, ui: &egui::Ui) -> Option<MarkingOutcome> {
        let asked_at = self.discard_asked?;
        let context = ui.ctx().clone();
        let narrow = mobile::is_narrow(&context);
        let width = (context.content_rect().width() - 48.0).min(400.0);
        let height = if narrow { mobile::TOUCH } else { 0.0 };
        let (mut keep, mut discard) = (false, false);
        let modal = egui::Modal::new(egui::Id::unique("marking-discard")).show(&context, |ui| {
            ui.set_width(width);
            ui.heading(tr("Discard your changes?"));
            ui.label(tr("They aren't saved yet."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                keep = ui
                    .add(egui::Button::new(tr("Keep editing")).min_size(vec2(0.0, height)))
                    .clicked();
                let ready = crate::widgets::confirm_ready(ui, asked_at);
                let text = egui::RichText::new(tr("Discard")).color(ui.visuals().error_fg_color);
                discard = ui
                    .add_enabled(ready, egui::Button::new(text).min_size(vec2(0.0, height)))
                    .clicked();
            });
        });
        self.answer_discard(discard, keep || modal.should_close())
    }

    /// The shooter's answer to "Discard your changes?".
    fn answer_discard(&mut self, discard: bool, keep: bool) -> Option<MarkingOutcome> {
        if discard || keep {
            self.discard_asked = None;
        }
        discard.then_some(MarkingOutcome::Cancelled)
    }

    fn apply(&mut self, command: Command, space: Option<&Space>) {
        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ColdBore | Command::Flier => {
                if let Some(Handle::Shot(index)) = self.selected {
                    self.toggle_shot(index, command == Command::ColdBore);
                }
            }
            Command::Delete => {
                match self.selected {
                    Some(Handle::Shot(index)) => self.remove_shot(index),
                    Some(Handle::Aim) => self.set_aim(None),
                    Some(Handle::Scale(index)) => self.scale.remove(index),
                    None => {}
                }
                self.selected = None;
            }
            Command::Deselect => self.selected = None,
            Command::Nudge(direction, big) => {
                let (Some(handle), Some(space)) = (self.selected, space) else {
                    return;
                };
                let step = match (space, big) {
                    (Space::Grid { .. }, false) => 0.5,
                    (Space::Grid { .. }, true) => 5.0,
                    (_, false) => 1.0,
                    (_, true) => 10.0,
                };
                if let Some(at) = self.handle_mark(handle)
                    && let Some(moved) = space.nudged(at, direction, step)
                {
                    let moved = mark_bounds(space).clamp(point(moved));
                    self.set_handle_mark(handle, xy(moved));
                }
            }
            Command::Next => match self.step {
                Step::Scale => {
                    if matches!(self.scale.state(), ScaleState::Ready(_))
                        && !self.finder.awaits_size(&self.scale)
                    {
                        self.go_to(Step::Aim);
                    }
                }
                Step::Aim => self.go_to(Step::Shots),
                Step::Shots => {}
            },
            Command::Skip => {
                if self.step == Step::Scale {
                    self.go_to(Step::Aim);
                }
            }
            Command::NoAim => {
                self.set_aim(None);
                self.go_to(Step::Shots);
                self.fit = true;
            }
            Command::GoTo(step) => {
                let index = |step: Step| self.steps().iter().position(|s| *s == step);
                if index(step) < index(self.step) {
                    self.go_to(step);
                }
            }
            Command::Zoom(to) => self.zoom_view(to),
            Command::Back | Command::Cancel | Command::Done => {}
        }
    }

    // --- The canvas ---

    /// What the canvas shows now; `None` while the photo loads.
    fn space(&self, context: &MarkingContext) -> Option<Space> {
        match &self.marks {
            Marks::Grid(grid) => Some(Space::Grid {
                size_mm: [grid.width_mm, grid.height_mm],
            }),
            Marks::Photo(_) => {
                let photo = context.photo?;
                let size_px = [photo.size_px[0] as f64, photo.size_px[1] as f64];
                if self.step != Step::Scale
                    && let Some(calibration) = self.core_calibration()
                    && let Reference::Rectangle {
                        width_mm,
                        height_mm,
                        ..
                    } = calibration.reference()
                {
                    Some(Space::Straightened {
                        size_px,
                        calibration,
                        rectangle_mm: [width_mm, height_mm],
                    })
                } else {
                    Some(Space::Photo { size_px })
                }
            }
        }
    }

    fn live_view(&mut self, units: &crate::units::UnitPrefs, context: &MarkingContext) -> LiveView {
        let shots = self.shot_count();
        let result = self.live.get(
            &self.marks,
            self.calibration.as_ref(),
            context.calibre_mm,
            context.distance_m,
        );
        let spread = result
            .as_ref()
            .ok()
            .and_then(|measured| measured.stats.as_ref())
            .filter(|stats| stats.spread.is_some());
        LiveView {
            text: live::strip_text(units, result, shots, context.distance_m),
            centre_mm: spread.map(|stats| stats.centre),
            mean_radius_mm: spread.map(|stats| stats.mean_radius),
        }
    }

    /// The camera for the stored view, or fitting the whole content.
    fn camera(&self, space: &Space, rect: Rect, fit: f64) -> Camera {
        let extent = space.extent();
        let whole = Camera {
            centre: extent.centre(),
            scale: fit,
            screen_centre: rect.center(),
        };
        match self.view {
            Some(view) if !self.fit => match space.to_display(view.centre) {
                Some(centre) => Camera {
                    centre,
                    scale: fit * view.zoom,
                    screen_centre: rect.center(),
                },
                None => whole,
            },
            _ => whole,
        }
    }

    /// The camera close on the aim point, for marking shots round it: about
    /// 100 mm across the view's shorter side.
    fn camera_on_aim(&self, space: &Space, rect: Rect, fit: f64, camera: Camera) -> Camera {
        let Some(aim) = self.aim().and_then(|aim| space.to_display(aim)) else {
            return camera;
        };
        let core = self.core_calibration();
        let scale = match space.mm_per_unit(core.as_ref(), aim) {
            Some(mm_per_unit) => rect.width().min(rect.height()) as f64 * mm_per_unit / 100.0,
            None => fit * 2.5,
        };
        Camera {
            centre: aim,
            scale: scale.max(fit),
            screen_centre: rect.center(),
        }
    }

    /// The mark under a press at `at`, if any.
    fn under(
        &self,
        at: Pos2,
        space: &Space,
        camera: &Camera,
        touch: bool,
        calibre_mm: Option<f64>,
    ) -> Under<Handle> {
        let core = self.core_calibration();
        let nearest = self
            .handles()
            .into_iter()
            .filter_map(|(handle, mark)| {
                let screen = camera.to_screen(space.to_display(mark)?);
                let ring = match handle {
                    Handle::Shot(_) => ring_radius(space, core.as_ref(), calibre_mm, mark)
                        .map_or(paint::FIXED_RING_RADIUS, |r| (r * camera.scale) as f32),
                    _ => 0.0,
                };
                let distance = (screen - at).length();
                (distance <= reach(touch).max(ring + 4.0)).then_some((handle, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match nearest {
            None => Under::Nothing,
            Some((mark, _)) => Under::Mark {
                mark,
                drag_at_once: match mark {
                    Handle::Shot(_) => self.selected == Some(mark),
                    _ => true,
                },
            },
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, rect: Rect, context: &MarkingContext, live: &LiveView) {
        let palette = theme::palette(ui.ctx());
        let id = ui.scope_id().with("marking-canvas");
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, palette.sunken);
        let Some(space) = self.space(context) else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tr("Loading the photo…"),
                egui::TextStyle::Body.resolve(ui.style()),
                palette.text_muted,
            );
            return;
        };
        let narrow = mobile::is_narrow(ui.ctx());
        let extent = space.extent();
        let fit = geometry::fit_scale(&extent, rect);
        let max_scale = space.max_scale();
        let mut camera = self.camera(&space, rect, fit);
        self.fit = false;
        if std::mem::take(&mut self.zoom_to_aim) {
            camera = self.camera_on_aim(&space, rect, fit, camera);
        }

        let buttons = chrome::zoom_buttons(rect, narrow);
        let on_button = |at: Pos2| buttons.iter().any(|(button, _)| button.contains(at));
        let input = ui.input(|input| {
            (
                input.time,
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.pointer.interact_pos(),
                input.any_touches(),
                input.multi_touch(),
                input.zoom_delta() as f64,
                input.smooth_scroll_delta(),
                input.key_down(egui::Key::Space),
                input.pointer.hover_pos(),
            )
        });
        let (time, pressed, down, pointer, touches, multi, zoom_delta, scroll, space_key, hover) =
            input;

        if let Some(multi) = multi {
            if response.contains_pointer() || self.press.is_some() {
                if let Some(press) = &mut self.press {
                    press.gesture.cancel();
                }
                camera = camera
                    .zoomed(multi.zoom_delta as f64, multi.center_pos)
                    .panned(multi.translation_delta);
            }
        } else if response.hovered()
            && let Some(at) = hover
        {
            if zoom_delta != 1.0 {
                camera = camera.zoomed(zoom_delta, at);
            } else if scroll != Vec2::ZERO {
                camera = camera.panned(scroll);
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            camera = camera.panned(response.drag_delta());
        }

        if pressed
            && self.press.is_none()
            && multi.is_none()
            && response.contains_pointer()
            && let Some(at) = pointer
            && !on_button(at)
        {
            let under = self.under(at, &space, &camera, touches, context.calibre_mm);
            let mark_start = match under {
                Under::Mark { mark, .. } => self
                    .handle_mark(mark)
                    .and_then(|mark| space.to_display(mark)),
                Under::Nothing => None,
            };
            self.press = Some(Press {
                gesture: Gesture::new(at, time, under, space_key),
                touch: touches,
                mark_start,
            });
        }
        if let Some(mut press) = self.press {
            let at = pointer.unwrap_or(press.gesture.position());
            if down {
                let step = press.gesture.update(at, time);
                self.press = Some(press);
                camera = self.on_step(step, press, &space, camera);
                if let Some(wait) = press.gesture.hold_in(time) {
                    ui.ctx()
                        .request_repaint_after(Duration::from_secs_f64(wait + 0.01));
                }
            } else {
                self.press = None;
                let release = press.gesture.release(at, time);
                if self.on_release(release, time, &space, &camera) {
                    camera = Camera {
                        centre: extent.centre(),
                        scale: fit,
                        screen_centre: rect.center(),
                    };
                }
            }
        }
        let camera = geometry::clamped(camera, &extent, fit, max_scale);
        self.visible_px = match space {
            Space::Photo { .. } => extent
                .intersect(&camera.visible(rect))
                .map(|seen| [seen.min.x, seen.min.y, seen.max.x, seen.max.y]),
            _ => None,
        };
        self.view = Some(GroupView {
            zoom: camera.scale / fit,
            centre: space
                .to_mark(camera.centre)
                .or(self.view.map(|view| view.centre))
                .unwrap_or_else(|| xy(extent.centre())),
        });

        self.paint_scene(&painter, &space, &camera, rect, context, live);
        for (button_rect, to) in buttons {
            if chrome::zoom_button(ui, button_rect, to) {
                self.zoom_view(to);
                ui.ctx().request_repaint();
            }
        }
        if let Some(press) = self.press {
            let focus = match press.gesture.moving() {
                Some(_) => press
                    .gesture
                    .moving()
                    .and_then(|handle| self.handle_mark(handle))
                    .and_then(|mark| space.to_display(mark)),
                None if press.gesture.is_placing() && self.places() => {
                    Some(camera.to_display(press.gesture.position()))
                }
                None => None,
            };
            if let Some(focus) = focus {
                self.paint_loupe(ui, &space, &camera, rect, focus, press.touch, context, live);
            }
        }
    }

    /// Acts on a frame of a press; the camera after it.
    fn on_step(
        &mut self,
        step: gesture::Step<Handle>,
        press: Press,
        space: &Space,
        camera: Camera,
    ) -> Camera {
        match step {
            gesture::Step::Nothing => camera,
            gesture::Step::Held(handle) => {
                if handle.is_some() {
                    self.selected = handle;
                }
                camera
            }
            gesture::Step::Pan(delta) => camera.panned(delta),
            gesture::Step::Move { mark, offset } => {
                if let Some(start) = press.mark_start {
                    let moved =
                        start + Point::new(offset.x as f64, offset.y as f64) * (1.0 / camera.scale);
                    if let Some(at) = space.to_mark(moved) {
                        let at = mark_bounds(space).clamp(point(at));
                        self.set_handle_mark(mark, xy(at));
                        self.selected = Some(mark);
                    }
                }
                camera
            }
        }
    }

    /// Acts on a press ending; `true` when a double tap asks to fit the
    /// whole photo.
    fn on_release(
        &mut self,
        release: Release<Handle>,
        time: f64,
        space: &Space,
        camera: &Camera,
    ) -> bool {
        let landing = |at: Pos2| {
            space
                .to_mark(camera.to_display(at))
                .filter(|mark| mark_bounds(space).contains(point(*mark)))
        };
        match release {
            Release::Tap { at, on } => {
                let last = self.last_tap.take();
                if gesture::is_double_tap(last.map(|tap| (tap.at, tap.time)), at, time) {
                    if let Some(depth) = last.and_then(|tap| tap.changed_to)
                        && depth == self.history.undo_depth()
                        && let Some(before) = self.history.forget_last()
                    {
                        self.restore(before);
                    }
                    return true;
                }
                match on {
                    Some(handle) if self.step == Step::Scale => self.selected = Some(handle),
                    _ => {
                        if let Some(mark) = landing(at) {
                            self.place(mark);
                        }
                    }
                }
                let changed = self.commit();
                self.last_tap = Some(LastTap {
                    at,
                    time,
                    changed_to: changed.then(|| self.history.undo_depth()),
                });
                false
            }
            Release::Place { at } => {
                if let Some(mark) = landing(at) {
                    self.place(mark);
                }
                self.last_tap = None;
                false
            }
            Release::Selected(handle) => {
                self.selected = Some(handle);
                false
            }
            Release::Moved(_) | Release::Nothing => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_loupe(
        &self,
        ui: &egui::Ui,
        space: &Space,
        camera: &Camera,
        canvas: Rect,
        focus: Point,
        touch: bool,
        context: &MarkingContext,
        live: &LiveView,
    ) {
        let palette = theme::palette(ui.ctx());
        let size = (canvas.width().min(canvas.height()) * 0.34).clamp(96.0, 150.0);
        let rect = geometry::loupe_rect(camera.to_screen(focus), canvas, size, touch);
        let loupe = Camera {
            centre: focus,
            scale: camera.scale * 3.0,
            screen_centre: rect.center(),
        };
        let painter = ui.painter_at(rect.intersect(canvas));
        painter.rect_filled(rect, 0.0, palette.sunken);
        self.paint_scene(&painter, space, &loupe, rect, context, live);
        paint::loupe_frame(&ui.painter_at(canvas), rect);
    }

    /// The photo or grid and every mark on it, through `camera`.
    fn paint_scene(
        &self,
        painter: &egui::Painter,
        space: &Space,
        camera: &Camera,
        rect: Rect,
        context: &MarkingContext,
        live: &LiveView,
    ) {
        let extent = space.extent();
        let visible = camera.visible(rect);
        match space {
            Space::Grid { .. } => {
                let dark = theme::palette(painter.ctx()).dark;
                paint::grid(painter, &extent, camera, &visible, dark);
            }
            _ => {
                if let (Some(photo), Some(region)) = (context.photo, extent.intersect(&visible)) {
                    painter.add(paint::photo_mesh(
                        photo.texture.id(),
                        space,
                        &region,
                        camera,
                        16,
                    ));
                }
            }
        }
        let core = self.core_calibration();
        let at = |mark: [f64; 2]| space.to_display(mark).map(|p| camera.to_screen(p));

        if self.step == Step::Scale {
            self.paint_scale(painter, space, camera);
        }

        if self.step == Step::Shots
            && let (Some(centre), Some(radius)) = (live.centre_mm, live.mean_radius_mm)
        {
            let ring: Option<Vec<Pos2>> = (0..72)
                .map(|i| {
                    let angle = std::f64::consts::TAU * i as f64 / 72.0;
                    let mm = centre + Point::new(angle.cos(), angle.sin()) * radius;
                    space
                        .mm_to_display(core.as_ref(), mm)
                        .map(|p| camera.to_screen(p))
                })
                .collect();
            if let Some(ring) = ring {
                paint::outlined_line(painter, &ring, paint::STATS, true, true);
            }
            if let Some(centre) = space.mm_to_display(core.as_ref(), centre) {
                paint::centre(painter, camera.to_screen(centre));
            }
        }

        if let Some(aim) = self.aim().and_then(at) {
            paint::aim(
                painter,
                aim,
                self.step == Step::Aim && self.selected == Some(Handle::Aim),
            );
        }

        let shots: Vec<(usize, [f64; 2], bool, bool, Pos2)> = (0..self.shot_count())
            .filter_map(|index| {
                let (mark, cold_bore, called_flier) = self.shot(index)?;
                Some((index, mark, cold_bore, called_flier, at(mark)?))
            })
            .collect();
        // Numbers go outside the rings, away from the group's middle.
        let middle = live
            .centre_mm
            .and_then(|mm| space.mm_to_display(core.as_ref(), mm))
            .map(|display| camera.to_screen(display))
            .or_else(|| {
                let count = shots.len().max(1) as f32;
                let sum = shots
                    .iter()
                    .fold(Vec2::ZERO, |sum, shot| sum + shot.4.to_vec2());
                (!shots.is_empty()).then(|| (sum / count).to_pos2())
            });
        for (index, mark, cold_bore, called_flier, screen) in shots {
            let radius = ring_radius(space, core.as_ref(), context.calibre_mm, mark)
                .map_or(paint::FIXED_RING_RADIUS, |r| (r * camera.scale) as f32);
            let look = if cold_bore {
                ShotLook::ColdBore
            } else if called_flier {
                ShotLook::Flier
            } else {
                ShotLook::Counted
            };
            let selected = self.step == Step::Shots && self.selected == Some(Handle::Shot(index));
            let away = middle.map_or(Vec2::ZERO, |middle| screen - middle);
            paint::shot(painter, screen, radius, index + 1, look, selected, away);
        }
    }

    /// The scale's points, and what they make.
    fn paint_scale(&self, painter: &egui::Painter, space: &Space, camera: &Camera) {
        let at = |mark: [f64; 2]| space.to_display(mark).map(|p| camera.to_screen(p));
        let state = self.scale.state();
        match self.scale.mode {
            scale::ScaleMode::TwoPoints => {
                if let [Some(a), Some(b)] = self.scale.points.map(|p| p.and_then(at)) {
                    let color = match state {
                        ScaleState::Ready(_) => paint::STATS,
                        _ => theme::ALARM,
                    };
                    paint::outlined_line(painter, &[a, b], color, true, false);
                    if self.scale.distance_mm > 0.0 {
                        paint::label_pill(
                            painter,
                            a + 0.5 * (b - a) + vec2(0.0, -10.0),
                            &format!("{} mm", self.scale.distance_mm),
                            egui::Align2::CENTER_BOTTOM,
                        );
                    }
                }
            }
            scale::ScaleMode::GridSquares | scale::ScaleMode::FourCorners => {
                if let ScaleState::Ready(calibration) = state
                    && let Reference::Rectangle {
                        corners,
                        width_mm,
                        height_mm,
                    } = calibration.reference()
                    && let Some(quad) = corners
                        .iter()
                        .map(|corner| at(xy(*corner)))
                        .collect::<Option<Vec<Pos2>>>()
                {
                    paint::outlined_line(painter, &quad, paint::STATS, false, true);
                    paint::outlined_line(painter, &quad[0..2], theme::BLAZE, false, false);
                    let (width, height) = if self.scale.mode == scale::ScaleMode::GridSquares {
                        (
                            trf!(
                                "{} squares · {} mm",
                                self.scale.squares_across,
                                tenths(width_mm)
                            ),
                            trf!(
                                "{} squares · {} mm",
                                self.scale.squares_down,
                                tenths(height_mm)
                            ),
                        )
                    } else {
                        (
                            trf!("width {} mm", tenths(width_mm)),
                            trf!("height {} mm", tenths(height_mm)),
                        )
                    };
                    paint::label_pill(
                        painter,
                        quad[0] + 0.5 * (quad[1] - quad[0]) + vec2(0.0, 10.0),
                        &width,
                        egui::Align2::CENTER_TOP,
                    );
                    paint::label_pill(
                        painter,
                        quad[0] + 0.5 * (quad[3] - quad[0]) + vec2(10.0, 0.0),
                        &height,
                        egui::Align2::LEFT_CENTER,
                    );
                }
            }
        }
        for (index, mark) in self.scale.handles() {
            if let Some(screen) = at(mark) {
                let label = match self.scale.mode {
                    scale::ScaleMode::TwoPoints => ["A", "B"][index.min(1)].to_owned(),
                    scale::ScaleMode::GridSquares | scale::ScaleMode::FourCorners => {
                        (index + 1).to_string()
                    }
                };
                paint::handle(
                    painter,
                    screen,
                    &label,
                    self.selected == Some(Handle::Scale(index)),
                );
            }
        }
    }

    /// Zooms the stored view about its middle; the canvas applies it.
    fn zoom_view(&mut self, to: ZoomTo) {
        match (to, &mut self.view) {
            (ZoomTo::In, Some(view)) => view.zoom *= 1.5,
            (ZoomTo::Out, Some(view)) => view.zoom /= 1.5,
            _ => self.fit = true,
        }
    }

    /// The keys on a desktop: undo, redo, delete, nudge, flags and zoom.
    fn keys(&self, ui: &mut egui::Ui, commands: &mut Vec<Command>) {
        if ui.ctx().text_edit_focused() || self.discard_asked.is_some() {
            return;
        }
        use egui::{Key, Modifiers};
        let selected = self.selected.is_some();
        let shot = matches!(self.selected, Some(Handle::Shot(_)));
        ui.input_mut(|input| {
            if input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || input.consume_key(Modifiers::COMMAND, Key::Y)
            {
                commands.push(Command::Redo);
            } else if input.consume_key(Modifiers::COMMAND, Key::Z) {
                commands.push(Command::Undo);
            }
            if selected
                && (input.consume_key(Modifiers::NONE, Key::Backspace)
                    || input.consume_key(Modifiers::NONE, Key::Delete))
            {
                commands.push(Command::Delete);
            }
            if selected && input.consume_key(Modifiers::NONE, Key::Escape) {
                commands.push(Command::Deselect);
            }
            if shot && input.consume_key(Modifiers::NONE, Key::C) {
                commands.push(Command::ColdBore);
            }
            if shot && input.consume_key(Modifiers::NONE, Key::F) {
                commands.push(Command::Flier);
            }
            if selected {
                for (key, direction) in [
                    (Key::ArrowLeft, vec2(-1.0, 0.0)),
                    (Key::ArrowRight, vec2(1.0, 0.0)),
                    (Key::ArrowUp, vec2(0.0, -1.0)),
                    (Key::ArrowDown, vec2(0.0, 1.0)),
                ] {
                    for _ in 0..input.count_and_consume_key(Modifiers::SHIFT, key) {
                        commands.push(Command::Nudge(direction, true));
                    }
                    for _ in 0..input.count_and_consume_key(Modifiers::NONE, key) {
                        commands.push(Command::Nudge(direction, false));
                    }
                }
            }
            if input.consume_key(Modifiers::NONE, Key::Plus)
                || input.consume_key(Modifiers::NONE, Key::Equals)
            {
                commands.push(Command::Zoom(ZoomTo::In));
            }
            if input.consume_key(Modifiers::NONE, Key::Minus) {
                commands.push(Command::Zoom(ZoomTo::Out));
            }
            if input.consume_key(Modifiers::NONE, Key::Num0) {
                commands.push(Command::Zoom(ZoomTo::Fit));
            }
        });
    }

    /// The line telling what to do now.
    fn instruction(&self, narrow: bool) -> String {
        match self.step {
            Step::Scale => scale::instruction(&self.scale),
            Step::Aim => match self.aim() {
                None => tr("Tap the point you aimed at.").to_owned(),
                Some(_) => tr("Drag the crosshair to move it, or go on to the shots.").to_owned(),
            },
            Step::Shots => {
                let next = self.shot_count() + 1;
                if self.press.is_some_and(|press| press.gesture.is_placing()) {
                    return trf!("Lift to place shot {}.", next);
                }
                match self.selected {
                    Some(Handle::Shot(index)) if narrow => {
                        trf!("Shot {} selected · drag it to move it", index + 1)
                    }
                    Some(Handle::Shot(index)) => trf!(
                        "Shot {} selected · arrows nudge it · Delete removes it · C cold bore · F flier · \
                         Esc deselects",
                        index + 1
                    ),
                    _ => trf!("Tap to place shot {} · hold a shot to select it", next),
                }
            }
        }
    }
}
