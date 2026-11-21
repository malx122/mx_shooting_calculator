# Gyttorp shotgun shells: research note (checked 2026-11-21)

## Who makes "Gyttorp" today

- Gyttorp no longer exists as a shotshell brand. RUAG Ammotec, through Norma Precision, bought all
  of Gyttorp AB in 2017 (Norma history page: "Norma acquires the shotshell factory Gyttorp in
  Sweden"). In 2019 the Swedish sales company launched the range under the **Norma** name and
  "successivt fasar ut det gamla anrika sortimentet från Gyttorp" (Jaktkatalogen 2019). Svensk Jakt
  (5 April 2020) and Jakt & Jägare (2023) say the Gyttorp name was gone from shotshells by 2020.
  Some of the Norma names come from the old Gyttorp loads: Special, Grouse, Magnum, Wetland, Steel Max.
- Norma has been owned by **Beretta Holding** since July 2022 (Norma history page).
- The factory: Jakt & Jägare (6 Oct 2023) quotes Norma Precision AB's HR chief: "Ja, vi stänger
  fabriken i Nora och flyttar till Spanien". The article doesn't name the Spanish maker, and I
  found no maker page that does. So the current Norma shotshells are probably no longer made at
  Gyttorp/Nora. Who loads them in Spain is **unconfirmed**.
- Gyttorp is now only a Norma/Ammotec Sweden brand for accessories (oils, axes, bait lights,
  cartridge boxes). Retailers say "Gyttorps patroner kommer framöver att marknadsföras och säljas
  under varumärket Norma" (Torsbo Handel brand page).
- `gyttorp.json` therefore lists the **current Norma 12/70 shotshell range** (the successor range).
  It has 12 product lines and 28 SKUs, all 12/70 with plastic hulls. No 16, 20, 28 or .410 gauge
  shells, no slugs, and no bismuth or tungsten loads.

## Pages used

Maker (Norma / its Swedish sales company):
- Norma caliber page 12/70 (lists all 28 products): https://www.norma-ammunition.com/sv/produkter/kalibrar/12-70
- The 28 Norma product pages (load g, size with mm, velocity labelled V0, "Syfte" Jakt/Skytte),
  found through https://www.norma-ammunition.com/sv/produkter/sitemap/sitemap-products-sv.xml.
  Each URL is in the JSON.
- Norma family descriptions (archived old en-gb site, Dec 2025 to Mar 2026), e.g.
  https://web.archive.org/web/20251127050616/https://www.norma-ammunition.com/en-gb/products/dedicated-hunting/shotgun-shells/norma-wetland
- Norma history: https://www.norma-ammunition.com/about-norma/history
- RUAG Ammotec Sweden on gyttorp.se, launch article 28 May 2019 (table with type, hull, g, sizes, "M/sek V1"):
  http://web.archive.org/web/20190823001844/http://www.gyttorp.se/2019/05/28/normahagelpatroner/
- Jaktkatalogen 2019 PDF (RUAG Ammotec Sweden), pp. 2, 12, 13:
  http://web.archive.org/web/20200910112315/http://www.gyttorp.se/wp-content/uploads/2019/05/Jaktkatalogen2019.pdf
- Jaktkatalogen 2018 p. 22-23 (old Gyttorp range, used only for the discontinued list):
  https://web.archive.org/web/2019/http://www.gyttorp.se:80/jaktkatalogen2018/22/
- Ammotec Sweden AB product pages (archived 2023; the live site no longer resolves). They give
  descriptions, materials, and the Grouse wad. For example:
  https://web.archive.org/web/20230401051808/https://www.ammotecsweden.se/norma-cup-jaktstig-28-1270-us9

Retailers (used only to fill gaps, and marked as such in `notes`):
- Motonet.fi Norma Special 36 #5 (plastic wad "Muovinen välitulppa"): https://www.motonet.fi/tuote/norma-special-36-g-1270-5-10-kpl?product=55-10995
- Motonet.fi Norma Steel Max Match 28 #7 (use, V0): https://www.motonet.fi/tuote/norma-steel-max-match-1270-28-g-7-25-mm-432-gr-25-kpl?product=55-24669
- Kjells Vapen, Caliber44, Torsbo Handel, jakt.se: brand status and leftover stock only.

Dead maker sites: `gyttorp.se` serves a hosting provider's "Inaktiv sida" page. `http://www.gyttorp.se`
redirects to `ruagammotec.se`, which did not respond. `gyttorp.com` does not resolve.

## What I couldn't find

- **Wad** for Magnum, Extra, Wetland, Jaktstig, and all Lead Max and Steel Max loads. Only two are
  known: Grouse is plastic (maker side) and Special is plastic (retailer). Steel Max Match 24 has "en
  mycket exklusiv förladdning", but the material isn't stated.
- **Steel-proof / HP status** for Wetland and Steel Max. It isn't stated anywhere, so `steel_proof` is null.
- **Where velocity is measured.** The current Norma pages say V0. The 2019 sales-company table gave
  the same numbers as "V1". The JSON uses `v0` and notes the conflict.
- The mm for the **Magnum 42 duplex No 2-4** is printed only as the range "3,2 - 3,7 mm", so it's left empty.
- Pellet counts, chamber pressure, pattern data, and any BC. There are no slugs in the range.
- A current (2025/2026) Norma catalogue PDF. None turned up.
- The name of the Spanish contract loader.

## Discontinued

- **The whole Gyttorp-branded range** (phased out 2019-2020). From Jaktkatalogen 2018:
  - Gyttorp Grouse 32 g (US 5, 6, 7; 390 m/s V1)
  - Gyttorp Fasan 30 g nickel-plated lead (paper hull 65 mm, felt wad)
  - Gyttorp Special 36 g (US 1, 3, 5)
  - Gyttorp Gås 36 (US 4)
  - Gyttorp Röda 30 g / 35 g (paper hull 65 or 70 mm, felt wad; 35 g Röda also in a plastic hull, US 5 and 6)
  - Gyttorp Magnum 42 g (US 1, 3, 5, duplex 2+4)
  - Gyttorp Nickel Magnum 40 g (plastic hull, US 3 and 5)
  - Gyttorp Wetland 28 (steel, US 3, 4, 5; 425 m/s) and Wetland 32 (steel, US 2, 3, 4; 420 m/s)
  - Gyttorp Fågel and Fågel 30
  - Gyttorp Hunting in 16 and 20 gauge
  - **Gyttorp Slug** 12/70: 32 g solid lead, 490 m/s V1, plastic hull
  - The old Gyttorp Steel Max (sport)

  Svensk Jakt (2020) says Norma has no 16 or 20 gauge range, and the Gyttorp 16/20 stock was being sold off.
- **Norma Jaktmatch 24 12/70 No 7 / 2,5 mm** (steel, training, sold since 2017). It was on Norma's site in
  2022 under product number 77112844, which now belongs to Norma Jaktstig 28. Some retailers still list
  it (Caliber44, ammocenter.se), but the maker doesn't.
- **Norma Special 36 US1** was listed by Ammotec Sweden in 2023 but isn't on the current Norma site.
- Norma Wetland 28 No 5 and Wetland 32 weren't carried over: Norma Wetland is 28 g, No 3 and No 4 only.
