# How popular each caliber is

Each caliber in `crates/ballistics-store/catalog/calibers.json` has a
`popularity` block. It grades, from 1 (rare) to 5 (very common), how common
the caliber is:

- **for hunting:** Nordic countries, the rest of Europe, North America;
- **for civilian rifle competition, by discipline:** precision rifle
  (PRS/NRL), F-class, benchrest, extreme long range, rimfire, and Nordic
  field and range shooting.

Military use doesn't count. Graded November 2026.

```json
"popularity": { "hunting_nordic": 5, "hunting_europe": 5, "hunting_north_america": 5, "precision": 3, "f_class": 5, "benchrest": 1, "long_range": 1, "rimfire": 1, "nordic_field": 3 }
```

To list the most popular calibers for one use:

```sh
cargo run -q -p ballistics-store --example popular -- hunting_europe 20
```

Calibers with the same grade are listed by how many factory loads the
catalog has for them.

## These are rough grades

Nobody publishes caliber sales, and the app collects nothing about its
users, so the grades come from public stand-ins:

- **Good for:** "the twenty most popular", or common against rare.
- **Not good for:** telling #7 from #8.
- **What a 1 means:** no factory rifle and few factory loads in the sources,
  not that nobody uses it.

**Hunting:** 60 % how many of a region's rifle makers chamber the
caliber, 40 % how many factory loads its ammunition makers offer.

- **Factory loads** are counted from the app's own bullet catalog. The
  load count is scaled so the 90th percentile of calibers counts as full.
  - Europe: Norma, Lapua, Sako, RWS, Geco, Sellier & Bellot, Fiocchi, Prvi
    Partizan, GGG, SK, CopperBear, Tarvas.
  - North America: Federal, CCI, Hornady, Remington, Winchester, Barnes,
    Berger, Aguila.
- **Rifles:** the calibers the makers' current bolt-action hunting rifles
  are offered in, from their own sites (November 2026).
  - Europe: Sako 90, Tikka T3x, Blaser R8 (its 2026 caliber overview),
    Sauer 100 and 404/505, Mauser 18.
  - North America: Ruger American and Hawkeye, Savage 110, Browning X-Bolt,
    Winchester Model 70, Bergara B-14. Some of these lists cover only a few
    model variants and may be short.
- **Nordic:** Sako and Tikka with the Nordic ammunition makers (Norma,
  Lapua, Sako, SK, CopperBear, Tarvas), blended 60/40 with the European
  score, since Blaser and Sauer sell widely there too.
- **Grades** from the combined score: 5 from 0.65, 4 from 0.40, 3 from 0.20,
  2 from 0.04, else 1.
- **What the count misses:** double rifles and combination guns, so rimmed
  calibers for them (9.3×74R, 7×65R, 8×57 IRS and the like) grade lower than
  their real use in Europe. So do calibers that are mostly reloaded.

**Competition:** each discipline graded from one or two recent sources.
A published share of 20 % or more gives a 5, 8 % a 4, 3 % a 3, less a 2.
Without numbers: "dominant" 5, "common" 4, "occasional" 3. Not mentioned,
or not allowed: 1.

- **Precision rifle:** PrecisionRifleBlog, "What the pros use", top 200 PRS
  Open shooters (2025): 6mm Dasher 46 %, 6mm GT 9 %, 6.5 Creedmoor, .308 Win
  and 6mm Creedmoor 5 % each, 6mm BR 4 %.
- **F-class:** Accurate Shooter (2025).
  - F-Open: .284 Win and its kin are the most common; .300 WSM and 7mm SAUM
    are common.
  - F-T/R: limited to .308 Win and .223 Rem.
- **Benchrest:** Accurate Shooter (2024–25): 6mm PPC dominates group
  shooting and .30 BR score shooting; 6mm BR and Dasher at 600 yards.
- **Extreme long range:** King of 2 Miles finals (2023–2026), mostly
  .375 CheyTac and the .416s (Hellfire and Barrett, which the catalog
  lacks).
- **Rimfire:** ISSF, biathlon and NRL22 are all .22 LR; NRL22 bars .17 HMR
  and .22 WMR.
- **Nordic field shooting:** the Swedish federation's rule book (2023) and
  the Norwegian DFS.
  - Gevär 6,5 is 6.5×55 only; scoped classes allow bolt-actions up to 8 mm.
  - 6mm BR and 6.5×47 Lapua are common at 300 m.

25×47 Lapua, 25 Creedmoor, 6mm BRA, 25 GT, .416 Barrett and .416 Hellfire
were added for these sources. 25×47 Lapua, 25 GT and 6mm BRA are wildcats,
built from their parent cases. The .416 Hellfire comes from Manson's public
chamber print, with the CheyTac head assumed, since the print doesn't give
one.

## Updating

Redo it about once a year:

1. Recount the factory loads from the catalog.
2. Fetch the rifle makers' current caliber lists and the latest
   competition surveys.
3. Regrade with the rules above, and update the date here.
