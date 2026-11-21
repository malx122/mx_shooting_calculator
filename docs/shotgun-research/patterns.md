# How shotgun patterns form and spread: notes for the pattern model

Researched 2026-11-21. Every number has a source key in square brackets; the keys are listed in
§10. These marks are used throughout:

- **[unverified]**: taken from a search-result summary, a forum post, or a page I could not open.
  It is not confirmed against the primary text.
- **[derived]**: my own calculation from the cited tables, with short fitting scripts that were not
  kept. The engine's tests (`crates/ballistics-core/src/tests.rs`) check the final model against the
  same tables.

### What could not be read

- **Oberfell & Thompson.** *The Mysteries of Shotgun Patterns* (Oklahoma State University Press,
  1957; 1960 edition, 328 pp.; Riling reprint 2005) is not online. Only secondary accounts of it
  were reachable.
- **Burrard.** *The Modern Shotgun* vol. III is on archive.org only as a restricted lending item
  (`modernshotgunvol0000majo`), so its text was not readable. Burrard's numbers here come through
  BASC 2008 and through the Gough Thomas / Eley tables reproduced in Spencer 2001.
- **Journée.** Général Journée, *Tir des fusils de chasse* (Gauthier-Villars, 3rd ed. Paris 1920,
  77 figures and 66 tables [Feu-Follet]) was not found in digitised form. **No number in this note
  comes from Journée directly.** The British range tables may descend from Journée, but that link
  is **[unverified]**.

---

## 0. Summary of findings

1. **The angular spread is not constant with range.** Every classic table (Gough Thomas, Eley,
   Burrard) and the forensic literature show a pattern that grows *faster than linearly* with
   range, and more so the tighter the choke.
   - Fitting σ = b·(d − d₀) to the Gough Thomas percentage table puts the "virtual origin" d₀ of the
     spread downrange of the muzzle, and further out for tighter chokes [derived]:
     | Choke | d₀ |
     |---|---|
     | True cylinder | 4 m |
     | Improved cylinder | 6.5 m |
     | Half choke | 8 m |
     | Three-quarter choke | 10 m |
     | Full choke | 12.5 m |
   - The current constant-angle model misses the table by up to 8 percentage points for Full at
     60 yd. A one-parameter "linear plus quadratic" model fits within 2.6 points (§8).
2. **A circular bivariate normal is the right lateral distribution.** Its radius then follows a
   Rayleigh distribution.
   - Gough Thomas's share of the 30-in pellets that fall inside the 20-in circle (46–62 %, rising
     with choke) agrees with the Gaussian prediction to about one point [GT via Spencer; derived].
   - Forensic coordinate studies find x and y each about normal [Walsh 2007].
   - A minority report finds non-Gaussian buckshot and cylinder patterns [FSI 1989, unverified].
3. **Patterns are close to random (Poisson) at the 5-inch scale.** They are slightly patchier than
   Poisson at 150–250 pellets in the 30-in circle (§3.4).
   - Shot-to-shot variation is much larger than Poisson sampling alone. Single pattern percentages
     range 31–41 % for No. 7 from a cylinder barrel, and 23–68 % for BB from quarter choke [BASC 2008].
4. **Choke names map onto percentages more reliably than onto constrictions.** The same 0.010 in
   is "Improved Cylinder" in the US and "¼ choke" in Britain, and the German fraction scale is
   lighter again (§1).
5. **Steel patterns tighter than lead** by about 1–2 choke steps, roughly +10 points at 40 yd. It
   stops gaining beyond about half choke, and CIP and the makers limit the choke for steel (§1.4).
6. **Shot string.**
   - Length at 40 yd: lead 11–12 ft (3.4–3.7 m), steel 7–9 ft (2.1–2.7 m), passing in about
     9–15 ms [Shooting Times; derived].
   - A 40-mph crossing bird moves 0.15–0.27 m during that time. Modern measurements put the loss of
     density on the bird at about 5 %; Brister's 1970s lead loads lost about 30 % [Field & Stream].

---

## 1. Chokes: names, constrictions, nominal percentages

### 1.1 Constriction systems (12 gauge)

There are three incompatible naming systems, so **key the engine on the nominal percentage, not
on the constriction.**

**British (English) system** [Spencer 2001 T5]. Pattern percentage: the share of the charge in a
30-in circle at 40 yd [Burrard via BASC 2008].

| British name | Constriction (in) | Constriction (mm) [derived ×25.4] | % at 40 yd |
|---|---|---|---|
| True cylinder | 0 | 0 | 40 |
| Improved cylinder | 0.005 | 0.13 | 50 |
| ¼ choke | 0.010 | 0.25 | 55 |
| ½ choke | 0.020 | 0.51 | 60 |
| ¾ choke | 0.030 | 0.76 | 65 |
| Full | 0.040 | 1.02 | 70 |

Burrard (1948, *The Modern Shotgun* III) says these percentages "are approximate for all sizes of
guns and shot" and form "a universal rule". Eley's Shooter's Diary reprints them every year
[BASC 2008].

**US / tube makers' system.** Beretta OCHP chart, lead shot [Beretta]; constrictions agree with
Shotgun Life's typical 12-gauge values [ShotgunLife].

| US name | Letters | Stars | Constriction (in) | Constriction (mm) | UK equivalent | Beretta colour |
|---|---|---|---|---|---|---|
| Cylinder | C | ★★★★★ | 0.000 | 0 | | Blue |
| Skeet (USA) | SK | | 0.005 | 0.13 | | Purple |
| Improved Cylinder | IC | ★★★★ | 0.010 | 0.25 | ¼ | Yellow |
| Light Modified | LM | | 0.015 | 0.40 | | Orange |
| Modified | M | ★★★ | 0.020 | 0.50 | ½ | Green |
| Improved Modified | IM | ★★ | 0.025 | 0.60 | ¾ | Black |
| Light Full | LF | | 0.030 | 0.75 | | Grey |
| Full | F | ★ | 0.035 | 0.90 | Full | White |
| Extra Full | XF | | 0.040 | 1.00 | | |

- Shotgun Life gives the same steps up to Improved Modified, then Full 0.030 in and Extra
  Full/Turkey 0.035–0.045 in+ [ShotgunLife].
- Its 40-yd percentages are Cyl 40, SK 45, IC 50, LM 55, M 60, IM 65, F 70, XF 75–90 [ShotgunLife].

**European star markings** [Spencer 2001 T6, from Gough Thomas]:

| Marking | Choke | Constriction |
|---|---|---|
| CL | True cylinder | 0 |
| ★★★★ | Improved cylinder | 0.2–0.3 mm |
| ★★★ | ½ | 0.4–0.6 mm |
| ★★ | ¾ | 0.7–0.8 mm |
| ★ | Full | 0.9–1.0 mm |

Beretta adds ★★★★★ for Cylinder [Beretta].

**Swedish, Italian and other marks with percentages in the 75-cm circle at 35 m** [Hassel 2023,
Bilaga 9; also ShotgunsSE-choke]. Hassel defines this percentage on *lead* loads.

| Choke | Marks | % at 75 cm / 35 m | Constriction [ShotgunsSE-choke] |
|---|---|---|---|
| Cylinder | CYL, C, IIIII, ★★★★★ | 30–40 | 0 |
| Skeet | SK, 1/8 | 40–50 | 0.005 in, 0–0.1 mm |
| Improved Cylinder | ++, 1/4, IC, IIII, ★★★★ | 55 | 0.010 in, 0.1–0.2 mm |
| (3/8) | | 57 | 0.2–0.3 mm |
| Modified | +-, 1/2, M, III, ★★★ | 60 | 0.020 in, 0.4–0.6 mm |
| (5/8) | | 63 | 0.6–0.7 mm |
| Improved Modified | ++-, 3/4, IM, II, ★★ | 65 | 0.025 in, 0.7–0.8 mm |
| (7/8) | | 67 | 0.8–0.9 mm |
| Full | +, 1/1, F, I, ★ | >70 | 0.035 in, 0.9–1.1 mm |

**German fraction scale** [DJL]. Constrictions in mm, with what each would be called in the other
systems:

| Constriction (mm) | German (lead) | English | US / steel |
|---|---|---|---|
| 0 | Zylinder | | |
| 0.125 | Skeet | Skeet | SK |
| 0.250 | ¼ | ¼ | IC |
| 0.380 | ½ | ¼–½ | LM |
| 0.500 | ¾ | ½ | M |
| 0.625 | 1/1 (Voll) | ¾ | IM |
| 0.750 | | ¾–1/1 | LF |
| 0.850 | | 1/1 | F |
| 1.050 | | 5/4 | XF |

- **A German "½" is about a US Light Modified, and a German "Voll" (1/1) is about a US Improved
  Modified.**
- DJL still gives roughly the same percentages at 35 m / 75 cm with about 3-mm shot: ¼ ≈ 55 %,
  ½ ≈ 60 %, ¾ ≈ 65 %, Voll > 70 %.

**Nominal 40-yd percentage per US name, other sources:**

- Jack O'Connor, via North American Outdoors: Cyl 35–40, Skeet 35–40, IC 45–50, M 55–65, IM 65–70,
  F 70–80, XF > 80 [NAO, unverified, from a search summary].
