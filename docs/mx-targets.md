# MX targets

An MX target is a sheet MXSC prints (Cards → MX target) for shooting groups.
A mark near each corner lets the app find the sheet on a photo by itself:
the marks give the scale, straighten a photo taken at an angle, and tell
where every aim point is. Each mark also carries a small code with the
sheet's layout, so the photo alone is enough: nothing has to be chosen in
the app first.

The format is open, like the backup file: anyone may print sheets that the
app reads, or read MXSC's sheets with other software. This page is the
whole format, version 1. The code that writes it is
`crates/app/src/cards/mx_target.rs`; the code that reads it is
`crates/groups-vision/src/marks.rs` (plain Rust, no image library).

## The sheet

All sizes are millimetres on the sheet as printed, x to the right from the
sheet's left edge and y down from its top edge.

| Paper  | Portrait (w × h) | Module `u` | Mark (9 `u`) | Mark centre inset `i` | Mark rectangle (portrait) |
|--------|------------------|-----------:|-------------:|----------------------:|---------------------------|
| A4     | 210 × 297        | 2.5        | 22.5         | 24                    | 162 × 249                 |
| Letter | 215.9 × 279.4    | 2.5        | 22.5         | 24                    | 167.9 × 231.4             |
| A3     | 297 × 420        | 3.5        | 31.5         | 30                    | 237 × 360                 |

In landscape the sheet's width and height swap; the inset, the module and
the marks stay the same.

- **Corner marks.** The marks' centres are at `(i, i)`, `(W − i, i)`,
  `(W − i, H − i)` and `(i, H − i)`: top-left, top-right, bottom-right and
  bottom-left. The rectangle through them is `(W − 2i) × (H − 2i)`. Each
  mark's outer edge is at least 12.75 mm (A3: 14.25 mm) from the paper's
  edge, clear of the margin printers can't reach, and has paper round it
  on every side.
- **Field.** The aim points and the grid sit in the field from
  `x0 = i − 4.5u` to `W − x0` across and from `y0 = i + 6u` to `H − y0`
  down: between the rows of marks, a module and a half clear of them.
- **Aim points.** 1, 2, 4 or 6, in a block of columns × rows:

  | Aim points | Portrait | Landscape |
  |-----------:|----------|-----------|
  | 1          | 1 × 1    | 1 × 1     |
  | 2          | 1 × 2    | 2 × 1     |
  | 4          | 2 × 2    | 2 × 2     |
  | 6          | 2 × 3    | 3 × 2     |

  Each aim point starts at the middle of its share of the field and is
  then moved to the nearest grid crossing, counted from the sheet's middle
  `(W/2, H/2)`: every 10 mm or 25.4 mm with a grid, every 5 mm without one.
  They are numbered row by row from the top left, and the number is
  printed small above each.
- **Aim marks.** An outline with a fine cross (0.25 mm lines) through the
  centre: a **diamond** (a square on its corner, the cross joining its
  points) or an upright **square**. The size is the width across (tip to
  tip for a diamond): 25 mm (for about 100 m), 40 mm (150 m), 60 mm (200 m)
  or 80 mm (300 m); at those ranges each is 0.25–0.3 mil wide in the scope. The
  outline is a tenth of the width thick, measured across the corners. A
  size that doesn't fit is printed smaller, in whole 5 mm, so that
  neighbouring marks keep 10 mm apart and 3 mm from the field's edge (the
  smallest is 10 mm); the code still carries the size chosen, and the
  reader works out the same printed size.
- **Grid.** None, 10 mm or 1 inch squares, light blue (0.15 mm lines), over
  the field, with lines through the sheet's middle. It is light enough to
  print grey on a black-and-white printer and not to be taken for ink.
- **Text.** "MX target" and the layout in words between the top marks; the
  print advice, a 100 mm ruler with millimetre ticks, and the format and
  layout code in small print between the bottom marks.

## The corner marks

A mark is 9 × 9 square modules:

```
  0 1 2 3 4 5 6 7 8
0 # # # # # # # # #     #  black frame, 1 module
1 # . . . . . . . #     .  white ring, 1 module
2 # . D d d d w . #     D  orientation: always black
3 # . d d d d d . #     w  orientation: always white
4 # . d d d d d . #     d  data: 21 bits
5 # . d d d d d . #
6 # . w d d d w . #
7 # . . . . . . . #
8 # # # # # # # # #
```

Every mark is printed upright with the sheet, so its black orientation
module is towards the sheet's top-left: whichever way a photo is turned,
each mark shows which way the sheet's top is.

The 21 data modules, read row by row from the top left of the 5 × 5 data
area and skipping its four corners, are the bits of a **word**, most
significant first; black is 1.

| Bits (of 21) | Meaning |
|--------------|---------|
| 20–19 | Corner: 0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left |
| 18    | Version: 0 for format 1 (a reader refuses 1) |
| 17–16 | Paper: 0 A4, 1 A3, 2 Letter (3 is refused) |
| 15    | 1 for landscape |
| 14–13 | Aim points: 0 one, 1 two, 2 four, 3 six |
| 12–11 | Grid: 0 none, 1 10 mm, 2 1 inch (3 is refused) |
| 10    | Aim mark: 0 diamond, 1 square |
| 9–8   | Aim mark size: 0 25 mm, 1 40 mm, 2 60 mm, 3 80 mm |
| 7–0   | CRC-8 of bits 20–8 |

