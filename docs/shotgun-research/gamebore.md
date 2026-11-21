# Gamebore (Gamebore Cartridge Co., Hull, UK): catalog notes

Checked 2026-11-21. All data comes from the maker. No retailer pages were used: the
maker's pages already gave every value that retailers could have filled in. No prices
are recorded.

## Files

- `gamebore.json`: the current range, 108 entries for 39 products. There are 25 game
  products (24 on the website plus 1 only in the brochure) and 14 clay products (13 on
  the website plus 1 only in the brochure). There is one entry per load, wad and case
  combination. A row printed as "Plastic or Fibre" is split into a plastic entry and a
  fibre entry.
- `gamebore-discontinued.json`: 24 entries for 7 discontinued game products, taken
  from the maker's own pages as archived by the Wayback Machine. The schema is the same,
  and every `notes` field starts with "DISCONTINUED". Keep this out of the main catalog,
  or use it only to recognise boxes people still own.
- `gamebore-downrange.json`: 97 rows of the maker's own velocity-by-distance chart. Each
  row gives m/s at 20, 30, 40, 50, 60, 70 and 80 m for each shot size of a load. The
  site's chart script loads it from `/php/ajax/fetchShotSizeData.php`. These figures
  could be used to fit or check drag. Several loads repeat the same curve, so the figures
  look modelled rather than measured.

## Pages used (all maker's own)

- Range lists: https://www.gamebore.com/cartridges/game (24 products) and
  https://www.gamebore.com/cartridges/clay (13 products). The sitemap
  https://www.gamebore.com/sitemap.xml has no other product pages.
- One product page per product, at `https://www.gamebore.com/cartridge/game/<slug>` and
  `.../cartridge/clay/<slug>`. Each entry's `source_url` gives its page. The data comes
  from each page's "Technical performance" table and its "Shot Sizes" block, which shows
  the size number and the mm.
- Game brochure, PDF dated 2025-02-25: https://www.gamebore.com/files/brochure-game.pdf
  (34 pages). The text is outlined, so I rendered the pages and read the tables.
- Clay brochure, PDF dated 2025-02-25: https://www.gamebore.com/files/brochure.pdf
  (20 pages).
- Discontinued lines: the Wayback Machine's URL index for `gamebore.com/cartridge/*`,
  plus archived copies of the maker's pages. I also checked that every old URL now
  redirects to gamebore.com/404.

## How Gamebore prints its data

- **Velocity**: each table has two columns, "Muzzle Velocity FPS" and "Velocity MPS
  V1".
  - The m/s figure is about 5 % lower than the fps figure converted. For example,
    1475 fps would be 449.6 m/s, but the table prints 426 m/s.
  - Gamebore never defines "V1". The Dark Storm Steel and Mammoth Magnum Steel pages
    carry the footnote "*Steel MPS Velocity calculated at Muzzle and falls within
    MAXIMUM CIP Momentum".
  - `velocity_mps` holds the printed V1 m/s value. `velocity_at` is `"v0"` only for
    those two steel pages and `"not_stated"` everywhere else.
  - The printed fps figure is kept in `notes`, in the form
    `'Muzzle Velocity FPS' 1475`. If you need a muzzle figure, convert that fps value.
  - Subsonic and Hushpower loads print "SUBSONIC" or "Hushpower" in the fps column and
    still give an m/s figure (295 m/s and 350 m/s). The .410 19 g Low Noise load gives
    no figure at all, so its velocity is null.
- **Length**: the length column is printed like "20/70HD", "16/65", "8/70" or "25/76".
  - `length_mm` is the second number.
  - Gamebore doesn't explain the first number. It is most likely the height of the
    brass head in mm, but that is my reading, not the maker's. "HD" stands for its HD
    case. Two pages print the length alone ("65" or "70").
  - 86 mm is the 3½" case. The 12G Super Game High Bird and the Regal loads are true
    2½" (65 mm) cases.