- The guncustomizing.com chart (unattributed) gives the 12-gauge constriction bands with
  percentages: Cyl .000–.001 → 40; Skeet .002–.006 → 45; IC .007–.013 → 55; M .014–.023 → 60;
  IM .024–.031 → 65; F .032–.040 → 70; XF .040+ → 75 [guncustomizing].

**A naming mismatch to note.** The Beretta chart [Beretta p.3] gives these percentages at
20/30/40 yd:

| Choke | 20 yd | 30 yd | 40 yd |
|---|---|---|---|
| Cylinder | 80 | 60 | 40 |
| Skeet | 92 | 72 | 50 |
| IC | 100 | 77 | 55 |
| M | 100 | 83 | 60 |
| IM | 100 | 91 | 65 |
| F | 100 | 100 | 70 |

- These are Gough Thomas's British rows TC / IC / ¼ / ½ / ¾ / F relabelled with US names
  (compare §2.1).
- So in Beretta's chart US "IC" (0.010) carries 55 %, while US sources usually give IC 50 %. The
  difference is about 5 points, which is inside the shot-to-shot noise (§3.5).

### 1.2 20 gauge

Choke diameters from WaterfowlChoke [WFC]; constrictions [derived] = bore minus choke.

| System | Bore (in) | Skeet | IC | M | IM | Full | XF |
|---|---|---|---|---|---|---|---|
| Invector Plus 20 | 0.630 | .004 (0.10 mm) | .011 (0.28) | .018 (0.46) | .021 (0.53) | .028 (0.71) | .032 (0.81) |
| Beretta/Benelli Mobil 20 | 0.627 | .004 | .011 | .018 | .021 | .028 | .032 |
| Fixed 20 | 0.625 | .004 | .022 | .030 | .035 | .040 | |
| Invector Plus 12 (comparison) | 0.740 | .005 | .010 | .020 | .025 | .030 | .040 |

- **Fixed 20 gauge:** the IC to Full constrictions look implausibly large. Treat that table row as
  doubtful.
- **20-gauge bands** from guncustomizing.com (unattributed): Cyl .000–.001, Sk .002–.004,
  IC .005–.011, M .012–.019, IM .020–.026, F .027–.033, XF .034+ [guncustomizing].
- **Percentages by gauge.** Spencer 2001 (after Gough Thomas): "All shotguns of a given choke
  specification – regardless of gauge – should throw the same percentage … and have the same
  diameter of pattern." The pellet count changes with gauge; the percentage does not.

### 1.3 Variation behind the nominal values

Giblin & Compton (UCL Ballistics Research Laboratory) fired 10 shots per choke at 40 yd. They
used one test barrel with linear-taper chokes of 0, 10, 20, 30, 40 and 50 thou, and seven loads:
lead 2.4 / 3.1 / 4.25 mm, bismuth 3.1, steel 2.8 / 3.3, zinc 2.9 [BASC 2008].

- **Percentage against choke.** It rises from cylinder to ¼ and usually to ½, then is flat. It
  *drops* from Full (40 thou) to "super full" (50 thou).
- **No. 7 lead (2.4 mm).**
  - Cylinder shots ranged 31–41 %, half choke 54–68 %.
  - Real differences only from cylinder up to ¾. "A pattern measured at around 70 % could have been
    fired from a half, three-quarter, full and super full choke."
- **BB+ lead (4.25 mm).**
  - Quarter choke ranged 23–68 %, super full 32–75 %.
  - Means: cylinder 34 %, quarter 47 %, half 63 %.
- **Steel.**
  - 2.8 mm steel improves only up to half choke.
  - 3.3 mm steel sits at about 45 % for any choke from quarter to super full.
- **Bismuth and zinc.** Bismuth 3.1 mm stops improving at half choke, zinc 2.9 mm at ¾.

A German field test is consistent with this [all4shooters]. It shot 94 combinations at 30 m into
a 75-cm circle and averaged about 65 % for Full and about 53 % for Quarter choke.

### 1.4 Steel shot

How much tighter:

- **Federal underground range, 40 yd** [Field & Stream]. Steel #3 at 1600 fps patterned **62.5 %**
  against lead #6 at 1500 fps **52.6 %** (+10 points). Choke not stated in the article.
- **Hassel 2023.** Steel "often gives one to two degrees tighter choke than normal lead loads".
  - A ½ choke with steel behaves like at least ¾ (65 %).
  - A ¾ choke with steel behaves like Full, "around 75 %".
  - Over-choking makes the gun "spray" (uncontrolled spread).
- **Roster.** "It's not that steel shoots 'too tight' … it's that lead shoots 'too loose'". He
  advises opening the choke one step, for example Modified to IC, for steel [MT Outdoors 2013].
- **Beretta.** "Steel and Tungsten shot typically patterns tighter than lead". Beretta "strongly
  recommends" no tighter than Modified for steel, tungsten and other non-lead shot [Beretta].
- **Plateau.** Steel 2.8 mm gains nothing beyond ½ choke; steel 3.3 mm gains nothing beyond ¼
  [BASC 2008, Giblin & Compton].

Limits:

- **CIP High-Performance steel** (fleur-de-lys steel proof): steel above 4 mm in 12 bore, or above
  3.5 mm in 20 bore, only with chokes under 0.5 mm [CIP via steel-shot.com].
  - Shooting UK states the CIP recommendation as "half choke maximum" for HP steel, and "full choke
    maximum" for Standard steel in guns with heavier barrels.
  - UK proof houses recommend at most ¼ choke for light English nitro-proofed guns [ShootingUK].
  - Hassel: HP steel requires steel proof and choke ≤ ½ [Hassel 2023].
- **CIP steel classes, 12 bore** [CIP via steel-shot.com]:

  | Class | Pressure | Velocity at 2.5 m | Shot | Momentum |
  |---|---|---|---|---|
  | Standard | 740 bar | ≤ 400 m/s | ≤ 3.25 mm | ≤ 12 Ns |
  | High Performance | 1050 bar | ≤ 430 m/s | | 13.5 Ns (12/70), 15 Ns (12/73+) |

  Shot hardness: below HV1 110 at the surface and below 100 in the core.
- **CIP choke cone** for steel-proof barrels: at most 0° 30′, and at most 1° in the middle
  [Hassel 2023; CIP via steel-shot.com].

---

## 2. Pattern percentage and spread against range

### 2.1 Percentage in the 30-inch circle, by range and choke

**Table A: yards** (Gough Thomas, via Spencer 2001 Table 7):

| Boring | 30 yd | 35 | 40 | 45 | 50 | 55 | 60 |
|---|---|---|---|---|---|---|---|
| True cylinder | 60 | 49 | 40 | 33 | 27 | 22 | 18 |
| Improved cyl. | 72 | 60 | 50 | 41 | 33 | 27 | 22 |
| ¼ choke | 77 | 65 | 55 | 46 | 38 | 30 | 25 |
| ½ choke | 83 | 71 | 60 | 50 | 41 | 33 | 27 |
| ¾ choke | 91 | 77 | 65 | 55 | 46 | 37 | 30 |
| Full | 100 | 84 | 70 | 59 | 49 | 40 | 32 |

**Table B: metres** (Eley Shooter's Diary 1992, via Spencer 2001 Table 8):

| Boring | 20 m | 25 | 30 | 35 | 40 | 45 | 50 | 55 |
|---|---|---|---|---|---|---|---|---|
| True cylinder | 75 | 63 | 53 | 43 | 35 | 28 | 22 | 18 |
| Improved cyl. | 85 | 74 | 64 | 53 | 43 | 34 | 27 | 22 |
| ¼ | 90 | 80 | 70 | 58 | 48 | 39 | 31 | 25 |
| ½ | 97 | 86 | 76 | 64 | 54 | 43 | 34 | 27 |
| ¾ | 100 | 93 | 83 | 70 | 58 | 47 | 38 | 30 |
| Full | 100 | 100 | 90 | 74 | 62 | 51 | 41 | 32 |

**Table B is a unit conversion of Table A, not independent data** [derived]. For example, Table A
interpolated to 35 m (38.3 yd) gives about 43 % for TC, the same as Table B.

**US 20/30/40-yd versions.**
- They appear in the Beretta chart [Beretta], Wildfowl magazine Chart 1 [Wildfowl] and the GMK
  chart, with these 40-yd / 30-yd values: Cyl 40/60, Skeet 50/72, IC 55/77, M 60/83, IM 65/91,
  F 70/100.
- These are columns of Table A with the names mapped. Only the 20-yd column is new: Cyl 80,
  Skeet 92, all others 100.

**Structure of Table A** [derived]:
- **Each row is the previous one shifted.** IC(d) ≈ TC(d − 5 yd), ½(d) ≈ IC(d − 5 yd), and
  Full(d) ≈ ½(d − 5 yd). So Full is TC shifted 15 yd.
- This matches Gough Thomas's statement (1975, *Shotguns and Cartridges for Game and Clays*,
  Table 2 p. 73) that a 40 % cylinder pattern at 40 yd is "extended to 55 yd by a full choke"
  [BASC 2008].