Bits 18–8 are the sheet's **layout code** (0–1023, printed at the foot of
the sheet). The CRC is CRC-8 with the polynomial x⁸ + x² + x + 1 (0x07),
starting from zero, over the 13 bits 20–8 fed most significant first, with
no final XOR. Over these 21-bit words it catches every error of up to three
modules.

All four marks carry the same layout and differ in the corner, so any mark
read tells the whole layout, and the corner it is at.

## Reading a photo

`groups_vision::marks::find_marks(image, region)`:

1. **Work copy.** The region is reduced by whole-pixel averaging to about
   1200 px on its long side. A pixel is ink when it is darker than three
   quarters of the mean round it (a window of 1/15 of the long side), which
   takes out shadows and uneven light.
2. **Candidates.** Connected shapes of ink (8-connected, by runs) whose
   outline makes a convex quadrilateral covering nearly all of their hull,
   with a hole (ink 20–62% of the quadrilateral, the frame being 40%), no
   corner sharper than about 37° and sides no more than 3 : 1.
3. **Reading.** The quadrilateral's homography places the 81 modules; each
   is sampled at nine points. The frame's median gives the ink level and
   the ring's the paper level; a candidate is a mark when nearly all the
   frame modules are ink and the ring paper. Exactly one data corner must
   be black: it gives the turn. The word must pass its CRC and name a
   layout of this version.
4. **Measuring.** On the photo's own pixels, each of the frame's eight
   edges (outer and inner) is found along up to 32 profiles across it as
   the centroid of the rise from ink to paper, to a fraction of a pixel;
   a straight line is fitted to each, leaving out points far from it (a
   bullet hole across the frame), and the mark's homography is fitted to
   the eight corners where the lines meet. This is done twice. The mark's
   **centre** is where its homography puts the middle of its square, which
   is exact in perspective.
5. **The sheet.** A homography from sheet mm to photo px is fitted to the
   frame corners of every mark measured (up to 32 points). A mark whose
   code couldn't be read (a hole through its data) is looked for where the
   others put it and measured the same way (**Found**). A mark not found at
   all is placed where the sheet's homography puts it (**Estimated**). A
   mark that doesn't lie on the sheet the others make is left out, while
   three remain.
6. **Finer look.** If not all four marks were read, the search runs again
   on a work copy twice as fine (up to 2400 px), for small or blurred marks,
   and the better result is kept.

The result (`Marks`) has the layout, the four mark centres in photo px
(sheet top-left, top-right, bottom-right, bottom-left, whichever way the
photo is turned), how each was placed, the photo's quarter turns, the fit's
RMS in px, its **flatness** (that RMS in modules of the marks as seen:
below about 0.05 for a flat sheet, worth a warning above 0.12), and the aim
points in sheet mm and in photo px.
`Marks::calibration()` makes a calibration with
`Calibration::sheet_corners` from the mark centres in the sheet's own order
and the mark rectangle's size: its millimetres follow the sheet, with the
origin at the bottom-left mark and y up the sheet, whichever way the photo
is turned, so "up" and "right" on the target stay the sheet's. The aim
points are mapped through it. Stored with a photo, such a calibration is a
`four_corner` one with `ordered: true` (see
[export-format.md](export-format.md)).

Errors say why: the region is too small, no marks, marks whose codes can't
be read, only some marks (and which), or marks that don't lie on one flat
sheet. With a mark placed from the other three, the photo is refused when
the flatness is above 0.12: three marks on a curled sheet still fit some
plane well, and the fourth placed from it could be millimetres out.

**Accuracy.** On synthetic photos (the card itself drawn and photographed
through a camera tilted up to 35°, turned any way, with uneven light and a
shadow, blur, noise and JPEG at quality 80) the mark centres come within
about 0.2 px of the truth and the aim points within 0.06 mm. A covered mark is
placed within about 0.2 px from the other three on a flat sheet; on real
paper, curl and the lens's distortion make that larger. A 4000 × 3000 photo
takes about 7 ms on a laptop (optimised build); one that needs the finer
look (a mark covered) about 20 ms.

## Printing and photographing

- **Print at 100% (no scaling)**, "actual size", never "fit to page" (the
  PDF asks the print dialog for no scaling and for paper of its own size,
  but not every viewer listens), and check the ruler at the foot with a ruler of your own: it should measure
  100 mm. The marks can only tell the size they were printed at; a sheet
  printed at 97% gives every group 3% small.
- Laser or inkjet, black ink: the corner marks and the ruler are pure black
  (the black ink alone), so a printer doesn't mix them from colours. Matte paper photographs better than glossy.
- Keep the corner marks flat, clean and uncovered; a hole through one is
  fine, and one mark covered or torn off is made up from the other three
  (less exactly).
- Photograph the whole sheet with all four marks in view, as square-on as
  you can (up to about 35° is read), in even light without glare on the
  marks. The sheet should fill most of the photo: each mark should be at
  least about 60 px across on a 4000 px photo.

## Limits

- The marks fix the sheet's plane. A sheet that curls or sags bends that
  plane, and points far from the marks are then a little out; pin the
  sheet flat on a board.
- Lens distortion is not corrected. Phone cameras correct their own; a wide
  lens near the photo's edge can be a few px out.
- A mirrored photo (a selfie camera's) is not read.
- Two MX targets in one photo: the layout seen most is used; marks of a
  second sheet with the same layout may make the result inconsistent.
- The printer's own scaling error is not seen (see the ruler).
