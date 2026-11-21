# Finding the grid

Most precision targets are printed on grid paper, and a photo rarely shows
the whole sheet. **Find the grid** in the scale step looks for the grid in
the part of the photo on screen (and once, by itself, over the whole of a
new photo), and places four crossings as if they had been tapped: the
largest rectangle of whole squares it can vouch for, with the squares
counted across and down. The square's size can't be seen in pixels, so the
shooter gives it; the app keeps the last one used. The four crossings then
make an ordinary four-corner scale (see
[group-statistics.md](group-statistics.md), section 2).

The code is `crates/groups-vision/src/grid/`; it is plain Rust with no
image-processing library.

## Method

1. **Ink and paper.** Each pixel is the mean of its darkest channel and its
   luma, so green, blue and orange lines show as ink like black ones. The
   region is reduced by whole-pixel averaging to at most 1600 px on its
   long side. The paper's own brightness round each pixel is taken as the
   local maximum (which drops thin lines) smoothed over about 1/24 of the
   region, and each pixel's **darkness** is how far below that it is, as a
   share: shadows, uneven light and paper colour cancel out.
2. **Directions.** A histogram of Sobel gradient directions, weighted by
   strength, has two peaks for a grid, at least 55° apart; rings, holes and
   noise spread evenly and set the floor (the median). Each peak must stand
   clear of the floor, else there is no grid.
3. **Spacing and phase.** For each direction the darkness in the middle
   60% of the region is projected across the lines into a profile. Only
   thin peaks are kept (a point less the higher of its neighbours at
   several distances), so bullet holes and the black centre, which are
   wide, drop out. The profile's autocorrelation gives the spacing: the
   shortest lag with a peak at least half the highest, refined from the
   furthest clear multiple. The phase of the profile's Fourier term at that
   spacing places the line nearest the middle. The squares are square, so
   a family that comes out far finer than the other (thrown by faint fine
   lines or JPEG blocks) is looked at again near the other's spacing.
4. **Following the grid.** The two families make a first model: a
   homography from pixels to grid units `(u, v)`, line `n` being `u = n`.
   It starts in a box five squares either side of the middle (larger while
   something dark, such as the black centre, hides the lines there). The
   spacing and phase are first corrected there by trying scales within
   ±12% and shifts of a tenth of a square, keeping the one that lines up
   the most looks with lines. Each line is then looked for across its
   predicted place (a window of ±0.42 spacing while the box grows, ±0.3
   after): the darkest point standing clear of both sides, placed by the
   centroid of its upper half; a look whose sides are themselves dark is
   inside the black centre or a hole and counts for nothing. The homography is fitted to the points found by least squares on
   `u·w(p) = n`, which is linear in its eight unknowns, weighted so each
   point counts by its miss in pixels, with Huber weights and points over a
   quarter spacing out left out (rings, holes and writing cross the lines
   too). The box grows by 1.4 times a round until it covers the region,
   then two more rounds. Perspective is fitted once the box spans 60% of
   the region (before that it is held near none); lens distortion is not
   modelled.
5. **Levels.** If only every second or third line was found the model is
   coarsened. Then places between the lines are checked for a finer grid
   (every 1/10, 1/8, 1/6, 1/5, 1/4, 1/3, 1/2) by **stacking** the darkness
   across many places at each fraction: faint thin minor lines show in the
   median where single looks miss them. Each line's weight is its stacked
   darkness summed across it. Heavier lines every `k` (2 to 10) count as
   the big squares when the lightest of them (bar one in ten) is at least
   1.3 times the heaviest of the others (bar one in ten), in both families.
6. **Checks.** A model is kept only if most of its lines were found (the
   heavy ones when there are any), the points found sit within 8% of a
   square of where it puts them (RMS), and the two spacings are within
   1.25 times of each other (square squares seen at up to about 35°). A
   finer grid that fails these falls back to the lines as first found;
   otherwise there is no grid.