- **Each row falls by a factor of about 0.82 per 5 yd**, roughly exp(−0.040 per yard).

**Range at which each choke throws 70 %:** Full 40 yd, IM 38, M 35, LM 30, IC 28, Skeet 25,
Cylinder 20 [forum post, unverified]. This is roughly the same structure.

**A cruder, unattributed US table** drops 10 points per 5 yd from 100 % [guncustomizing]:
- Full is 100 at 25 yd, falling to 30 at 60.
- Cylinder is 100 at 10 yd, falling to 10 at 55.
It is linear in percentage and does **not** agree with Table A. **Do not test against it.**

### 2.2 Spread diameter, by range and choke

The spread is "the pattern diameter that is covered by the bulk of the shot charge" [Spencer 2001].

**Table C: inches, yards** (Gough Thomas, via Spencer 2001 Table 11):

| Boring | 10 yd | 15 | 20 | 25 | 30 | 35 | 40 |
|---|---|---|---|---|---|---|---|
| True cyl | 20 | 26 | 32 | 38 | 44 | 51 | 58 |
| Imp. cyl | 15 | 20 | 26 | 32 | 38 | 44 | 51 |
| ¼ | 13 | 18 | 23 | 29 | 35 | 41 | 48 |
| ½ | 12 | 16 | 21 | 26 | 32 | 38 | 45 |
| ¾ | 10 | 14 | 18 | 23 | 29 | 35 | 42 |
| Full | 9 | 12 | 16 | 21 | 27 | 33 | 40 |

**Table D: centimetres, metres** (Eley, via Spencer 2001 Table 10):

| Boring | 10 m | 15 | 20 | 25 | 30 | 35 |
|---|---|---|---|---|---|---|
| True cyl | 54 | 71 | 88 | 105 | 122 | 140 |
| Imp. cyl | 38 | 55 | 72 | 89 | 106 | 124 |
| ¼ | 34 | 49 | 64 | 80 | 97 | 115 |
| ½ | 31 | 44 | 58 | 73 | 90 | 108 |
| ¾ | 27 | 39 | 52 | 66 | 82 | 101 |
| Full | 23 | 33 | 45 | 59 | 75 | 94 |

**Copies of the same table:**
- Wildfowl Chart 4 at 10/20/30/40 yd: Cyl 20/32/44/57, IC 15/26/38/51, Mod 12/20/32/46,
  Full 9/16/26/40, and a "spreader" row of 23/37/51/66 [Wildfowl].
- guncustomizing.com at 20/30/40 yd: Cyl 32/44/58, IC 26/38/51, Mod 21/32/45, Full 16/27/40
  [guncustomizing].
- These are almost certainly the same Burrard / Gough Thomas table. Its Burrard origin is
  **[unverified]**.

**Other estimates:**
- **A different, much tighter table** [ShotgunLife], unattributed, at 20/30/40 yd:
  - Cyl 20–22 / 30–34 / 40–44 in
  - Full 11–13 / 18–21 / 20–24 in

  It is inconsistent with Table C, presumably because it uses a different, core-only definition.
  **Do not use it as a reference.**
- **Kanstrup:** at 25 m the swarm is "over 2 m long and 1 m in diameter, elliptic" [Kanstrup 2010].
- **Roster:** the swarm at the bird is 3–5 ft across and 6–8 ft long [MT Outdoors 2013].
- **Forum summary:** at 36 m a cylinder spreads 3.05 m and a full 2.45 m, and at 20 m a cylinder
  gives 1 m and ¼ choke 80 cm [French forum summary, unverified]. These look like total-spread
  figures.

**What "bulk" means** [derived]. With the Gaussian σ fitted to Table A, the Table C diameters hold:

| Range | Share of pellets inside the Table C diameter |
|---|---|
| 30–40 yd | **85–89 %** |
| 20 yd | about 95 % |
| 10–15 yd | about 99–100 % |

- So at hunting ranges, "spread" ≈ the 85 % circle. This matches the German *Kerngarbe* (core
  swarm), defined as holding about 85 % of the pellets [DJL].
- At short range, the table's "spread" behaves like the total spread.

### 2.3 Is the angular spread constant? (spread = a + b·d?)

**No.** The evidence:

1. **The successive increments in Table C grow for tight chokes** [derived].
   - Full adds 3, 4, 5, 6, 6, 7 in per 5 yd; true cylinder adds 6, 6, 6, 6, 7, 7.
   - Fitting D = a + b·d (inches, yards) gives:

     | Choke | a (in) | b (in/yd) |
     |---|---|---|
     | TC | +7.0 | 1.257 |
     | IC | +2.3 | 1.200 |
     | ¼ | +0.5 | 1.164 |
     | ½ | −0.4 | 1.100 |
     | ¾ | −2.2 | 1.064 |
     | F | −3.5 | 1.043 |

   - The Eley metric fit (cm, m) runs from a = +19.5 cm, b = 3.43 cm/m for TC to a = −8.8 cm,
     b = 2.83 cm/m for Full.
   - So the intercept falls from positive for an open barrel to *negative* for choked ones.
2. **The percentages in Table A imply the same.** Assume a Gaussian, find the σ that gives each
   tabulated percentage, and fit σ = b·(d − d₀) [derived]:

   | Choke | b (in/yd) | b (mrad) | d₀ (yd) | σ at 40 yd (in) |
   |---|---|---|---|---|
   | TC | 0.421 | 11.7 | 4.4 | 15.0 |
   | IC | 0.395 | 11.0 | 7.1 | 13.0 |
   | ¼ | 0.367 | 10.2 | 7.1 | 12.1 |
   | ½ | 0.363 | 10.1 | 8.9 | 11.3 |
   | ¾ | 0.355 | 9.9 | 10.9 | 10.3 |
   | Full | 0.363 | 10.1 | 13.7 | 9.5 |

   - With these, the table is reproduced to within 2.7 points.
   - The *growth rate* is about the same for every choke (about 10–12 mrad of σ per unit range).
     **Choke mainly delays the start of the spread.**
3. **Constant-angle model, fitted per choke to Table A** [derived]:

   | Choke | RMS error (points) | Maximum error (points) |
   |---|---|---|
   | TC | 1.1 | 2.0 |
   | IC | 2.3 | 3.7 |
   | ¼ | 2.4 | 3.9 |
   | ½ | 3.3 | 5.2 |
   | ¾ | 4.4 | 7.4 |
   | Full | 6.4 | 8.1 |
   | Overall | 3.7 | |

   - Anchored at 40 yd, Full predicts 88 % at 30 yd (table: 100) and 41 % at 60 yd (table: 32).
     **It is only adequate for open borings.**
4. **The forensic range-of-fire literature, at short range.**
   - **Linear fits.** For 12-gauge birdshot at 3.1–15.2 m, the square root of the pattern area (or
     the pattern radius) is linear in range, r > 0.99. The area itself is quadratic in range
     [Rowe and co-workers, blind-study abstract, unverified via search summary].
     - Heaney & Rowe, *J. Forensic Sci.* 28(2):433–436 (1983), first applied linear regression
       [AAFS 2013].
     - Arslan et al., *J. Forensic Sci.* 56(4):988–992 (2011). 12-gauge full choke and cylinder,
       16-gauge cylinder, #2 and #5 shot, 75–1000 cm. Each shotgun, choke and pellet type needs its
       own formula [OJP].
   - **Quadratic fits.** For lead birdshot at 5–15 m, a **second-order polynomial (parabolic)**
     fits the effective radius better than a line (R² 99.7–99.9 %) [AAFS 2017, Carcamo et al.]. The
     same study finds pellet size dominant and barrel length (33–72 cm) negligible.
   - **The "1 in per yd" rule.** Butler-Derose et al. found spread area against distance at
     R² 0.97 and "refuted the long-held notion that every inch of pellet spread is equal to a
     one-yard distance" [AAFS 2020].
   - I found no published coefficients (intercepts) that I could open.
5. **Mechanism.**
   - A DEM simulation shows the choke compressing the shot column sideways as it leaves the muzzle.
     This changes the pellet collisions and gives a tighter, more uniform pattern, with an optimum
     near a 1.02 mm tapered constriction [Davidson, Thomson & Birkbeck 2002].
   - Compton's UCL thesis modelled the cloud by adding a random force term to each pellet's
     equation of motion and found the width and length by ensemble averaging [Compton 1996, from
     the abstract via a search summary, unverified].
   - A random lateral force acting all along the flight makes the spread grow faster than linearly
     [derived, physical argument]. This is the hypothesis that fits the tables.

**Published fits:**
- None of the hunting literature publishes a formula.
- The forensic papers fit pattern radius or √area linearly, or as a quadratic, against range at
  1–15 m, for one gun and one load each.
- My own fits to the hunting tables are in §8.

---

## 3. Radial distribution within the pattern

### 3.1 Gaussian (bivariate normal) or not?

