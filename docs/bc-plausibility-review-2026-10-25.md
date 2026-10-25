# Catalog BC plausibility review

Review date: 2026-10-25. Baseline commit: `1d077ef0a652faf7298bb067f091700781b8c9fe`.
Decision: record findings only; leave catalog, defaults and solver unchanged.

Subsequent action: the user authorized BC01's correction on 2026-10-25.
Fiocchi #709959 now has G7 0.243 and a corrected source note. The findings below
and CSV preserve the original audit snapshot, including original values and
file hashes; they should not be read as the current catalog state after this fix.
The user also subsequently authorized BC03 corrections: PPU A-689 G1 0.535
and A-718 G1 0.400. These are applied, with provenance notes.
Further approved actions: BC02 Fiocchi #709891 now uses G1 0.355, with the
disputed G7 0.213 preserved in notes and cleared from the active field. BC09
PPU A-594 now uses component G1 0.485. BC10/BC11 have visible provenance
labels in the catalog and bullet editor; other values were retained as recommended.
For the subsequent PPU investigation, see [the follow-up](ppu-bc-follow-up-2026-10-25.md).

## Scope and meaning

All 26 JSON files in `crates/ballistics-store/catalog/bullets/` were screened:
1,529 records, 1,529 G1 coefficients and 459 G7 coefficients (1,988 values).
Every record has G1; 1,070 have no G7. No missing, zero, negative, nonfinite or
obvious decimal-order BC was found. The accompanying CSV contains every entry,
including entries with no specific concern. An empty finding is not certification.

The review checked numeric validity, mass and diameter context, G1/G7 ratios,
same-product discrepancies, component/ammunition relationships, family weight
trends and source notes. Targeted manufacturer pages and a manufacturer PDF were
checked for the strongest candidates. Every product page was not revisited.
No firing, Doppler measurements or independent experimental verification occurred.
No catalog-wide fit to published velocity tables was performed.

Sectional density (SD) was computed as weight in pounds / diameter in inches squared.
Use catalog diameter where present, otherwise the caliber table's bullet diameter;
use grains where present, otherwise convert grams with 0.06479891 g/gr.
G1/SD gives shape context, not a pass/fail law. Initial broad inspection triggers
were G1/SD outside 0.65–2.70 and G1/G7 outside 1.75–2.30. Additional review covered
G1/SD below 0.80 or above 2.35, product discrepancies and weight-trend reversals.
These are deliberately heuristic inspection thresholds, not measured uncertainty
bounds. Different bullet designs may share a product-line name.

