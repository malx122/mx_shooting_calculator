# Shotshell dimensions for drawing: sources, legend and gaps

Researched 2026-11-21. Data: `shotshell_dimensions.json`, next to this file. It has one entry for
each of the 20 calibers in `shotgun-calibers.json`.

Every C.I.P. number was read from the sheet PDFs, and every SAAMI number from the drawing pages,
rendered and read one by one. Only numbers are taken. The C.I.P. sheets say "Reproduction
forbidden", and the SAAMI pages say "No part of this publication may be reproduced", so no drawing
is copied.

## Result in brief

- **All 20 calibers have full C.I.P. cartridge numbers**: g, t, d, h and l, each with its tolerance.
- **11 calibers also have a SAAMI drawing.** It adds the rim bevel, the fillet radius and
  maximum loaded lengths for folded and rolled crimps. These are 10/89, 12/70, 12/76, 12/89, 16/70,
  20/70, 20/76, 28/70, 28/76, .410/63.5 (SAAMI 2½") and .410/76.
- **9 calibers have no loaded length in any standard**: 12/65, 12/67, 16/65, 16/67, 20/65, 24/65,
  28/65, 32/65 and .410/65. For them `length_loaded_mm` is null, and `length_loaded_estimate`
  holds an inferred value.
- **No standard gives the head (brass) height.** SAAMI writes "CAP HEIGHT OPTIONAL", and C.I.P.
  draws the cap line without a dimension. The heights in the data come from makers' hulls, or are
  inferred.

## Legend: the C.I.P. Table VII shotshell sheet

The sheet has two halves: "CARTRIDGE MAXIMALE" (lower-case letters) and "CHAMBER MINIMALE"
(capitals). The cartridge values are maxima with minus tolerances; the chamber values are minima
with plus tolerances. Dimensions are in mm.

| Letter | Meaning (cartridge) | Chamber counterpart |
|---|---|---|
| g | Rim (head) diameter | G, rim recess diameter |
| t ¹⁾ | Rim thickness | T ¹⁾, rim recess depth |
| d ¹⁾ | Body diameter just ahead of the rim; its leader points at the rim-body corner | D ¹⁾ |
| h | Body diameter at the mouth (the top of the case) | H ¹⁾, chamber diameter at the chamber end, where the cone starts |
| l | Case length for each marking (e.g. 12/70), opened, i.e. uncrimped | L ¹⁾, minimum chamber length |
| — | | B ¹⁾, bore diameter (a minimum, plus tolerance) |
| — | | α1 ¹⁾, forcing-cone angle, 10°30′ max |
| — | | α, 0° ≤ α ≤ 35°, with r = 0.5: the chamfer and radius at the mouth of the rim recess |
| — | | Fe, headspace: 0.10 (proof barrel), 0.20 (break-action), 0.35 (automatic) |
| — | | Pmax, PK, PE (bar), and M, the transducer position (25/30 or 17, read as mm from the breech face, an inference) |

¹⁾ is the sheet's footnote: "Check for safety reasons".

How the case length is meant: C.I.P. gives the case length l just under the chamber length L (69.80
against 69.90 for 12/70). SAAMI labels the same length "UNCRIMPED". RWS writes that the nominal
"total case length – not the length of the ready-loaded, crimped cartridge – is 65, 67.5, 70 or 76 mm".

### SAAMI cartridge drawing (Z299.2), for comparison
- The rim diameter, the body diameter ahead of the rim (dimensioned to the intersection of lines, ✱),
  the mouth diameter, and the rim thickness, each with a minus tolerance.
- The **55° B** rim bevel. It appears to be measured from the case axis, which is 35° from the rim
  face. That matches the 0–35° C.I.P. chamber chamfer, but this is my reading of the drawing.
- **.015 (0.38) R MAX**, the radius at the rim-body corner.
- **B**, a basic diameter on the bevel (e.g. 12 ga .8500 = 21.590 mm). The rim thickness is
  dimensioned to it, so the SAAMI rim thickness (12 ga 1.463 −0.356) is smaller than C.I.P. t
  (1.85 −0.30). This is a reading of the drawing; SAAMI does not say it in words.
- A reference dimension Δ, from the rim to the body.
- "CAP HEIGHT OPTIONAL".
- The length uncrimped (−2.54), folded and rolled. Crimped lengths use the general tolerance,
  "LENGTH TOL −.250 (6.35)".

## Numbers read (C.I.P. cartridge maxima, mm)

| Gauge | g | t | d | h | Sheet revision |
|---|---|---|---|---|---|
| 10 | 23.65 −0.64 | 1.90 −0.40 | 21.70 −0.24 | 21.30 −0.47 | 2006-09-19 |
| 12 | 22.45 −0.48 | 1.85 −0.30 | 20.60 −0.28 | 20.20 −0.47 | 2018-10-17 |
| 16 | 20.65 −0.33 | 1.65 −0.35 | 18.90 −0.23 | 18.55 −0.49 | 2010-05-26 |
| 20 | 19.40 −0.40 | 1.55 −0.35 | 17.70 −0.22 | 17.35 −0.49 | 2020-11-10 |
| 24 | 18.45 −0.40 | 1.55 −0.35 | 16.75 −0.20 | 16.45 −0.50 | 2006-09-19 |
| 28 | 17.40 −0.38 | 1.55 −0.35 | 15.85 −0.18 | 15.55 −0.49 | 2017-05-17 |
| 32 | 16.10 −0.40 | 1.55 −0.35 | 14.55 −0.15 | 14.25 −0.35 | 2006-09-19 |
| .410 | 13.60 −0.40 | 1.55 −0.35 | 12.00 −0.11 | 11.75 −0.38 | 2016-05-18 |

Case l, each −2.50: 65.00 (x/65); 67.50 (x/67); 69.80 (x/70); 76.00 (x/76); 88.70 (x/89);
63.50 (.410/63.5).

## Loaded (crimped) length against fired length

SAAMI maxima, in mm, before the −6.35 tolerance:

| Drawing | Uncrimped | Folded | Rolled | Folded shortening |
|---|---|---|---|---|
| 10 ga 3½" | 89.15 | 78.99 | 83.57 | 10.16 |
| 12 ga 2¾" | 70.10 | 61.09 | 62.23 | 9.01 |
| 12 ga 3" | 76.45 | 67.44 | 70.10 | 9.01 |
| 12 ga 3½" | 89.15 | 80.14 (folded or rolled) | — | 9.01 |
| 16 ga 2¾" | 70.10 | 61.98 | 62.87 | 8.12 |
| 20 ga 2¾" | 70.10 | 62.10 | 64.77 | 8.00 |
| 20 ga 3" | 76.45 | 68.45 | 71.12 | 8.00 |
| 28 ga 2¾" | 70.10 | 63.50 | 65.02 | 6.60 |
| 28 ga 3" | 76.45 | 69.85 | 71.37 | 6.60 |
| .410 2½" | 64.26 | 59.94 | 60.96 | 4.32 |
| .410 3" | 76.45 | 72.14 | 73.66 | 4.31 |

- **The brief's assumption is too long.** It expected a star-crimped 70 mm shell at about 64 mm
  and a roll-crimped one at about 66 mm. SAAMI's *maximum* for a 12 ga 2¾" is 61.09 mm folded
  and 62.23 mm rolled.
- **RWS's rule of thumb is shorter still.** RWS says roll-crimped loads are "approx. 6 mm
  shorter" and star-crimped loads "approx. 11 mm shorter" than the nominal length, i.e. about 64
  and 59 mm for a 70 mm shell.
- **Folded shortening is constant per gauge**, at about 0.37–0.48 × the mouth diameter. It is the
  same for every length of a gauge.
- **How the estimates were made** (inferred): for lengths without a SAAMI drawing, the C.I.P.
  length l minus that gauge's SAAMI shortening. For 24 and 32 gauge, l − 0.44·h folded and
  l − 6 mm rolled (the RWS figure).

**Crimp folds and depth:**
- **Fold count.** Gun Digest says today's crimp "has either a six- or eight-segment fold". The
  eight-fold is "a little better for the smaller shot sizes", and the six-fold is used "with larger
  shot in hunting and field loads". "The smaller shells in the 28-gauge and the .410-bore use a
  six-fold crimp", and so do "the large shells of a 10-gauge". MEC and Hornady tooling agrees:
  6 and 8 points for 12, 16 and 20; 6 only for 10, 28 and .410. For 24 and 32, 6 folds is inferred.
- **Depth.** Hodgdon: "All shotshell reload data on the Hodgdon website has been created using an
  average crimp depth of 0.055″" (1.40 mm). Gun Digest: "about 1/16-inch" (1.6 mm). This is how
  deep the centre of the folded crimp sits below the mouth edge.

## Head (brass) height

- **No standard defines it.** SAAMI: "CAP HEIGHT OPTIONAL". C.I.P.: no dimension.
- **Maker data.** Cheddite hulls, as listed by Ballistic Products:
  - 12 ga 2¾": 8, 10, 12, 16, 20 and 25 mm (the 25 mm hull is "40% of the hull's length" brass when crimped);
  - 12 ga 2½" (65 mm): 8;
  - 12 ga 3" and 3½": 16;
  - 16 ga: 8, 10 and 16;
  - 20 ga: 8 and 16;
  - 28 ga: 8 and 16;
  - .410: 8 (2" and 2½") and 16 (2½" and 3");
  - 10 ga 3½": 20 (0.725").