- **For the Gaussian:**
  - Oberfell & Thompson state that, over many shots, pellet distribution follows a normal curve.
    The Shotgun Insight author adds that the normal distribution "is a model that fits the observed
    distributions quite well", not a definition [Shotgun Insight; secondary account of O&T].
  - Walsh (AFTE, 2007), #4 lead at 10 and 15 m: horizontal and vertical coordinates are each about
    normal, "supporting the view that shotgun patterns follow a bivariate normal distribution".
    Pellets from the outside of the shot column land at the pattern's edge, and pellet deformation
    does not predict landing position [Walsh 2007].
  - Compton's UCL thesis (1996) found the horizontal and vertical distributions independent and
    best described as Gaussian [forum quote of the thesis, unverified].
  - Lowry's hit model uses a Gaussian ("bell-shaped") impact distribution [Lowry/Wakeman].
- **Against:**
  - A 1989 *Forensic Science International* paper ("Experimental evaluation of the distribution of
    the pellets in shotgun pellet patterns"; No. 2 birdshot and 00 buckshot, cylinder-bore
    12 gauge) reportedly found that the test patterns did not follow Gaussian distributions
    [unverified, search summary only].
  - Shotgun Insight: patterns from poor shell or choke design can fail to fit [Shotgun Insight].
- **The best quantitative check** is Gough Thomas's 20-inch data:

  | Boring | GT: share of 30-in pellets inside the 20-in circle | Gaussian prediction at the same 40-yd percentage [derived] |
  |---|---|---|
  | True cylinder | 46–51 % | 50.8 % |
  | Improved cylinder | 48–54 % | 53.0 % |
  | ½ choke | 52–57 % | 55.8 % |
  | Full | 57–62 % | 59.2 % |

  - Uniform density would give 44.4 % (area ratio).
  - The Gaussian prediction falls inside every Gough Thomas range, and it reproduces the rule that
    choke concentrates the centre "disproportionately" [GT via Spencer 2001 Table 9; derived].
  - An independent measurement agrees: "On average, 61 % of the shot inside any 30-inch circle
    ended up in the 20-inch core", varying ±9 %, with 28-gauge loads [Mule Deer, 24hourcampfire,
    his own patterns]. A Gaussian gives 61 % at p30 ≈ 74 % [derived].
  - Lowry's empirical numbers via SmallBoreShotguns: at 40 % overall, 20.3 % in the core and 19.7 %
    in the ring, a ratio of 0.51. The Gaussian gives 0.508 [unverified: the page could not be
    opened, figure from a search summary].

**What Oberfell & Thompson fitted.** I could not verify a specific function from them, beyond the
statement that the distribution is normal. **[unverified]**

Their rating methods [secondary sources]:

1. **The 30-in to 20-in ratio.** Pattern percentage together with the ratio of the 30-in count to
   the 20-in count, looked up in a table graded excellent / good / fair / poor. The thresholds were
   not found [Even Patterns page, luckyweasel.com].
2. **Patch counting.** The number of non-overlapping 5-in circles that can be placed in the 30-in
   circle without a hit, at 40 yd with No. 6 shot [Shotgun Insight].
   - O&T p. 47: an empty 30-in circle holds 27 such discs [Shotgun Insight].
   - Most patchiness is in the 20–30-in "annular ring", and there are almost no empty patches in
     the 20-in core [Mule Deer, 24hourcampfire].

### 3.2 Central density, and the 10-, 20- and 30-in circles

For a circular Gaussian [derived]:

| p30 at 40 yd | 20-in / 30-in | 10-in / 30-in | Centre density ÷ mean density in the 30-in circle |
|---|---|---|---|
| 40 % (TC) | 0.508 | 0.138 | 1.28 |
| 50 % (IC) | 0.530 | 0.148 | 1.39 |
| 55 % (¼) | 0.543 | 0.154 | |
| 60 % (½) | 0.558 | 0.161 | 1.53 |
| 65 % (¾) | 0.574 | 0.169 | |
| 70 % (F) | 0.592 | 0.179 | 1.72 |

- The zones (10-in disc, 10–20-in ring, 20–30-in ring) have areas in the ratio 1 : 3 : 5
  [Shotgun Insight]. A uniform pattern would put 11 %, 33 % and 56 % of the 30-in pellets in them.
- **Consequence for lethality.** The common practice of dividing the 30-in count by the 30-in
  area underestimates the density at the centre of a Full pattern by a factor of about 1.7, and
  overestimates it in the outer ring.

### 3.3 Flat-top and heavier-tail models

- **Flat-top designs.**
  - A Winchester patent aims to move part of the dense, "generally Gaussian" central core out
    toward the fringe for uniformity [US patent 8171849, design claim, unverified].
  - "Spreader" loads and wads give wider patterns: Wildfowl lists a spreader row of 23/37/51/66 in
    at 10–40 yd [Wildfowl].
  - I found no published radial-density data for flat-top or modern "FliteControl" type wads.
    **[gap]**
- **Heavy tails.** Over-choking ("spraying") and deformed soft lead give fliers [Hassel 2023;
  Roster]. I found no fitted tail parameter.
- Pellets from the outside of the shot column end up on the pattern's edge [Walsh 2007].

### 3.4 Clustering and patchiness: how random is a pattern?

**Gough Thomas's counts of empty, non-overlapping 5-in patches in a 30-in pattern** (12 gauge):

| Pellets in 30-in circle | 50 | 100 | 150 | 200 | 250 | 300 |
|---|---|---|---|---|---|---|
| GT: empty 5-in patches | 13–19 | 8–11 | 4–7 | 2–5 | 0–3 | 0–2 |
| Monte Carlo, Gaussian + Poisson, scaled ×27/24 [derived] | ≈16 | ≈8.3 | ≈3.8 | ≈1.8–2.1 | ≈0.4 | ≈0.1 |

Sources: [GT via Spencer 2001 Table 12]; Monte Carlo [derived].

- **Monte Carlo set-up.** Pellets were placed independently at random from a Gaussian with p30 of
  0.5 or 0.7, and empty discs were counted by greedy packing.
  - The greedy packing fits only 24 discs in an empty circle against O&T's 27, hence the ×27/24
    scaling.
  - Results with p30 = 0.5: 14.4, 7.4, 3.4, 1.6, 0.3, 0.1.
  - A uniform random pattern gives nearly the same numbers.
- **Reading.** At 50–100 pellets, real patterns sit inside Gough Thomas's ranges. At 150–250, the
  simulated counts fall at or below the bottom of the ranges.
  - So **real patterns are slightly patchier (clumped) than an independent Poisson scatter**, but
    not grossly so.
  - A Poisson model of pellet positions is a reasonable first model.
  - Part of the excess may come from my packing heuristic rather than real clumping. **Treat this
    conclusion as tentative.**
- **Metric versions of the patch test.**
  - Denmark: a 75-cm circle at 35 m, counting empty 12-cm (CD-sized) circles; a cartridge should
    rate at least "middel" for hunting [IFJ; Kanstrup 2010].
  - 12 cm at 35 m is the same angle as 5 in at 40 yd (12.2 cm) [derived].
- **The German Wannsee 16-field target.** In use since the 1930s and shot at 35 m: a quartered
  35-cm inner circle (4 fields) and a 75-cm outer circle divided into 12 equal fields [DJL].
  - A DEVA test of lead-free shot judged a load adequate for duck and pheasant at ≥ 6 hits in every
    field [Wild und Hund, unverified from a search summary].
- **Other evenness measures:**
  - Shotgun Insight weights the 10-in, 20-in and 30-in zones and tests shot-to-shot
    repeatability [Shotgun Insight].
  - shotguns.se uses the ratio of the 50-cm count to the 75-cm count and counts empty 125-mm holes
    [ShotgunsSE-eval].

### 3.5 Shot-to-shot variation

- BASC / Giblin & Compton, 10 shots per choke:
  - No. 7 lead: 10–15 points between the lowest and highest pattern at one choke.
  - BB lead: up to 45 points (23–68 % at ¼ choke).
  - "Five really is the minimum but 10 is much more reliable" [BASC 2008].
- Weldin (*American Rifleman*, December 1927): 4,000 patterns with one load and choke ranged from
  19 to 95 % [Wakeman-imprecision].
- Fifteen shots of 1¼ oz #2 steel through a factory Modified choke put 96 to 132 pellets in the
  30-in circle [Wakeman-imprecision].
- **Compared with sampling alone** [derived]:
  - Binomial sampling gives a standard deviation of only 2.8 points for 300 pellets at 60 %, and
    5.3 points for 85 pellets.
  - The observed variation needs a shot-to-shot variation of σ itself:

    | Shot | Coefficient of variation of σ (CV) |
    |---|---|
    | Small lead shot | about 5 % |
    | BB | about 15–20 % |

  - A 10 % CV in σ moves a 60 % pattern over roughly 53–68 % (±1 SD).

---

## 4. Effect of pellet size, material, velocity and buffering