BC depends on reference drag model and velocity interval. There is no universal
G1-to-G7 conversion; “G7 should be half G1” cannot establish an error, especially
at subsonic/transonic velocities. [Hornady explains its velocity-dependent BCs](https://www.hornady.com/BC).
A plausible published number can still be biased or unsuitable outside its quoted
velocity interval. Catalog `verified_on` records source work, not physical testing.
The app selects G7 by default when it is available (`catalog.rs`), making erroneous
G7 data particularly relevant. That behavior was inspected and left unchanged.

## Findings awaiting a decision

| Ref | Priority | Entries / stored BC | Assessment and evidence |
| --- | --- | --- | --- |
| BC01 | High: source mismatch | Fiocchi #709959, .308 175 gr Sierra MatchKing; G1 0.496, G7 0.310 | Current [official page](https://fiocchi.com/en/709959.html) lists G1 0.496 and G7 **0.243 Litz**, unlike the catalog and its note. Stored G7 is about 28% above that source. Clear disagreement with the current source, although this does not establish which figure was originally published or independently measured. |
| BC02 | Medium: model pair | Fiocchi #709891, .308 165 gr HPBT GameKing; 0.355 / 0.213 | Ratio 1.667 is unusually small for a supersonic boat-tail pair. The [official page](https://fiocchi.com/en/709891.html) does publish both numbers, so this is not an established transcription error. Catalog Sierra 165 gr HPBT GameKing has 0.363 / 0.183; Fiocchi's G7 is about 16% higher. Exact projectile version and velocity averaging need reconciliation. |
| BC03 | High: same bullet conflict | PPU A-689 Creedmoor 140 gr G1 0.400; A-718 Creedmoor 120 gr G1 0.519 | The [PPU 2023 catalog](https://www.prvipartizan.com/download/PPU_2023.pdf), printed pp. 40–41 (PDF page index 20), identifies B-689 and B-540 respectively. Its component table gives B-689 **0.535** and B-540 **0.400**. Loaded B-689 is about 25% below its component; loaded B-540 about 30% above. The same spread also gives A-540/B-540 0.400 and A-721/B-689 0.519. Strong internal publication conflict, suggestive of a table error; do not silently swap numbers. |
| BC04 | Medium: model pair | Sierra 6mm 90 gr TGK #4100T; 0.490 / 0.211 | Ratio 2.322; unusually large for a streamlined supersonic bullet. G1 is also high relative to SD (about 2.25), though not impossible. The catalog attributes both numbers to official Specs. The [current page](https://sierrabullets.com/6mm-90-gr-tipped-gameking-tgk/) identifies the SKU, but its Specs values were not exposed in this browser extraction. Independent confirmation of the pair remains open; do not claim the current page verified these numbers. |
| BC05 | Medium: unusually low, published | Lapua N563 6.5mm 140 gr Naturalis component and both 6.5 Creedmoor / 6.5×55 loads; G1 0.201 | G1/SD about 0.70, conspicuously low beside other pointed 6.5mm 140 gr designs. Nevertheless the [current official component page](https://www.lapua.com/product/65-mm-cal-naturalis-hunting-bullet-n563-91-g-140-gr/) explicitly says 0.201, as does the [2020 official catalog](https://www.lapua.com/wp-content/uploads/2020/01/lapua-catalogue-2020_letter_ENG-USA_www.pdf). Existing notes mention a retailer alternative of 0.329 and a supporting velocity table; neither was independently established here. Keep as a published outlier, not a proven mistake. |
| BC06 | Low: component/load conflict | Barnes .308 150 gr TTSX; ammunition G1 0.440, component 0.420 | About 4.8% difference. Already documented in catalog notes. The [official 2022 catalog](https://www.barnesbullets.com/wp-content/uploads/2022/03/2022-Barnes-Catalog-FINAL.pdf) independently supports 0.440 for load #21540. Component #30366's 0.420 comes from the existing catalog evidence; its current variant Specs were not exposed in this browser extraction. Both values are plausible; variant or measurement differences remain possible. |
| BC07 | Medium: packaging discrepancy | Federal AE65CRD2B / AE65CRD2 6.5 Creedmoor 120 gr OTM; G1 0.391 / 0.421 | Catalog notes identify 50-round and 20-round packs, both at 2900 fps. A 7.7% BC difference is unexpected if packaging is the only difference. Check projectile identity and source revision before unifying. This is an internal-catalog lead; these two pages were not freshly verified in this review. |
| BC08 | Medium: product discrepancy | Federal Fusion .280 Rem / 7mm-08, 140 gr; G1 0.485 / 0.390 | About 24% difference for same nominal line, diameter and weight. Both current official pages support the catalog: [.280 Rem](https://www.federalpremium.com/rifle/fusion/11-F280FS1.html), [7mm-08](https://www.federalpremium.com/rifle/fusion/11-F708FS1.html). Both list 1.135 in bullet length. Individual values are plausible, but the size of the difference merits checking projectile versions and measurement basis. Same length does not prove identical geometry. |
| BC09 | Low: smaller same-bullet conflict | PPU A-594 .308 Solid 156 gr G1 0.450; B-594 component 0.485 | About 7.2% below component. Both appear in the [same PPU table spread](https://www.prvipartizan.com/download/PPU_2023.pdf). Far less striking than BC03; averaging or projectile details may explain it. |
| BC10 | Provenance concern | Aguila 19 records | Catalog notes explicitly describe **16 fitted BCs**, **2 user-supplied BCs**, and **1 borrowed BC**. Fitted values are plausible effective coefficients but not published manufacturer BC measurements. Particularly check the 29 gr .22 Short's fitted 0.172 before extrapolating beyond its fitted interval; its size alone does not prove an error in subsonic flight. User-supplied Target Competition 0.173 and Subsonic HP 0.111 remain user-supplied evidence. The .17 HMR 20 gr value 0.125 is borrowed from Hornady XTP without Aguila identifying an XTP projectile. These provenance classifications come from existing notes, not new source verification. |
| BC11 | Provenance concern | Three Hornady .17 HMR entries and Winchester S17HMR1LF | G1 0.115–0.125 is plausible for the size and shape, but catalog notes attribute the BCs to third-party sources rather than a located manufacturer BC statement. Retain that distinction; agreement between retailers need not mean independent measurement. No new retailer verification was performed here. |

Exact affected IDs and cross-references are in the CSV. BC03 includes A-540,
A-721 and the two component records as comparison context, not additional proven
errors. BC02 includes its Sierra component comparison in the ledger too.

## Outliers with a plausible explanation

| Ref | Entries | Why they should not be called errors solely from this screen |
| --- | --- | --- |
| CTX01 | S&B .45-70 405 gr SP 0.306 / 0.199 | Ratio 1.538 is on the [official SP table](https://www.sellierbellot.us/products/rifle-ammunition/rifle-ammunition-sp/). Muzzle speed about 460 m/s takes it through the transonic region. The supersonic boat-tail ratio heuristic is weak evidence here. Actual BC suitability across that interval remains unmeasured. |
| CTX02 | S&B .308 200 gr subsonic FMJ 0.558 / 0.242 | Ratio 2.306 appears in the [official FMJ table](https://www.sellierbellot.us/products/rifle-ammunition/rifle-ammunition-fmj/) for bullet 2950. About 314 m/s is subsonic under standard conditions; a ratio near 2 is not required. |
| CTX03 | Sierra .30 125 gr HP FN 0.119; .35 200 gr RN 0.148; .45-70 300 / 400 gr HP FN 0.120 / 0.152 | Low G1/SD about 0.56–0.66 is consistent with blunt/flat/hollow-point geometry. These are contextually plausible low BCs, not decimal errors established by this audit. Values rely on existing catalog sourcing. |
| CTX04 | Berger .375 379 / 407 gr ELR solids G1 0.951 / 1.022 | G1/SD about 2.47 is high but credible for very long streamlined solids. A BC greater than 1 is not inherently impossible. Both G7 values and size context are internally coherent; no fresh source verification of these two pages was performed. |
| CTX05 | Berger 230 gr Hybrid OTM Tactical component 0.710 / 0.364 and discontinued .300 Win Mag load 0.717 / 0.368 | Roughly 1% difference in both models, with differing published form factors in existing notes. Plausible source/version differences, not enough evidence of a defect. |

Other family weight reversals were inspected: Federal Fusion .30-30 versus pointed
.308 bullets, Norma Whitetail 156 versus 140 gr 6.5mm, Federal Power-Shok .270
150 versus 130 gr, and Federal .458 Bear Claw 500 versus 400 gr. Different nose
shapes or projectile versions can outweigh additional mass; no error was established.
Small Hornady ELD-X component/load differences and several-percent manufacturer
differences likewise do not justify a correction on their own.

## Follow-up boundaries

The outstanding task is a maintainer decision on the unresolved findings above.
BC01, BC02, the two BC03 Creedmoor corrections and BC09 were subsequently
authorized and corrected as noted at the top. BC10/BC11 received provenance
labels. Other values were retained; unresolved source conflicts remain recorded. A source mismatch can be resolved through
source provenance; physical accuracy would still require independent measurement.
The ledger preserves original source URLs and `verified_on` dates separately from
this review date. It also preserves source-file SHA-256 values so a later review
can tell whether the catalog changed.