- **RWS Rottweil**: Game Edition 12/70, 20/70 and 12/76 have a base cap height ("Höhe der
  Bodenkappe") of 16 mm. Special 36 12/70 has 12 mm.
- **Low and high brass.** So low brass is 8–12 mm and high brass 16–25 mm, which confirms the
  brief. 25 mm hulls exist only in 12 gauge in this data.
- **Gamebore** prints lengths as "8/70", "16/65" or "25/76". The first number is probably the brass
  height (see `docs/shotgun-research/gamebore.md`), but the maker does not say so.
- **In the data.** `head_height_typical_mm`, `_low_mm` and `_high_mm` are per caliber.
  `head_height_basis` says whether each is maker data or inferred. Inferred: 12/67, 16/67, 20/65,
  20/76, 24/65, 28/65, 28/76, 32/65, .410/65, and the bounds for 12/76 and 12/89.

## Primer

- **No standard gives a size.** Neither C.I.P. nor SAAMI Z299.2 gives a 209 primer dimension. The word
  "primer" does not occur in the text that can be extracted from Z299.2, and the drawings show
  none.
- **Outside diameter.** The best figures are informal. Retailers list the Cheddite CX2000 at
  .242 in. A forum measurement of a new Winchester W209 gives .243 in at the base of the battery
  cup, and other brands measure .239–.245 in. Fiocchi 616s are about .001 in larger. So the
  visible battery cup is about **6.15 mm** across.
- **Inner cup.** Federal, quoted by Ballistic Products: "The '209' came from the size of the cup,
  which is about .210" diameter", i.e. the inner primer cup is about **5.3 mm**.
- **The brief's "cup about 6.9 mm" was not found** in any source. The 6.2 mm figure matches the
  outer battery cup, not an inner part. The data uses 6.15 mm outer and 5.3 mm inner, both
  unverified against a standard.

## Hull colour

- **SAAMI Z299.2, "Body Tube Color"** (book page 44): "The color yellow has been reserved for
  20-gauge ammunition. This ammunition shall have a body tube that is primarily yellow. Yellow shall
  not be used for any other gauge/bore shotshell body. No other recommendations are made as to the
  color of service body tubes for other gauges/bores." This is a SAAMI standard, i.e. a voluntary
  US industry standard.
- **C.I.P.** No colour rule was found in Table VII or in the decisions I read.
- **Europe and the UK.** European makers still mostly sell yellow 20 ga hulls (Fiocchi, Cheddite),
  but only by custom. Shooting UK calls the idea that bores were always colour-coded (12 red,
  16 blue, 20 yellow) "one of those shooting myths".
- **In the data.** `hull_colour_rule` is "yellow" for 20 gauge and "none (SAAMI: yellow must not
  be used)" for the rest.