| Effect | Direction and size | Source and confidence |
|---|---|---|
| **Pellet size** | Smaller pellets disperse more. At 5 m, patterns from lead #9 were about **1.7×** larger than from #0, and from steel #9 about **1.4×** larger than from #1, with identical cases, powder and wads. | Kerkhoff et al., *J. Forensic Sci.* 69(2):461–468 (2024), abstract |
| Pellet size, hunting range | Burrard: the percentages hold "for all sizes of guns and shot" [BASC 2008]. BB patterns vary far more shot to shot [BASC 2008]. Pellet size has "a dominant influence on pellet dispersion" at 5–15 m [AAFS 2017]. | No clean per-size coefficient at 35–40 m was found **[gap]** |
| **Steel and other hard shot** | Tighter by 1–2 choke steps, about +10 points at 40 yd (§1.4), with a plateau beyond ½ choke. Steel also strings about half as long as lead [MT Outdoors 2013; Shooting Times]. | Federal via [Field & Stream]; [Hassel 2023]; [BASC 2008] |
| **Hard (antimony) and plated lead** | Clay loads with more antimony "often pattern tighter" than soft-lead field loads of the same weight and size [Beretta]. One 6 % antimony load at 1300 fps may out-pattern a 2 % antimony load at 1145 fps [forum, unverified]. Nickel plating plus buffer gave about +12 % pattern density for #4–#5 [forum, unverified]. | Mostly anecdotal |
| **Velocity** | Faster soft lead deforms more and patterns worse. Eley says its high-velocity cartridges "may deliver inferior patterns" [Spencer 2001]. Reported thresholds for lead: "over 1350 fps starts blowing patterns", best below 1260 fps (Muller Chokes), no significant change from 1100 to 1235 fps [forums, unverified]. Federal turkey loads: 1¾ oz at 1300 fps gave 70.2 %, 2 oz at 1150 fps 61.4% [Field & Stream]. | Weak. The Federal comparison is confounded by payload and runs the other way. Hard shot and good cups permit high velocity "with no pattern penalty" [forum, unverified]. |
| **Buffering** | Buffered Federal 00-buck at 1100 fps: 31.65-in pattern diameter at 40 yd. Unbuffered handload at 1210 fps: 36.89 in [Field & Stream]. Forum estimates of +10–15 % for large shot, mostly beyond 50 yd [unverified]. | Moderate (one measured pair) |
| **Wad** | A plastic wad gives about ½–1 step more choke than a fibre wad [ShotgunsSE-choke]. | Secondary |
| **Barrel length** | 56/66/71/81 cm gave about 58/59/62/60 % at 30 m [all4shooters]. Barrel length is negligible at 5–15 m [AAFS 2017]. A 20-cm (sawn-off) barrel spreads more beyond 1 m [Çelikel et al. 2012]. | |
| **Gauge** | Same percentage for the same choke regardless of gauge [Spencer 2001]. Federal dove loads: 12 ga 50.7 % against 20 ga 39.6 % at 40 yd [Field & Stream], choke not stated. 12 gauge may pattern tighter, "measurable but still insignificant compared to payload" [Jones, *Sporting Shotgun Performance*, via Chuck Hawks review]. | |
| **Choke beyond Full** | The percentage *drops* from 40 thou to 50 thou [BASC 2008]. DEM optimum near 1.02 mm [Davidson 2002]. The maximum in a 75-cm circle "hardly goes above 80 %" [ShotgunsSE-choke, citing BASC]. | |

---

## 5. Standards and test procedures

- **British / US.** Share of the charge in a 30-in (76.2 cm) circle at 40 yd (36.58 m). The circle
  is drawn round the densest part of the pattern, and the count is divided by the total pellets in
  the load [Burrard via BASC 2008; Lowry/Wakeman].
  - Lowry defines pattern percentage as the 30-in count divided by the total, × 100, at 40 yd by
    default.
- **Continental Europe and Scandinavia.** Share of the *lead* charge in a 75-cm circle at 35 m.
  - Sweden [Hassel 2023; jägarexamen online].
  - Denmark [IFJ; Kanstrup 2010]. Danish guns are normally regulated so that the line of sight and
    the centre of the swarm meet at 35 m [IFJ].
  - Germany, on the Wannsee target [DJL].
  - RWS, via a search summary: "the number of hits … in a 75 cm test circle at a testing distance
    of 35 m is put into relation to the total number of shot pellets" [unverified].
  - **I found no source using a 70-cm circle.** [gap]
- **CIP.** I found no CIP rule on pattern percentage. CIP regulates pressure, velocity, momentum,
  shot size and hardness, and proof (§1.4). **[unverified that no CIP pattern test exists.]**
- **Number of shots.** At least 5, better 10 [BASC 2008]; 3–5 per pattern [ShotgunsSE-eval];
  at least 3, averaged [SSAA, unverified].
- **Converting 30 in / 40 yd to 75 cm / 35 m.**
  - The 75-cm / 35-m circle subtends a 2.9 % larger angle: 10.71 mrad of radius against 10.42.
  - So it reads about **+2 points** higher for the same gun with constant-angle spread, and
    **+1.5 to +4.5 points** with the range-dependent model of §8 (TC 40 → 41.5, ½ 60 → 62.8,
    F 70 → 73–75) [derived].
  - Eley's table, at 35 m but in the 30-in circle, gives 43 / 53 / 58 / 64 / 70 / 74 for TC to
    Full, against 40 / 50 / 55 / 60 / 65 / 70 at 40 yd [Spencer 2001 Table 8].
  - The Nordic and German tables simply re-use 55 / 60 / 65 / 70 for ¼ / ½ / ¾ / Full
    [Hassel 2023; DJL].
  - So in practice **a "Nordic percentage" is numerically the same label, about 2–4 points
    generous**, which is within shot-to-shot noise.

---

## 6. Shot string

**Measured lengths:**

- **H.P. White Laboratory, at 40 yd** [Shooting Times]:

  | Load | String length | Velocity at 40 yd |
  |---|---|---|
  | #4 Hevi-Shot | 11–12 ft | 920 fps |
  | #4 lead | **11–12 ft** | 803 fps |
  | #4 steel | **8–9 ft** | 699 fps |
  | #2 high-velocity steel | **7–8 ft** | 845 fps |

- **Roster.** Steel and other hard nontoxic shot string "about half as long as the sloppy lead
  string". At 30–40 yd the nontoxic string is about 7 ft long and 4 ft across [MT Outdoors 2013].
- **Federal (Carlson & Moser), measured at 20 yd** [Field & Stream]:

  | Load | String length at 20 yd |
  |---|---|
  | 12 ga 3-in, 1450 fps | 42 in |
  | 12 ga 3½-in, 1500 fps | 49 in |
  | .410, 1135 fps | 66 in |
  | 20 ga, 1210 fps | 46 in |
  | 12 ga dove, 1235 fps | 55 in |
  | 20 ga dove, 1165 fps | 57 in |
  | Steel #3, 1600 fps | 61 in |
  | Lead #6, 1500 fps | 55 in |

  - The .410's 66 in would be "about 10 feet long at 40 yards" (extrapolated).
- **Kanstrup.** Over 2 m long at 25 m [Kanstrup 2010].
- **Spencer.** "Approximately half the shot is in the front third" of the string [Spencer 2001].
- **Definition.** The US convention is the shortest length that holds 90 % of the pellets at 40 yd
  [Lowry/Wakeman]. In a hard-ball test, 80 % strings averaged about 50 in at 40 yd [Lowry,
  unverified from a search summary].

**Time to pass** [derived]:

| String | Pellet speed | Time | 40-mph (17.9 m/s) bird moves |
|---|---|---|---|
| 3.5 m lead | 245 m/s | ≈ **14 ms** | 0.26 m |
| 2.6 m steel | 213 m/s | ≈ **12 ms** | 0.22 m |
| 2.3 m high-velocity steel | 258 m/s | ≈ **9 ms** | 0.16 m |

The "teal against a 7-ft #2 steel string" example in Shooting Times gives about 0.5 ft, consistent
with this.

**Effect on moving (crossing) targets:**

- **Brister.**
  - He shot a moving pattern board 20–30 ft long, towed past at about 40 mph.
  - A 10-gauge 1⅝ oz load at 60 yd gave **74 %** on a still target and **54 %** on the moving one
    [Outdoor Life].
  - Brister's 1976 lead waterfowl magnums lost about 30 % [Field & Stream].
- **Modern loads.** Pattern density lost by a 40-mph bird at 40 yd is about **5 %** [Field & Stream].
- **Lowry's model.**
  - Load: 12 ga, 1¼ oz #2 steel, 1365 fps. Target: 70 ft/s at 40 yd.
  - With the lead correct, average hits stay **9.94** for strings of 0–20 ft.
  - With the lead 10 in wrong, hits fall from 3.43 (0-ft string) to 3.26 (10 ft) to 2.65 (20 ft)
    [Lowry/Wakeman].
  - So the string matters mainly when the lead is wrong.
- **Burrard.** His 1920s tests shot at a steel plate on a truck at about 40 mph. He concluded the
  importance of string length "was probably exaggerated" [Shooting Times].
- **Roster's teaching.** It is worse to shoot behind than ahead, because shooting ahead still uses
  the middle or end of the string [MT Outdoors 2013].

---

## 7. Effective pattern and lethality-density criteria

### 7.1 Published criteria

**Pellet hits needed:**

