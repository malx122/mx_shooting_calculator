# Project memory

## BC plausibility review — 2026-10-25

The user requested a plausibility review of every catalog BC and asked that
suspicions be recorded for a later decision. **Do not change BCs on the strength
of this review without that decision.** The initial review changed no values.
The user subsequently authorized the Fiocchi corrections, three PPU
corrections and the provenance labels described below.

Reviewed the 26 bullet catalog files: 1,529 entries, 1,529 G1 values and 459 G7
values. This was a catalog-wide numerical/context screen with targeted primary-source
checks, not experimental validation or a fresh source verification of every row.

See [the review](docs/bc-plausibility-review-2026-10-25.md) for evidence and
limitations, and [the complete entry ledger](docs/bc-plausibility-review-2026-10-25.csv)
for exact IDs, original values, screening results and finding references.

Resolved on user instruction: Fiocchi #709959 G7 changed from 0.310 to 0.243,
matching the rechecked official page; its note was corrected too. The CSV remains
the original audit snapshot, including its original file hashes.

Retained for future clarification: Sierra 90 gr 6mm TGK
0.490/0.211 needs its two BC definitions reconciled; Lapua N563 G1 0.201 is
unusually low but confirmed as published. Also retain the smaller Barnes TTSX,
Federal American Eagle and Federal Fusion cross-product discrepancies in the
review. Aguila includes fitted, user-supplied and borrowed BCs, which must not be
treated as manufacturer measurements. A catalog `verified_on` date is not a
live-fire verification.

### PPU follow-up — 2026-10-25

See [the closer examination](docs/ppu-bc-follow-up-2026-10-25.md).
Standard-atmosphere G1 fits to PPU's own 100/200/300 m velocity tables imply
approximately 0.529 for A-689 and 0.406 for A-718, supporting the identified
component values 0.535 and 0.400 respectively. The user approved these values:
A-689 was changed from 0.400 to 0.535 and A-718 from 0.519 to 0.400, with
source notes explaining the inference from publication consistency, not physical
validation. A-721 remains at 0.519. A-594 was subsequently approved for
0.485 and changed with a source note explaining the moderate-confidence inference.

### Recommendations for unresolved findings — 2026-10-25

The user approved these recommendations. Fiocchi #709891 now uses G1 0.355
with G7 cleared and the disputed 0.213 preserved in its note. PPU A-594 now
has G1 0.485. Other numerical values below were retained as recommended.
The following list preserves the reasoning behind the decision:

- Fiocchi #709891: use G1 0.355. With the current catalog schema,
  `bc_g7` is null and the disputed published G7 0.213 remains in the source
  note, so it is not automatically selected. Do not replace it with an inferred
  G7. Rechecked https://fiocchi.com/en/709891.html: velocities at 100–500 m
  are 745/665/590/525/460 m/s from 830 m/s. Using the core drag tables, density
  1.225 kg/m³, sound speed 340.294 m/s and 1D RK4 at 0.25 m steps, G1 0.355
  gives 745.6/666.2/591.8/522.8/460.4 (RMS discrepancy 1.4 m/s); G7 0.213
  gives 758.9/691.3/627.4/566.8/509.1 (35.9 m/s). This establishes publication
  consistency, not measured accuracy. The G7 source attribution to Litz does
  not establish that it refers to this exact projectile.
- PPU A-594: use G1 0.485 from its identified B-594 component, with
  moderate confidence; the earlier velocity-table fit of 0.493 supports it.
  Keep A-721 at 0.519: both 0.519 and component 0.535 are close to fitted 0.527.
- Sierra 6mm 90 gr TGK: retain 0.490/0.211 pending confirmation of each model's
  velocity interval or a manufacturer correction. Ratio alone cannot select
  a replacement.
- Lapua N563: keep G1 0.201, which the official page still explicitly publishes.
- Barnes 150 gr TTSX: retain ammunition 0.440 and component 0.420 separately.
- Federal American Eagle: retain 0.391 for AE65CRD2B and 0.421 for AE65CRD2.
  Both official pages were freshly checked: same listed bullet length 1.170 in,
  but different BCs AND velocity tables. Their tables cannot independently
  resolve which BC represents the physical bullet. Keep the SKU distinction.
- Federal Fusion .280 Rem / 7mm-08: retain 0.485/0.390 by SKU; matching nominal
  weight and length do not establish identical bullets or BC measurement basis.
- Aguila and third-party-sourced rimfire: retain plausible values; their
  fitted/user-supplied/borrowed/retailer provenance is now visible in the catalog
  and the catalog source section of the bullet editor.
  Aguila .17 HMR 20 gr 0.125 especially remains a provisional cross-brand proxy.
- Contextual S&B, blunt Sierra and high-BC Berger outliers: no numerical change
  recommended from the plausibility screen.

Implementation: optional catalog-only `bc_source` metadata labels 23 identified
rimfire entries (16 estimates, 2 user-supplied, 1 provisional borrowed, 4 retailer).
Unclassified entries are not automatically called manufacturer-verified. Labels
are translated into Swedish and Finnish; source notes are expandable in the
bullet editor. Existing personal bullet copies retain their own BCs; newly
added catalog copies use the corrections. The original audit CSV is unchanged.