## Gaps and oddities

- **SAAMI 10 ga rim thickness.** It is printed ".0624-.0160 (1.858-0.406)". But .0624 in is
  1.585 mm, and the chamber on the same page gives ".0624 (1.585) MIN". The data uses 1.585.
- **SAAMI 16 ga.** The 16 ga page in Z299.2 is stamped "REVISED – See SAAMI.org for revised
  drawing". The 2025-09-23 revision keeps the cartridge numbers and changes the chamber (bore
  .665 +.020 in).
- **SAAMI 28 ga 3"** is not in the book. It is a public introduction from 2023-06-14, and it is
  used for 28/76.
- **C.I.P. 12/67 and 16/67.** The TDCC online list gives their chamber as 65.1 mm (they are made
  to fire in 65 mm chambers), but the sheets print L = 67.60.
- **C.I.P. 20/70 revision.** The online list gives it as 2021-11-10, while the sheet says
  2020-11-10.
- **Dimensions the cartridge sheets do not give:**
  - the wall thickness;
  - the base-wad height;
  - the primer pocket;
  - the crimp geometry;
  - the cap height.
- **SAAMI has no 24 or 32 gauge** and no 65/67.5 mm lengths except the .410 2½". For those calibers
  the bevel and fillet are assumed from the other gauges.