7. **The rectangle.** Every line is looked for in every cell. A crossing
   counts as seen when it is inside the region and at least three of its
   four arms were found (or both of two, at the region's edge). Candidate
   rectangles have all four corners seen, every line inside them (of the
   counted level) found in at least 60% of its looks there and on average
   under 0.15 of a square off, and their lines found in at least half
   their looks overall. Looks inside something dark don't count.
8. **The count on the pixels.** Taking the candidates largest first (at
   most twelve, none under 15% of the region), every crossing along all
   four edges is refined (below). Each one that can be refined must sit
   within a quarter of a square of where the model puts it, at least
   three in four must refine, and all four corners must. Where one
   homography can't describe the sheet (curled paper), the model slips by
   whole lines somewhere, an edge crossing then lands half a square or
   more off, and the rectangle is shrunk to the part that holds, or
   refused. With big squares the corners are on heavy lines and the small
   squares share them.
9. **Corners.** Each crossing is refined on the pixels the app has for
   the photo: the display texture, which is the photo itself up to 4096 px
   on a computer but at most 2048 px on a phone, where a pixel of the
   texture is about two of the photo. Each of the two lines is found at
   twelve places either side of the crossing (between the other lines'
   crossings), fitted straight by total least squares with points more
   than a pixel off dropped, and the two fits intersected.

Corners are given top-left, top-right, bottom-right, bottom-left as
`Calibration::four_corners` sorts them, with **across** counted along the
top edge.

## In the app

A grid found doesn't set the scale until the shooter confirms the square's
size with one tap: the message names the size and the totals ("Found 20 ×
14 squares of 1 inch = 508 × 356 mm"), the size remembered for that level
is highlighted, and Next waits. The device remembers the big square's
size, the small one's and how many small squares a big one holds;
switching between big and small squares changes the size with the count.
When only one size of square is found but the last grid had two, the app
asks which it is. A rectangle over 600 mm or under 15 mm, or a scale at
which the bullet would be under 4 pixels or over a fifth of the photo, gets
a warning, and a grid found with a confidence under 0.35 gets a caution.
Swap is hidden after a find (the count already runs along the top edge)
unless the rectangle looks turned.

## How well it does

On the project's synthetic photos (`grid/synth.rs`: paper texture, light
falling off by 30%, a soft 35% shadow, line widths varying by 20%, lens
blur, sensor noise, JPEG at quality 65 to 80, scoring rings, a black centre
and bullet holes, turns up to 30°, perspective changing the spacing by up
to about a third) every count was exact and every corner within 0.15 px of
the true crossing, most within 0.05 px. An independent review with its own
generator found the corners 0.28 px off with scoring rings at the grid's
own pitch, and 0.46 px off where the grid stops mid-photo. Curled paper is
now counted right or refused in both test sets; before the edge check, the
review found counts one or two squares short at full confidence.

A 1500 px region takes about 35 to 50 ms on an Apple-silicon laptop
(optimised build); in Chrome a whole photo takes about 50 to 110 ms, two to
three times as long.

## What it can't do

- **The square's size.** Pixels give counts, not millimetres; the shooter
  confirms the size.
- **Curled paper and lens distortion.** The model is one homography. A
  sheet that bends gets a smaller rectangle where the lines still hold, or
  none. Barrel distortion of about 3% (at the photo's corner) and more is
  refused.
- **Steep angles.** Perspective of 27% one way and 8% the other with a 35°
  turn is refused, and the squareness check refuses squares seen at more
  than about 35°.
- **Turns near 45°.** The counts are right at any turn, but which edge
  counts as the top follows `four_corners`' sorting; there is nothing to
  swap.
- **Very fine or very faint lines.** Lines closer than about 3 px at the
  reduced size, or lighter than the paper's noise even when stacked, are
  not found; zoom in and search again. Millimetre paper with heavier 5 mm
  and 10 mm lines is not recognised as three levels and may be refused.
- **Screens and halftone prints.** A photographed screen's pixels or a
  print's dots can beat with the grid; such photos are mostly refused.
- **Big holes or a black centre over most of the view**, rings at the
  grid's own pitch, and dashed or dotted grids: too few crossings hold and
  it says so. It would rather find nothing than a wrong count, so some
  views a person could read are refused.