- **Steel proof**: the maker uses two classes, and `steel_proof` is set from the page.
  - "Standard Steel": any gun in good order with London or Birmingham nitro proof from
    1954 onwards, not Damascus barrels. Half choke maximum.
  - "High Performance" (HP): only for guns with the CIP Fleur de Lys steel proof mark.
    CIP sets no choke limit, but the maker "strongly advises half choke maximum". In the
    tables HP is shown as "HP" after the m/s figure, or in the proof text.
  - Bismuth HD: the maker says there are no choke or pre-1954 proof limits.
- **Wad**: the printed names are "Fibre", "Fibre QS"/"QS Fibre" (fibre Quad Seal),
  "Plastic", "Plastic QS", "Bio-Wad" (full-cup biodegradable), "Bio-Wad Rapid" and "Bio
  Wad Rapid". In the JSON they map to `fibre`, `plastic` and `bio`. The printed name is
  kept in `notes`.
- **Shot**:
  - Lead comes as "Gamebore Lead", "Diamond Shot" or "Diamond Shot Xtra". Xtra is a
    harder shot with added antimony, graded 7 times.
  - The brochure gives densities: lead "HD 11" (11.0 g/cm³), Bismuth HD 9.6, and
    Precision Steel 7.9.
  - Gamebore makes no tungsten load, current or archived, and no slug.

## English shot numbering as Gamebore uses it

Gamebore publishes no separate size chart. Instead, each product page and brochure
section prints its own sizes with mm, and many brochure sections add a label: "English
sized shot", "English sized Diamond Shot" or "Continental sized Diamond Shot (Xtra)".
**The same number means different diameters in different Gamebore lines.** Always take
the mm from the product's own entry, never from the size number alone. Below is each
number with its mm, as printed.

| No | English-sized lead (Gamebore Lead, Regal, Grouse, Pigeon, Super Game, .410, Mammoth, Buffalo) | Continental-sized Diamond Shot / Xtra (Black Gold UER, Dark Storm Game, 20G Black Gold, clay Dark Storm/Onyx/White Gold/Rose Gold) | Precision Steel (12G) | Bismuth HD |
|---|---|---|---|---|
| BB | 4.1 | | | |
| 1 | 3.6 | | 4.0 | |
| 2 | | | 3.81 | |
| 3 | 3.3 | 3.4 | 3.5 | 3.25 |
| 4 | 3.1 | 3.2 | 3.25 | 3.1 |
| 4.5 | | 3.1 | | |
| 5 | 2.8 | 3.0 | 3.0 | 2.8 |
| 5.5 | 2.75 | | | |
| 6 | 2.6 | 2.8 | | |
| 6.5 | | 2.5 | 2.6 (clay steel) | |
| 7 | 2.4 | | | |
| 7.5 | 2.3 | 2.4 | 2.5 (clay steel) | |
| 8 | 2.2 | 2.3 | | |
| 9 | 2.0 (site) / 2.1 (brochure) | 2.1 (Rose Gold) | 2.25 (clay steel) | |

Exceptions as printed:

- 20G Black Gold Steel uses 4 = 3.0 mm and 5 = 2.8 mm, unlike 12G steel.
- 12G DTL Elite uses 7.5 = 2.3 mm and 8 = 2.2 mm. The site calls it "English sized";
  the clay brochure calls it "Continental sized".
- 20G Super Competition uses 7.5/8/9 = 2.3/2.2/2.0 mm on the site, but 2.4/2.3/2.1 mm in
  the brochure.
- The 12G Black Gold HV Steel brochure page carries an "English sized Lead Shot" label
  even though the load is steel.

## Brochure (Feb 2025) vs website (Nov 2026): differences

The JSON uses the website's values and notes these differences:

- **Mammoth steel loads**:
  - The website has "12G Magnum Steel": 36 g, sizes 1, 3 and 4, 76 mm.
  - It also has "12G Mammoth Magnum Steel": 42 g, sizes 1 and 3, 86 mm.
  - The brochure calls both "Mammoth Magnum" and gives the 3½" 42 g load as sizes
    1, 3 and BB.
- **.410 loads**:
  - .410 Hunting 11 g: the site says Plastic wad; the brochure says Fibre and calls it
    2½". The site lists it at 8/65.
  - .410 Target: the site says 8/70; the brochure says 8/65.