| Source | Requirement |
|---|---|
| **Eley** [Spencer 2001] | Small birds (snipe): 2 pellets of ≥ 0.5 ft·lb (0.68 J). Medium (grouse): 3 of ≥ 0.85 ft·lb (1.15 J). Large (goose): 4 of ≥ 1.5 ft·lb (2.03 J). |
| **Burrard and Åkerman** [ShotgunsSE-cartridge] | Hits in the vital area: game < 0.5 kg needs 3, 0.5–1 kg needs 4, > 1 kg needs 5. Pellet energy ≥ 1.08 J (0.11 kgm) for birds and small game, ≥ 2.16 J (0.22 kgm) for larger. Vital areas are below. |
| **Roster** [MT Outdoors 2013] | From thousands of X-rays of ducks and geese: it is not trauma but **1–2 pellets** reaching brain, heart, lungs or spine. |

**Vital areas** [ShotgunsSE-cartridge; Burrard and Åkerman]:

| Game | Vital area |
|---|---|
| Duck (1.0 kg) | 2.0 dm² |
| Pheasant (1.2 kg) | 2.4 dm² |
| Pigeon / partridge (0.4 kg) | 0.8 dm² |
| Woodcock (0.3 kg) | 0.6 dm² |

Densities needed in the 75-cm circle at 35 m (44 dm²): pigeon 3.75/dm², about 165 in the circle;
duck about 88; pheasant about 92. The British rule of thumb is 100 in the 30-in circle for pheasant.

**Gough Thomas / Eley minimum pattern in the 30-in circle at 40 yd** [Spencer 2001 Table 15]:

| Game | Shot | Pellets |
|---|---|---|
| Snipe | 8 | 270 |
| Woodcock | 7 | 150 |
| Squirrel | 7 | 180 |
| Partridge | 6 | 130 |
| Grouse | 6 | 130 |
| Pheasant | 6 | 120 |
| Pigeon | 6 | 130 |
| Rabbit | 5 | 120 |
| Teal | 6/7 | 150 |
| Duck | 4 | 80 |
| Hare | 4 | 100 |
| Goose | 3 | 70 |

The pheasant figure of 120 is the most often repeated "Burrard" number [forums, unverified].

**Roster 2012 Nontoxic Shot Lethality Table** [Roster 2012]:

- **How it was built.** From 15 US steel-against-lead waterfowl shooting tests run 1968–1982, one
  pheasant test, and ammunition-company data. Velocity 1225–1450 fps. Steel 7.86 g/cc,
  90–95 DPH; HEVI-Shot 12.0 g/cc.
- **Column headings did not survive text extraction.** I read them as: range (yd), shot, charge
  (oz), minimum vital hits, minimum pellets in the 30-in circle, choke. The minimum-pellet column
  is the count at the range of the shot. **[unverified headings]**

| Quarry and situation | Range (yd) | Steel shot | Pellets in 30-in circle | Choke |
|---|---|---|---|---|
| Large geese, long range | 50–65 | BBB–T, 1¼ oz | 50–55 | IM |
| Large geese, over decoys | 35–50 | | 50–55 | IC or M |
| Medium / small geese | 35–65 | | 60–65 | |
| Large ducks | 20–65 | | 85–90 | |
| Medium ducks (wigeon, scaup) | 20–45 | 1 oz | 115–120 | |
| Small ducks (teal) | 20–45 | | 135–145 | M (20–35 yd), F (35–45 yd) |
| Pheasant | 20–50 | #3–#2, 1 oz | 90–95 | IC (20–30 yd), M (30–50 yd) |
| Turkey, head and neck | 20–40 | | 210–230 | F or XF |

- The vital-hit column reads 1–2 for waterfowl, 2–3 for pheasant and 3–4 for turkey.
- **CONSEP** put the average wounding (unretrieved) rate for ducks and geese at 25 % [MT Outdoors
  2013].

**Swedish hunting exam** (since 1985) [SJF-exam; Kanstrup 2010]:

- Running targets at 20 m, one going each way. Each shot must put **≥ 8 pellets in a 9-cm** hit
  area on the hare or fox figure, or **≥ 28 in an 18-cm** area on the roe-deer figure.
- That is 0.126 and 0.110 pellets/cm² [derived].
- Clays: 4 of 6 going-away targets.

**Danish practice** [Kanstrup 2010]:

- **Ethical maximum ranges:** roe deer 20 m, geese and fox 25 m, others 30 m.
  - These are set by *practical* effectiveness, including hunters' aim, not by ballistics.
  - Good shots mostly miss the centre by < 30 cm. Pattern spread covers at least a 50-cm radius at
    normal ranges. Poor shots miss by up to 130 cm.
- **Rules:** maximum shot size 4 mm. Minimum muzzle velocity 375 m/s for shot denser than 9 g/cc
  and 400 m/s for lighter shot. Shot below 7 g/cc is banned.
- **Wounding:** pellet-carrier frequency is 10–20 % for several species (Noer et al. 1996, 2006).
- **Hit model:** Kanstrup's Fig. 3 plots the probability that only non-vital parts are hit against
  the number of pellets striking. It shows a model curve and Bellrose's (1953) controlled shooting
  of mallards. The curve values are graphic only.
- Hassel reports that Svenska Jägareförbundet now recommends a 20 m maximum for some species
  [Hassel 2023, unverified at the primary].

### 7.2 How to compute them

- **Classic method (Burrard, Gough Thomas).** Mean density = pellets in the 30-in circle ÷ the
  circle's area (707 in² = 45.6 dm²). Expected hits = density × vital area. Require expected hits
  ≥ N, with each pellet's energy ≥ the threshold at that range.
  - For example, 120 pellets in the 30-in circle gives 0.0263 pellets/cm². On a 2.4-dm² pheasant
    that is λ ≈ 6.3 hits [derived].
- **Poisson method** [derived]:
  - For a target at offset r from the pattern centre, the expected hits are
    λ = N_total · ∫_A φ(x, y; σ) dA ≈ N_total · A · exp(−r²/2σ²) / (2πσ²).
  - P(≥ k hits) = 1 − Σ_{j<k} e^{−λ} λ^j / j!

  | λ | P(≥ 1) | P(≥ 2) | P(≥ 3) | P(≥ 4) | P(≥ 5) |
  |---|---|---|---|---|---|
  | 3 | 0.95 | 0.80 | 0.58 | 0.35 | 0.19 |
  | 4 | 0.98 | 0.91 | 0.76 | 0.57 | 0.37 |
  | 5 | 0.99 | 0.96 | 0.88 | 0.74 | 0.56 |
  | 6 | 1.00 | 0.98 | 0.94 | 0.85 | 0.72 |

  - So "expected hits = required hits" gives only about a 55–60 % chance of reaching the required
    count. Published minimum counts are not 90 % criteria.
- **Patch method (O&T, Danish CD).**
  - P(an area A is empty) = exp(−λ_A). With λ_A = ρ·A, this links the 5-in patch test to density.
  - A 5-in disc is 19.6 in² = 127 cm², described as the vital area of many gamebirds
    [24hourcampfire; Shotgun Insight].
- **Aim error.**
  - Average P(≥ k) over the aim-error distribution, not over the mean density.
  - Kanstrup's 30-cm against 130-cm miss distances [Kanstrup 2010] suggest an aim-error σ of
    roughly 15–20 cm for good shots and 50+ cm for poor shots. This is my rough reading.
    **[derived, rough]**
  - The BASC / Leeming model concluded that the share of mallard *hit* falls as choke increases,
    because tighter patterns need more accurate aim [BASC 2008].

---

## 8. Recommendations for the engine

### 8.1 Pattern model

- **Distribution.** A circular bivariate normal about the point of impact, so the radius is
  Rayleigh. Keep it; it is well supported (§3.1–3.2).
- **Spread against range.** Replace σ = k·d with a model that starts slower and accelerates, and
  more so for tighter chokes:

  ```
  x      = d / d40                      d40 = 40 yd = 36.576 m
  sigma40 = R30 / sqrt(-2 ln(1 - p40))  R30 = 0.381 m (15 in), p40 = nominal fraction in 30 in at 40 yd
  q      = 0.011 * max(0, P40 - 40)     P40 in percent (q = 0 for cylinder, 0.33 for Full)
  sigma(d) = sigma40 * x * (1 + q*(x - 1))
  ```

  - This is a quadratic through the origin: σ = k₁d + k₂d², with k₁ = σ₄₀(1 − q)/d₄₀ and
    k₂ = σ₄₀q/d₄₀².
  - It always reproduces the nominal 40-yd percentage exactly. It needs no table per choke, only
    p40.
- **Parameters per choke** [derived, `model_final.out`]:

  | Choke (lead) | p40 | σ₄₀ (cm) | q | σ at 20 / 30 / 35 / 40 / 50 m (cm) |
  |---|---|---|---|---|
  | Cylinder | 40 | 37.7 | 0.00 | 20.6 / 30.9 / 36.1 / 41.2 / 51.5 |
  | Skeet | 45 | 34.8 | 0.055 | (interpolate) |
  | Improved Cylinder | 50 | 32.4 | 0.11 | 16.8 / 26.0 / 30.8 / 35.8 / 46.0 |
  | Light Mod / ¼ (UK, Nordic) | 55 | 30.1 | 0.165 | 15.3 / 24.0 / 28.6 / 33.5 / 43.7 |
  | Modified / ½ | 60 | 28.1 | 0.22 | 13.9 / 22.2 / 26.7 / 31.4 / 41.6 |
  | Improved Modified / ¾ | 65 | 26.3 | 0.275 | 12.6 / 20.5 / 24.9 / 29.5 / 39.6 |
  | Full / 1/1 | 70 | 24.6 | 0.33 | 11.4 / 18.9 / 23.2 / 27.7 / 37.6 |
  | Extra Full | 75 | 22.9 | 0.385 | extrapolated, **unvalidated** |