- **.410/65 against SAAMI .410 2½".** They are close but not the same: C.I.P. l = 65.00 against
  SAAMI uncrimped 64.26. The SAAMI drawing is attached to .410/63.5 only.

## Sources

- C.I.P. TDCC Table VII list:
  http://bobp.cip-bobp.org/en/tdcc_public?page=1&cartridge_type_id=7
- C.I.P. Table VII sheets:
  - Cal. 10: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page3.pdf
  - Cal. 12: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/12-55-en.pdf
  - Cal. 16: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page6.pdf
  - Cal. 20: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabvii-20-en.pdf
  - Cal. 24: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page8.pdf
  - Cal. 28: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabvii-28-eng-2.pdf
  - Cal. 32: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page10.pdf
  - Cal. 410: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/410-en-2.pdf
- ANSI/SAAMI Z299.2 (2019, approved 2025-09-23): the cartridge and chamber drawings on book pages
  20–33 and the body tube colour on page 44:
  https://saami.org/wp-content/uploads/2025/09/ANSI-SAAMI-Z299.2-Shotshell-2019-Approved-2025-09-23.pdf
- SAAMI 16 ga 2¾" revised drawing (2025-09-23):
  https://saami.org/wp-content/uploads/2025/10/Public-Revision-16-gauge-2%C2%BE-2025-10-01.pdf
- SAAMI 28 ga 3" new cartridge (2023-06-14):
  https://saami.org/wp-content/uploads/2023/06/Public-Introduction-28-Ga-3-inch-2023-06-14.pdf
- RWS glossary, case length:
  https://rws-ammunition.com/en/infotainment/glossary/C/case-length
- RWS Rottweil Game Edition (base cap height 16 mm), all4shooters:
  https://www.all4shooters.com/de/jagd/munition/rottweil-game-edition-bleifreie-schrotpatronen-fuer-die-flugwildjagd/
- Cheddite hulls with brass heights, Ballistic Products. For example:
  - https://ballisticproducts.com/Cheddite-12ga-275-25mm-Primed-Hull-100_bag/productinfo/139127025
  - https://www.ballisticproducts.com/Cheddite-12ga-25-65mm-8mm-Primed-Hull-100_bag/productinfo/1391265
  - https://www.ballisticproducts.com/Cheddite-10ga-3-1_2-86mm-hulls-50_bag/productinfo/1391035
  - https://www.ballisticproducts.com/Cheddite-Multi-Hull-28ga-2-3_4-16mm-bag_100/productinfo/1392870/
  - https://www.ballisticproducts.com/Cheddite-410-2-1_2-8mm-brass-primed-and-skived-bag_100/productinfo/1394165/
- Ballistic Products, "The Evolution of Shotshell Primers" (the Federal quote on the .210" cup):
  https://www.ballisticproducts.com/00bprimer.pdf
- Gun Digest, "Shotgun Shell Crimp Styles and Performance":
  https://gundigest.com/more/how-to/shotgun-shell-crimps-styles
- Hodgdon, "Effect of crimp depth on shotshells" (reloading education): https://hodgdonreloading.com/
- Primer diameters, informal:
  - the Accurate Reloading shotshell forum (W209 measured .243 in);
  - retailer listings for the Cheddite CX2000 (.242 in), e.g. https://www.vanceoutdoors.com/products2.cfm/ID/278254/209/cheddite-cx2000-type-209-clerinox-shotshell-primer-1000box
- Shooting UK, "Should cartridge colour denote bore?": https://shootinguk.co.uk/?p=89170