- **Bismuth HD case**: the site prints the 12G length as "65" and the 20G as "70"; the
  brochure prints 16/65 and 16/70.
- **Only in the brochure**: these have no product page and are in the JSON with
  `source_url` pointing at the PDF page. They may be discontinued.
  - **12G Velocity+ HV Pigeon**: 30 g No 6, plastic or fibre wad, 8/70, "1390"
    (presumably fps). No m/s and no mm are given.
  - **28g Target**: 21 g No 9 (2.1 mm), plastic, 8/70, "1400".

## What's missing

- No m/s figure for 12G Velocity+ HV Pigeon or 28g Target: the brochure gives only an
  fps-style "VELOCITY". No mm for the Velocity+ No 6.
- The .410 Hunting sizes 5.5 and 7.5 have no mm on the product page. I took them from
  the brochure (2.75 and 2.3 mm) and said so in `notes`.
- Pellet counts, pattern percentages, chamber pressures and shot hardness figures are
  not published. Wad mass is also not published; the site says only "2.5x heavier than
  the equivalent plastic wad" for the UER Bio-Wad.
- Some loads are mentioned in the text but missing from the tables:
  - 12G Super Game High Bird is said to come with fibre and plastic wads, but the table
    lists only fibre.
  - The 28G Regal text mentions a "Regal 27gms wool wad load", but it isn't in the
    table. The archived 28G Pure Gold page had a 27 g wool load at 427 m/s.
- No slugs and no tungsten (TSS) loads exist in the range.
- No `bc_g1` applies to shot; it stays null.
- I found no catalog data at all for "Mullerite", on gamebore.com or in the Wayback
  index of its product URLs. Web searches found nothing tying it to Gamebore either. If
  the name matters, it needs another source.
- I found no "White Gold" game load, current or archived. White Gold is a clay line
  today: 12G White Gold, 12G White Gold Pro Steel and 20G White Gold.

## Discontinued lines and their current names

Every URL below now redirects to gamebore.com/404. Specs for the game lines, where an
archive copy exists, are in `gamebore-discontinued.json`.

**Game:**

| Old product (last archived) | Current replacement |
|---|---|
| 12G Black Gold Game (2022-11): 28–40 g, fibre or wool wad, 20/70, sizes 4/5/6 (3.2/3.0/2.8 mm) | 12G Black Gold Game UER, and 12G Dark Storm Game (Black Gold Dark Storm) |
| 12G Silver Steel Fibre (2022-09): 32 g No 4, FSC, 25/76 | Steel line: Dark Storm Steel (incl. 3"), Super Steel, Magnum Steel, Mammoth Magnum Steel, Regal Steel |
| 12G Pure Gold F2 and 12G Pure Gold Paper Case (2016): 65 mm game loads | 12G Regal Game (incl. Paper Case rows) |
| 28G Pure Gold (2016) | 28G Regal Game |
| 20G Traditional Game (2016): same loads as today's 20G Regal Game | 20G Regal Game |
| 16G Traditional Game (archive copy not retrieved) | 16G Regal Game |
| 20G Pure Gold, 12G TMX, 12G/20G Impact, 12G Velocity Pigeon, 20G Steel Hunting | No direct successor named. 12G Impact (2016) is in the discontinued JSON; for the others the Wayback index lists the URL, but no copy could be retrieved (archive.org gave empty answers, maybe rate limiting) |
| 12G Dark Storm Game (old URL; 2016 also listed a 32 g wool option) | 12G Dark Storm Game (with Quad Seal) |

**Clay** (URLs only; specs not collected): 12G Black Gold Clay, 12G Black Gold
Competition, 12G Black Gold F2, 12G Blue Diamond, 12G Dark Storm Competition, 12G Evo and
Evo Low Noise (now 12G Evo R with a Low Noise row), 12G Platinum, 12G Pure Gold Paper
(clay), 12G Super Steel Competition, 12G Velocity Plus, 12G White Gold Pro Steel (now at
`12g-white-gold-steel`), and 20G Super Competition Low Noise (now a row in 20G Super
Competiton).

**Spelling as printed:** "20G Super Competiton" is the site's spelling; the brochure
spells it "Competition".