- **Fit against Gough Thomas Table A** (30–60 yd):
  - This model: RMS error **1.1 points**, maximum **2.6**.
  - Constant-angle model: RMS 3.7, maximum 8.1.
  - Free two-parameter fits (σ = b(d − d₀), or k₁d + k₂d²) reach RMS 1.05–1.1, so the
    one-parameter q(p40) costs almost nothing.
- **Fit against Table C** (spread taken as the 85 % circle diameter, 2σ√(2 ln(1/0.15)) = 3.90σ):
  - Within **3.1 in** at 20–40 yd.
  - The model is 2–6 in *smaller* than the table at 10–15 yd, where the table's "bulk" is the total
    spread.
  - Optionally, for d < 15 m, display a total-spread circle (about 99 %, 6.07σ) instead.
  - Below about 25 m the model is also slightly too tight for open borings. At 20–25 m it is up to
    7 points above Eley's metric table for TC, IC and ¼, and 7 above Beretta's Cylinder figure at
    20 yd.
  - If that matters, a near-muzzle floor σ ≥ k_f·d would close most of the gap: k_f ≈ 11.4 mrad
    for TC and ≈ 10 mrad for IC and ¼. These values are what Eley's 20-m percentages imply
    [derived]. The floor is untested beyond that.
- **Valid range:** about 15–55 m. Beyond 60 yd it is extrapolation. For p40 > 75 it is unvalidated.
- **Alternative, for cylinder only.** If simplicity matters more, keep constant angle for Cylinder
  and Skeet, whose error is ≤ 2 points. It is not acceptable for Modified and tighter.

### 8.2 Choke name to parameters

1. **Name → nominal p40 (lead), by percentage, not constriction.** Using §1.1:

   | Choke name or mark | p40 (%) |
   |---|---|
   | Cylinder (C, CYL, ★★★★★, IIIII) | 40 |
   | Skeet (SK, 1/8) | 45 |
   | Improved Cylinder (IC) | 50 (US) |
   | ¼, ★★★★, IIII, ++ | 55 |
   | Light Modified (LM) | 55 |
   | 3/8 | 57 |
   | Modified (M, ½, ★★★, III, +-) | 60 |
   | 5/8 | 63 |
   | Improved Modified (IM, ¾, ★★, II, ++-) | 65 |
   | Light Full (LF), 7/8 | 67 |
   | Full (F, 1/1, ★, I, +) | 70 |
   | Extra Full (XF, 5/4) | 75 |
   | Spreader | about 30–35 (σ larger than Cylinder; no table) **[unverified]** |

   - European usage equates IC with ¼ and ★★★★ (55 %) [Beretta; Hassel 2023]. Let the user
     override p40, and show the constriction only as information.
   - For a numeric constriction from the user, interpolate p40 on the US/Beretta scale for 12 gauge:

     | Constriction | 0 mm | 0.13 | 0.25 | 0.38 | 0.50 | 0.64 | 0.76 | 0.89 |
     |---|---|---|---|---|---|---|---|---|
     | p40 (%) | 40 | 45 | 50 | 55 | 60 | 65 | 67 | 70 |

     - Above 1.0 mm, flat or *falling* [BASC 2008].
     - 20 gauge: use the same percentages per *name*; the constrictions are about 10–20 % smaller (§1.2).
2. **Percentage measured in 75 cm at 35 m** (Nordic). Solve for σ at 35 m with R = 0.375 m. Use the
   model to find p40; roughly p40 ≈ p75/35 − 2 to 4 points. Or accept the label as equal to p40
   (§5).
3. **Steel and hard shot** (my synthesis of §1.4; **the size of the shift is approximate**):
   - p40,steel = min(p40,lead + 10, 72) for chokes up to Modified. Use 72–75 for IM and Full, which
     gain nothing extra.
   - Warn when steel is used with a choke tighter than ½ / M. CIP limits HP steel and shot above
     4 mm to under 0.5 mm; Beretta recommends no tighter than Modified.
   - Halve the string length (§6).
4. **Pellet size and velocity.** No default correction; the evidence at hunting range is weak (§4).
   If a knob is wanted, allow ±5 points for large or soft shot against small or hard shot, and
   document it as an assumption.
5. **Shot-to-shot variation** (for simulation or hit-probability bands):
   - Draw σ per shot from a lognormal with CV ≈ **5–8 %** for small lead shot and **15–20 %** for
     BB-size shot (§3.5).
   - Draw pellet positions independently, which is Poisson. This reproduces the spread of
     single-shot percentages in BASC 2008, and patch counts close to Gough Thomas's (§3.4).
6. **Shot string** (optional):
   - Add an along-track smear for crossing targets, σ_track² = σ² + (v_target · σ_t)², where
     σ_t = (L₉₀/3.29)/v_pellet.
   - L₉₀ at 40 yd: about 3.5 m for lead, about 2.4 m for steel.
   - The expected effect is about 5 % loss of density at 40 yd and 40 mph [Field & Stream].
7. **Lethality.** Compute λ by integrating the Gaussian density over the vital area at the aim
   point, counting only pellets whose remaining energy is at least the threshold. Report P(≥ k) by
   Poisson, using the hits k and energy E_min of §7.1 (Eley or Burrard/Åkerman).

### 8.3 Tables to test against, with tolerances

| Test | Reference | Tolerance | Current constant-angle model | Proposed model |
|---|---|---|---|---|
| % in 30-in circle, 30–60 yd, all 6 borings | Gough Thomas Table A | ±3 points; for the 100 % entries require ≥ 92 % (model 92.1 for Full at 30 yd) | fails (up to 8.1) | passes (max 2.6) |
| Same in metres, 30–55 m | Eley Table B | ±3.5 points (it is a conversion of A; rounding) | | passes (max 3.2) [derived] |
| Same in metres, 20–25 m | Eley Table B | ±8 points | | passes. Eley extends below Table A's 30-yd start; the model is up to 7 points *higher* than Eley for TC, IC and ¼ at 20–25 m. This is the same short-range under-spread as the Table C row below. |
| US 20/30/40 yd, 6 chokes | Beretta / GMK chart (names mapped as in §1.1) | ±4 points; 100 % entries require ≥ 92 % | | Cyl at 20 yd: model 87 against 80. Full at 30 yd: 92 against 100. Flag both as known source disagreements. |
| Spread diameter, 20–40 yd | Gough Thomas Table C (85 % circle) | ±4 in | | passes (max 3.1) |
| Spread diameter, 20–35 m | Eley Table D | ±10 cm | | passes (max 7.7 cm, model smaller) |
| Spread diameter, 10–15 yd | Table C | model may be smaller by up to 6 in; test only "model ≤ table" | | |
| 20-in / 30-in share at 40 yd | Gough Thomas Table 9 | inside the published range ±1 point (TC 46–51, IC 48–54, ½ 52–57, F 57–62) | | passes |
| Empty 5-in patches at 40 yd, 50–300 pellets | Gough Thomas Table 12 | Monte Carlo mean within the range, or at most 1 below its lower limit | | 50/100 pass; 150–250 at or below the lower limit (real patterns are slightly patchier) |
| 40-yd percentage for 0/10/20/30/40 thou | BASC / Giblin & Compton (No. 7 lead: cylinder 31–41 %, half 54–68 %) | nominal p40 inside the measured range | | sanity check only |
| Choke shift rule | Gough Thomas | full at 55 yd ≈ cylinder at 40 yd, ±3 points | | |

Regression guard: the predicted percentage must fall monotonically with range for every choke, and
rise monotonically with p40 at every range.

---

## 9. Gaps and open questions

- The original Oberfell & Thompson figures were not reachable: their rating thresholds, their
  radial fit, and their "effective pattern" definition. Only secondary accounts were found.
- I found no Journée primary data. Whether the British tables (A, C) descend from Journée or from
  Burrard's own firings is **unverified**.
- I found no fitted forensic coefficients (intercepts and slopes) that I could open.
- I found no controlled study isolating velocity or antimony content at hunting ranges.
- I found no radial-density data for modern "flat-top" wads.
- The Roster column headings are inferred.
- The Swedish recommended maximum ranges (Jägareförbundet) were not verified at the primary source.

---

## 10. Sources

Primary or near-primary documents I opened:

- **[Spencer 2001]** Jonathan Spencer, "Shotgun barrels and ammunition: specifications &
  performance", December 2001. PDF hosted by NJFF Bodø. Tables credited inside to Gough Thomas's
  books (*Gun Book*, *Second Gun Book*, *Shotgun Facts & Fancies*, *Shotguns and Cartridges for
  Game and Clays*), to the Eley Shooter's Diary 1992, and to Purdey 1936.
  https://www.njff.no/nordland/bodo/jegerproven/_/attachment/inline/1abfbf54-463a-4fda-9b1e-3133c57f37c2:b559d36b37b523360ea2275f68bf52cefc19d7d8/Hagleammunisjon,%20antall%20hagl,%20choker,%20treffbilder%20og%20forranhold%20.pdf
- **[BASC 2008]** BASC information sheet "Shotgun chokes and patterns", 27 Oct 2008. Quotes Burrard
  1948, Gough Thomas 1975, Brindle 1984, Giblin & Compton (UCL BRL) and the Leeming model.
  https://www.kentwildfowlers.co.uk/pdfs/shotgun_chokes_and_patterns.pdf
- **[Beretta]** Beretta "Chokes constriction chart" and the OCHP lead chart, distributed by GMK.
  https://www.gmk.co.uk/docs/choke-constriction-chart.pdf
- **[Hassel 2023]** Tobias Hassel, *Alternativhagel för jakt* (Hagelkompendium), 2023.
  https://www.shotguns.se/Hagelkompendium.pdf
- **[ShotgunsSE-choke]** http://www.shotguns.se/html/choke.html
- **[ShotgunsSE-cartridge]** http://www.shotguns.se/html/cartridge.html. Cites Åkerman, *Jägarens
  vapen*, and Burrard, *The Modern Shotgun* III.
- **[ShotgunsSE-eval]** https://shotguns.se/html/evaluation.html
- **[DJL]** Deutsches Jagd-Lexikon, "Choke". https://www.deutsches-jagd-lexikon.de/choke/
- **[Roster 2012]** Tom Roster, *2012 Nontoxic Shot Lethality Table*, hosted by Texas Parks &
  Wildlife. https://tpwd.texas.gov/huntwild/wild/game_management/media/roster_nontoxic_shot.pdf
- **[MT Outdoors 2013]** T. Dickson, "Too many misses" (Tom Roster / CONSEP), *Montana Outdoors*,
  Sep–Oct 2013. https://fwp.mt.gov/binaries/content/assets/fwp/montana-outdoors/2013/shotgunclinic.pdf
- **[Kanstrup 2010]** N. Kanstrup & L. Thune Andersen, "Haglskydeprøve", Dansk Jagtakademi,
  4 Nov 2010.
  https://institutforjagt.dk/wp-content/uploads/2020/03/101104_Dansk_Jagtakademi_Udredning_om_haglskydeproeve.pdf
- **[IFJ]** Institut for Jagt, Jagttegnsbogen, "Oberfell og Thompson metode".
  https://institutforjagt.dk/jagttegnsbogen/kapitel-2-jagtvaben-og-ammunition/oberfell-og-thompson-metode-til-indskydning-af-haglgevaer/
- **[SJF-exam]** Svenska Jägareförbundet, "Swedish hunting examination".
  https://jagareforbundet.se/en/utbildning/jagarexamen/the-hunters-exam-huntsman-examination/swedish-hunting-examination
- **[Field & Stream]** "The Truth About Shotgun Ammo" (Federal range; engineers Carlson & Moser).
  https://fieldandstream.com/stories/guns/ammo/truth-about-shogun-ammo
- **[Shooting Times]** S. E. Mayer, "Long Shot String vs. Short Shot String" (H.P. White data).
  https://www.shootingtimes.com/editorial/long-shot-string-vs-short-shot-string/100155
- **[Outdoor Life]** "Three Things We Know About Shotgunning Thanks to … Bob Brister".
  https://www.outdoorlife.com/guns/bob-brister-shotguns/
- **[Lowry/Wakeman]** "The Essential Ed Lowry on Shotshell Velocity and Patterns".
  https://www.randywakeman.com/TheEssentialEdLowry.htm
- **[Wakeman-imprecision]** https://randywakeman.com/ShotgunPatternsandtheirSpectacularImprecision.htm
- **[Shotgun Insight]** https://shotgun-insight.com/ShotgunInsightOnline/SGHelpAndAbout/Help.html
  (secondary account of Oberfell & Thompson 1957)
- **[24hourcampfire]** "Patterning pheasant loads" (posts by "Mule Deer").
  https://www.24hourcampfire.com/ubbthreads/ubbthreads.php/topics/9004764/patterning-pheasant-loads
- **[Walsh 2007]** K. Walsh, "Shotgun Pellet Patterns: Pellet Distribution and the Effect of Pellet
  Deformation", AFTE.
  https://afte.org/dvd/shotgun-pellet-patterns-pellet-distribution-and-the-effect-of-pellet-deformation/
- **[AAFS 2013]** K. B. Morris et al., A25 "Comparative Study of Shotgun Spread Patterns". Cites
  Heaney & Rowe 1983 and Arslan et al. 2011.
  https://www.aafs.org/sites/default/files/media/documents/AAFS-2013-A25.pdf
- **[AAFS 2017]** D. Carcamo et al., D13 "Multivariate Analysis Applied to Sawed-Off Shotgun Pellet
  Dispersion". https://www.aafs.org/sites/default/files/media/documents/AAFS-2017-D13.pdf
- **[AAFS 2020]** K. Butler-Derose et al., E15 "An Evaluative Look at Shotguns: Pellet Spread".
  https://www.aafs.org/sites/default/files/media/documents/AAFS-2020-E15.pdf
- **[OJP]** Arslan, Kar, Uner & Cetin, *J. Forensic Sci.* 56(4):988–992 (2011), abstract.
  https://www.ojp.gov/ncjrs/virtual-library/abstracts/firing-distance-estimates-pellet-dispersion-shotgun-various-chokes
- **[Kerkhoff 2024]** Kerkhoff, Maitimu, Pater & de Jong, "The relationship between pellet size and
  shotgun dispersion patterns", *J. Forensic Sci.* 69(2):461–468, doi:10.1111/1556-4029.15427.
  Abstract via search summary.
- **[Davidson 2002]** Davidson, Thomson & Birkbeck, "Computational modelling of the shot pattern of
  a sporting shotgun", *Sports Engineering* 5(1):33–42, doi:10.1046/j.1460-2687.2002.00089.x.
  Abstract via search summary; record at https://eprints.gla.ac.uk/726
- **[Çelikel et al. 2012]** "The Effect of Barrel Length on Pellet Dispersion…", *Adli Tıp Dergisi*
  26(2):62–66. https://dergipark.org.tr/en/pub/adlitip/article/1902722
- **[CIP via steel-shot.com]** BASC summary of the CIP steel-shot rules.
  https://www.steel-shot.com/cip-saami-regulations-l5-19
- **[ShootingUK]** "Steel shot vs lead shot – the hard facts".
  https://www.shootinguk.co.uk/gear/steel-shot-vs-lead-shot-the-hard-facts-about-steel-shot/
- **[WFC]** WaterfowlChoke, "Choke Tube Diameter and Constriction Table" (Jan 2024).
  https://waterfowlchoke.com/choke-tube-diameter-and-constriction-table/
- **[guncustomizing]** "Technical Page" (unattributed tables). http://www.guncustomizing.com/tech.htm
- **[Wildfowl]** L. Simpson, "Choosing the Right Choke for Your Shotgun", *Wildfowl*, 2010.
  https://www.wildfowlmag.com/editorial/tips_strategies_shotgun_062304/281145
- **[ShotgunLife]**
  https://www.shotgunlife.com/briefs/the-effective-range-and-efficiency-of-different-choke-constrictions.html
- **[all4shooters]** "Großer Flinten-Praxistest".
  https://www.all4shooters.com/de/jagd/munition/praxistest-flinte-zusammenhang-von-lauflaenge-und-choke-groesse/
- **[Feu-Follet]** Bookseller listing for Journée, *Tir des fusils de chasse* (1920).
  https://edition-originale.com/en/works/literature-1/illustrated-books-18/journee-generale-tir-des-fusils-de-chasse-1920-36164

Sources known only from search summaries (**[unverified]**):

- North American Outdoors (Jack O'Connor's percentage ranges).
  https://northamericanoutdoors.org/understanding-the-complex-world-of-chokes/
- SmallBoreShotguns, "Patterns & Probabilities" (Lowry's empirical core and ring numbers). The page
  could not be opened because of a certificate error.
- *Forensic Science International* 1989, "Experimental evaluation of the distribution of the
  pellets in shotgun pellet patterns".
  https://www.sciencedirect.com/science/article/abs/pii/0379073889901989
- Compton D. J., *An experimental and theoretical investigation of shot cloud ballistics*, PhD,
  UCL 1996. https://discovery.ucl.ac.uk/id/eprint/1382490/ (returned 403).
- Wild und Hund, DEVA test of lead-free shot. https://wildundhund.de/bleifrei-schrotmunition-9113/
  (returned 403).
- Forum threads on velocity, buffering and antimony: shotgunworld.com, trapshooters.com,
  ultimatepheasanthunting.com.
- Chuck Hawks's review of A. Jones, *Sporting Shotgun Performance*.
  https://chuckhawks.com/shotgun_performance_jones.htm
