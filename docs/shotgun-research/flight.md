# Exterior ballistics of shotgun pellets: research for the point-mass sphere engine

Researched 2026-11-21. Purpose: improve `crates/ballistics-core/src/pellets.rs`, which flies one sphere
with a `Cd(Mach)` table and has not yet been checked against published pellet tables.

**How to read the sources**

- **[P]** is a primary or maker source I read myself: a report, standard, maker page or the author's own article.
- **[S]** is a secondary source I read: a magazine, retailer, review or reproduction.
- **[C]** is my own calculation. I wrote a small Python point-mass sphere solver (RK4 on distance, flat fire)
  to check the tables against each other. The script is described at the end. These numbers are not
  published data.
- **UNVERIFIED** marks something I could not confirm in a primary or reputable source, or a source I could not open.

Units: 1 ft = 0.3048 m; 1 yd = 0.9144 m; 1 ft-lb = 1.3558 J.
**Army Standard Metro** is 59 °F, 29.53 inHg, 78 % RH, ρ ≈ 1.2034 kg/m³. **ICAO** is 15 °C,
1013.25 hPa, ρ = 1.225 kg/m³.

---

## 0. Key numbers in one place

| Quantity | Value | Source |
|---|---|---|
| Pellet Reynolds number, 2–4 mm at 150–450 m/s, sea level | ≈ 2·10⁴ – 1.2·10⁵ | [C] Re = ρvd/μ with μ = 1.79·10⁻⁵ Pa·s |
| Sphere drag crisis (smooth, subsonic) | Re ≈ 2.5·10⁵ (log Re ≈ 5.4). There is none at M ≥ ~0.8–0.9. | McCoy, *Modern Exterior Ballistics*, §4.8, Fig. 4.35 [P] |
| Very rough spheres: critical Re | ≈ 10⁵. Roughness adds ≈ +10 % Cd subsonic and ≈ +5 % supersonic. | McCoy §4.8 [P] |
| Subsonic Cd at Re 10⁴ / 10⁵ (M → 0) | 0.40 / 0.49 | Krumins NOLTR 72-34 Table I [P]; Clift–Gauvin correlation [P, via review] |
| Cd at M = 1.0, Re 10⁴ vs the 9/16″ sphere | 0.71 vs 0.81 | Krumins Table I [P]; GS table [S] |
| Supersonic plateau, M 1.4–2 | ≈ 0.96–1.03 | Krumins [P]; GS [S]; Loth 2021 [P] |
| Lowry steel #2 (0.150″), from 1399 fps | 676 fps at 40 yd (120 ft) | Lowry Table 3 [P, reproduced] |
| Lowry lead #6 (0.110″), from 1375 fps | 675 fps at 40 yd | Lowry Table 3 [P, reproduced] |
| Eley standard cartridge, No. 6 (2.6 mm), OV 325 m/s | 180 m/s at 40 m | Eley Shooter's Diary 1992 via Spencer 2001 [S] |
| Wind drift, #8 lead at 1377 fps, 10 mph crosswind | 9.1 in at 40 yd (23 cm at 36.6 m) | Lowry & Garner, *Shotshell Ballistics for Windows* [P, screenshot] |
| Journée's maximum-range rule | yards ≈ 2200 × diameter in inches (≈ 79 m per mm) | ShootingUK [S]. Journée 1902 original not seen: UNVERIFIED. |
| C.I.P. velocity point for shot | 2.5 m from the muzzle (v2.5) | Naturvårdsverket [P-ish, a government page]; Sporting Gun [S] |
| US velocity point | 3 ft, with coils at 1.5 and 4.5 ft. Non-lead also uses screens at 6 ft over 8 ft. | ANSI/SAAMI Z299.2 [P] |
| C.I.P. steel limits, 12 ga standard / high-performance | ≤ 425 / ≤ 430 m/s (v2.5). Standard steel max 3.25 mm (US 4). | Naturvårdsverket [P-ish] |
| Brenneke Classic 12/70, 31.5 g | 430 → 370 → 330 → 300 → 275 m/s at 0/25/50/75/100 m | Brenneke [P] |
| Fitted G1 BC, Brenneke Classic / Winchester 1 oz Foster | ≈ 0.07 / 0.060 | [C] fit; Winchester catalogue lists 0.060 [P] |

**Bottom line for the engine**

- The current `SPHERE_DRAG` table is already within about 3–4 % of the best published pellet tables
  (Lowry, Eley) at 35–45 m.
- Loth et al. (2021) evaluated at Re = 5·10⁴ fits them as well or slightly better. It is a citable,
  data-fitted source.
- Inside the pellet Reynolds range, a Reynolds correction changes downrange velocity by ≤ 1 % [C].
  It is not needed.
- Details are in §7.

---

## 1. Sphere drag coefficient against Mach at shot-pellet Reynolds numbers

### 1.1 Which Reynolds numbers pellets fly at [C]

Re = ρvd/μ, with ρ = 1.225 and μ = 1.79·10⁻⁵:

| Pellet | 450 m/s | 300 m/s | 150 m/s |
|---|---|---|---|
| 2.0 mm (#9) | 6.2·10⁴ | 4.1·10⁴ | 2.1·10⁴ |
| 3.0 mm (#5) | 9.2·10⁴ | 6.2·10⁴ | 3.1·10⁴ |
| 4.0 mm (steel #1/BB-ish) | 1.2·10⁵ | 8.2·10⁴ | 4.1·10⁴ |
| 8.4 mm (00 buck) | 2.6·10⁵ | 1.7·10⁵ | 8.6·10⁴ |

So birdshot stays in the subcritical regime throughout its flight. Buckshot reaches Re ≈ 2.6·10⁵ only
while it is near Mach 1.3, where the drag crisis is suppressed (McCoy §4.8 [P]). Once it is subsonic
(below ~300 m/s) its Re is below 1.7·10⁵. A very rough or deformed buckshot ball could trip early;
the critical Re of very rough spheres is ~10⁵ (McCoy [P]).

The 9/16-inch steel sphere behind the GS drag function flew at Re ≈ 1.7·10⁵ at M 0.5 and ≈ 3.3·10⁵
at M 1 [C]. That is 3–10 times the Reynolds number of birdshot.

### 1.2 Published Cd(M) data

#### A. GS drag function: 9/16″ smooth steel sphere

- **Origin.** McCoy, *Modern Exterior Ballistics* (2nd ed.), §4.8, Fig. 4.34, credits the 9/16″ sphere
  curve to firings of 9/16″ steel spheres. The figure is labelled "BRL R 514" and cites ref. 21:
  A. C. Charters & R. N. Thomas, "The Aerodynamic Performance of Small Spheres from Subsonic to High
  Supersonic Velocities", *J. Aeronautical Sciences* 12(4):468–476, 1945, doi:10.2514/8.11287 [P].
- **Numeric table.** I took it from the open-source `py-ballisticcalc` (`drag_tables.py`, `TableGS`,
  "9/16" smooth sphere") [S].
  - https://raw.githubusercontent.com/o-murphy/py-ballisticcalc/master/py_ballisticcalc/drag_tables.py
  - I could not confirm that these points are exactly the BRL R 514 values: UNVERIFIED.
  - They do match McCoy's Fig. 4.34 solid curve as read by eye: ≈0.47 at M 0, ≈1.0 at M 1.5, ≈0.96 at M 3.
- **Same sphere elsewhere.** arc.id.au also describes GS as "a 'standard' spherical bullet, 9/16″
  diameter" [S] (https://arc.id.au/CannonballDrag.html).

| M | 0 | 0.3 | 0.5 | 0.6 | 0.7 | 0.8 | 0.9 | 0.95 | 1.0 | 1.05 | 1.1 | 1.2 | 1.5 | 2.0 | 3.0 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| GS Cd | 0.4662 | 0.4827 | 0.4970 | 0.5260 | 0.5920 | 0.6610 | 0.7370 | 0.7757 | 0.8140 | 0.8512 | 0.8870 | 0.9510 | 1.0080 | 1.0010 | 0.9610 |

**McCoy on smaller spheres.** Fig. 4.34 also plots a **0.10″ (2.54 mm) sphere**, which is pellet
size. McCoy writes: "The drag of a 0.10″ diameter sphere is everywhere lower than that of a 9/16″
sphere" [P]. Read by eye from the figure (approximate, ±0.03), the 0.10″ curve gives Cd ≈ 0.37 near
M 0, ≈ 0.45 at M 0.5, about 0.05–0.10 below GS through M 0.8–1.2, and ≈ 0.95 at M 2–3.

#### B. Krumins, *A Review of Sphere Drag Coefficients Applicable to Atmospheric Density Sensing*

- NOLTR 72-34, Naval Ordnance Laboratory, 1972. DTIC AD0742768.
- Read from the scan at https://archive.org/details/DTIC_AD0742768 [P].
- Table I, "Recommended drag table for spheres". It is built on Goin & Lawrence (subsonic range data)
  and on Aroesty, May, May & Witt and others (supersonic).
- The highest Reynolds number it tabulates is 10⁴, the bottom of the pellet range.

| M | 0.1 | 0.2 | 0.3 | 0.4 | 0.5 | 0.6 | 0.7 | 0.8 | 0.9 | 1.0 | 1.1 | 1.2 | 1.3 | 1.4 | 1.6 | 1.8 | 2.0 | 2.4 | 2.8 | 3.0 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Re 4000 | .388 | .410 | .403 | .420 | .440 | .465 | .510 | .568 | .650 | .800 | .915 | .967 | .988 | .998 | 1.000 | 1.000 | 1.002 | 1.012 | 1.029 | 1.040 |
| Re 10000 | .402 | .410 | .418 | .429 | .440 | .455 | .478 | .523 | .585 | .710 | .857 | .927 | .950 | .959 | .962 | .971 | .978 | .988 | 1.000 | 1.008 |

Krumins says the region between M ≈ 0.9 and 2 is poorly covered by data. "Additional measurements
between Mach numbers 1 and 2 are highly desired."

#### C. Correlations fitted to the ballistic-range data

These correlations are fitted to Bailey & Hiatt, Bailey & Starr, Charters & Thomas and others. I took
the formulas verbatim from Capecelatro, "Modeling high-speed gas–particle flows relevant to spacecraft
landings: A review and perspectives", arXiv:2109.02523, Appendix A (https://arxiv.org/abs/2109.02523) [P].

- **Loth, Daspit, Jeong, Nagata & Nonomura (2021)**, "Supersonic and hypersonic drag coefficients for a sphere",
  *AIAA J.* 59(8).
  - Valid for Re ≤ 2·10⁵ and M < 10.
  - Uses the "compression-dominated" branch for Re > 45:
    - `CM = 1.65 + 0.65·tanh(4M − 3.4)` if M < 1.5, else `2.18 − 0.13·tanh(0.9M − 2.7)`
    - `GM = 166M³ + 3.29M² − 10.9M + 20` if M < 0.8, else `5 + 40/M³`
    - `HM = 0.0239M³ + 0.212M² − 0.074M + 1` if M < 1, else `0.93 + 1/(3.5 + M⁵)`
    - `Cd = 24/Re·(1 + 0.15·Re^0.687)·HM + 0.42·CM / (1 + 42500/Re^(1.16·CM) + GM/√Re)`
  - Capecelatro calls it "the most accurate and comprehensive model developed for compressible
    gas–particle flows to date".
- **Parmar, Haselbacher & Balachandar (2010)**, "Improved drag correlation for spheres and application to
  shock-tube experiments", *AIAA J.* 48:1273–1276.
  - Valid for Re ≤ 2·10⁵ and M ≤ 1.75.
  - Fitted to the Bailey & Starr (1976) data.
  - Capecelatro notes that the paper contains a typo, corrected in Parmar's thesis. I used the corrected
    form from the review.
- **Henderson (1976)**, *AIAA J.* 14:707.
  - Valid for Re < 2·10⁴ only, so not used here.
  - Its subsonic branch had a maximum deviation of 16 % against Bailey & Starr, per the review.
- **Clift & Gauvin (1971)**, for incompressible flow (M → 0):
  - `Cd = 24/Re·(1 + 0.15·Re^0.687) + 0.42/(1 + 42500/Re^1.16)`
  - This gives 0.417 / 0.456 / 0.487 / 0.492 / 0.490 at Re 10⁴ / 2·10⁴ / 5·10⁴ / 10⁵ / 1.5·10⁵ [C].

Evaluated at pellet Reynolds numbers [C]:

| Mach | Loth21 Re=2e4 | Loth21 Re=5e4 | Loth21 Re=1e5 | Parmar10 Re=5e4 | Krumins Re=1e4 | GS (9/16″) | engine now |
|---|---|---|---|---|---|---|---|
| 0.00 | 0.431 | 0.462 | 0.471 | 0.487 | (0.402 at M 0.1) | 0.466 | 0.470 |
| 0.30 | 0.444 | 0.472 | 0.478 | 0.493 | 0.418 | 0.483 | 0.470 |
| 0.50 | 0.480 | 0.494 | 0.496 | 0.497 | 0.440 | 0.497 | 0.485 |
| 0.60 | 0.509 | 0.516 | 0.517 | 0.499 | 0.455 | 0.526 | 0.500 |
| 0.70 | 0.534 | 0.543 | 0.550 | 0.523 | 0.478 | 0.592 | 0.540 |
| 0.80 | 0.580 | 0.599 | 0.613 | 0.575 | 0.523 | 0.661 | 0.600 |
| 0.85 | 0.644 | 0.663 | 0.676 | 0.614 | – | 0.699 | 0.650 |
| 0.90 | 0.708 | 0.726 | 0.738 | 0.662 | 0.585 | 0.737 | 0.710 |
| 0.95 | 0.770 | 0.787 | 0.797 | 0.720 | – | 0.776 | 0.780 |
| 1.00 | 0.825 | 0.840 | 0.848 | 0.789 | 0.710 | 0.814 | 0.840 |
| 1.05 | 0.869 | 0.882 | 0.889 | 0.846 | – | 0.851 | 0.890 |
| 1.10 | 0.906 | 0.916 | 0.922 | 0.864 | 0.857 | 0.887 | 0.920 |
| 1.20 | 0.959 | 0.964 | 0.966 | 0.891 | 0.927 | 0.951 | 0.960 |
| 1.30 | 0.992 | 0.993 | 0.991 | 0.911 | 0.950 | 0.991 | 0.975 |
| 1.40 | 1.013 | 1.010 | 1.006 | 0.926 | 0.959 | 1.003 | 0.990 |
| 1.50 | 1.027 | 1.021 | 1.015 | 0.935 | – | 1.008 | 0.995 |
| 1.75 | 1.042 | 1.029 | 1.020 | 0.949 | – | 1.008 | 1.000 |
| 2.00 | 1.048 | 1.031 | 1.020 | – | 0.978 | 1.001 | 1.000 |
| 2.50 | 1.045 | 1.023 | 1.009 | – | – | 0.981 | 0.990 |
| 3.00 | 1.028 | 1.004 | 0.989 | – | 1.008 | 0.961 | 0.980 |

"Engine now" is the linear interpolation of `SPHERE_DRAG` in `pellets.rs`.

**How the sources disagree**

- Subsonic, the sources agree within ~±5 % at Re 5·10⁴–10⁵. Only Krumins' Re = 10⁴ column sits
  ~10 % lower.
- Transonic (M 0.85–1.1), the spread is ±10 %. Parmar and Krumins are lower; GS and Loth are higher.
- Supersonic plateau (M 1.3–2): 0.93 (Parmar, M ≤ 1.75) to 1.03 (Loth).

#### D. Primary measurements I could not open (cited, numbers UNVERIFIED)

- **Bailey & Hiatt.** "Sphere drag coefficients for a broad range of Mach and Reynolds numbers",
  *AIAA J.* 10(11):1436–1440, 1972. The full data are in AEDC-TR-70-291 (ARO Inc., March 1971),
  "Free-flight measurements of sphere drag at subsonic, transonic, supersonic and hypersonic speeds for
  continuum, transition and near-free-molecular flow conditions". It covers 0.1 < M < 6 and
  10¹ < Re < 10⁵ in the AEDC VKF aeroballistic range.
  - The citation and ranges come from Singh et al., arXiv:2012.04813 [S].
  - The DTIC and scispace mirrors refused automated download.
- **Miller & Bailey.** "Sphere drag at Mach numbers from 0.3 to 2.0 at Reynolds numbers approaching 10⁷",
  *J. Fluid Mech.* 93 (1979). It includes Bashforth's 1868 cannon data. Abstract only seen (Cambridge Core) [S].
- **Charters & Thomas (1945)**, cited above. It is the basis of GS. Singh et al. give its range as
  0.3 < M < 4 and 9.3·10⁴ < Re < 1.3·10⁶ [S].
- **Hodges.** "The drag coefficient of very high velocity spheres", *J. Aero. Sci.* 24(10):755, 1957.
  It covers M 2–10 and Re > 3·10⁶, which is not relevant to pellets [S].
- **Goin & Lawrence (1968)**, *AIAA J.* 6(5):961 (subsonic range, Re 200–10⁴). It is the basis of
  Krumins' subsonic columns [P, via Krumins].

**Hoerner on Reynolds effects.** *Fluid-Dynamic Drag* (1965), ch. XVI §(b), p. 16-16: "Tests in
ballistic ranges … between R = 10⁴ and 10⁶, seem to indicate only comparatively small variations" at
transonic and supersonic speeds. His sources are Charters & Thomas, Hodges and NOL. Full text at
https://archive.org/details/FluidDynamicDragHoerner1965 [P].

### 1.3 Is there a Reynolds-number effect worth modelling?

**Physics, which is real**

- McCoy Fig. 4.35 [P] shows Cd rising with Re at M 0.4 from ≈ 0.40 at Re 10⁴ to ≈ 0.50 at
  Re 2·10⁵, just before the crisis.
- At M 0.9 the curve rises from ≈ 0.62 at Re ~5·10³ to ≈ 0.75–0.8 at Re 10⁶. At M 1.2 it runs
  from ≈ 0.82 to ≈ 0.97.
- These are read from the figure, so approximate.
- Krumins (Re 10⁴) and Loth (Re 2·10⁴ to 10⁵) agree with the direction of this trend.

**Effect on pellet flight, which is small [C]**

- Across the real pellet Re band (2·10⁴–1.2·10⁵), Loth 2021 changes Cd by ≤ 7 % subsonic and
  ≤ 3 % supersonic.
- Flying the same pellets with Loth at the local Re versus Loth frozen at Re = 5·10⁴ changed velocity
  at 35–45 m by ≤ 1 %. This held for #9 (2.0 mm), #6, BB and 00 buck (00 buck at 100 m: 205.8 vs
  207.9 m/s).
- That is far below the ±3–8 % spread between the published reference tables (§2).

**Not relevant to pellets: the drag crisis**

- The crisis sits at Re ≈ 2.5·10⁵, or ≈ 10⁵ for very rough spheres (McCoy [P]).
- Birdshot never reaches it.
- Buckshot reaches that Re only while transonic, where the crisis does not occur (McCoy [P]).
- Do not model the crisis.

**Rough versus smooth.** McCoy [P]: large roughness gives ≈ +10 % Cd subsonic and ≈ +5 % supersonic.
Steel shot is smooth and round. Lead shot is deformed at firing (§5.1), which acts like added drag.

### 1.4 Is there a well-known "shot drag function"?

- **GS (9/16″ sphere, Charters & Thomas / BRL).** This is the de-facto sphere function in point-mass
  calculators [S, arc.id.au; py-ballisticcalc]. A forum post says the BfX calculator added GS for
  shotgun pellets with BC = 1 [S, bfxyz.nl via search snippet; the page returned 404: UNVERIFIED].
- **Lowry.** E. D. Lowry ran Winchester's shot-cloud time-of-flight programme for SAAMI.
  - The tables appeared mainly in *The American Rifleman*, 1989 [S, Wakeman].
  - The software *Shotshell Ballistics for Windows* (Lowry & Garner, 1996/1997) defines a pellet
    BC = sectional density / form factor, with form factor = 1.0 for a true sphere [P, Lowry text on
    randywakeman.com].
  - The underlying drag function is not published: UNVERIFIED.
  - Lowry says the older SAAMI tables had unknown origin and were "unwarrantedly generous" downrange
    [P, Lowry via Wakeman].
  - I found no BRL report by Lowry titled "Exterior Ballistics of Shotgun Pellets": UNVERIFIED, and
    probably it does not exist under that title.
- **Journée.** *Tir des fusils de chasse*, 2nd ed., Gauthier-Villars, Paris 1902, 387 pp.
  - The book includes "the method of applying the abacus for finding the remaining velocities of
    projectiles at different ranges" (Nature 66:545, 1902 review,
    https://archive.org/details/paper-doi-10_1038_066545a0) [P, review].
  - I could not find a scan of the book, so Journée's retardation law itself is UNVERIFIED.
- **Allen.** "Approximate ballistics formulas for spherical pellets in free flight", *Defence
  Technology* 14:1–11, 2018, doi:10.1016/j.dt.2017.11.004.
  - It averages two sphere Cd curves into one S-shaped curve made of three straight segments, and
    gives closed-form v(x) and t(x).
  - Abstract only seen (Semantic Scholar) [S]. The breakpoints are UNVERIFIED.

---

## 2. Published downrange tables for validation

### 2.1 Lowry / Winchester, "Shotshell Exterior Ballistics", Table 3 (lead and steel)

- Source: E. D. Lowry, reproduced as an image on randywakeman.com,
  https://randywakeman.com/EdLowryOnShotshellBallistics.htm [P, reproduced].
- Most of this work "appeared in 1989 in the American Rifleman" (Wakeman). The exact issue is UNVERIFIED.
- Conditions: "for shells loaded to produce a 1275 f.p.s. coil reading at 3 ft. when fired from a full choke".
  - The "0 ft" column is the actual starting velocity of the free-flying (front) pellets.
  - Lowry's standard atmosphere is 59 °F, 29.54 inHg, 50 % RH
    (https://randywakeman.com/TheEssentialEdLowry.htm) [P].
- Pellet densities from Lowry's sectional-density table (same site) [P, reproduced]:
  - steel SD #2 = 0.0223 lb/in², i.e. ρ = 7.86 g/cm³;
  - buffered lead SD #6 = 0.0231 lb/in², i.e. ρ ≈ 11.10 g/cm³ [C].

Velocity in ft/s; time in s; energy in ft-lb.

| Shot (dia.) | | 0 ft | 30 ft | 60 ft | 90 ft | 120 ft | 150 ft |
|---|---|---|---|---|---|---|---|
| Steel BB (0.180″) | v | 1376 | 1126 | 952 | 823 | 722 | 639 |
| | t | 0 | .0242 | .0533 | .0872 | .1262 | .1705 |
| | E | 25.45 | 17.05 | 12.19 | 9.11 | 7.00 | 5.48 |
| Steel 1 (0.160″) | v | 1391 | 1117 | 932 | 797 | 692 | 607 |
| | t | 0 | .0242 | .0537 | .0886 | .1291 | .1754 |
| | E | 18.26 | 11.77 | 8.20 | 6.00 | 4.52 | 3.48 |
| Steel 2 (0.150″) | v | 1399 | 1111 | 920 | 782 | 676 | 590 |
| | t | 0 | .0242 | .0540 | .0895 | .1308 | .1784 |
| | E | 15.21 | 9.60 | 6.58 | 4.76 | 3.55 | 2.71 |
| Steel 4 (0.130″) | v | 1415 | 1096 | 892 | 749 | 639 | 552 |
| | t | 0 | .0243 | .0548 | .0916 | .1350 | .1856 |
| | E | 10.13 | 6.08 | 4.03 | 2.84 | 2.07 | 1.54 |
| Lead 2 (0.150″) | v | 1351 | 1129 | 968 | 847 | 749 | 669 |
| | t | 0 | .0244 | .0532 | .0864 | .1241 | .1665 |
| | E | 20.17 | 14.07 | 10.35 | 7.91 | 6.20 | 4.95 |
| Lead 4 (0.130″) | v | 1363 | 1116 | 944 | 816 | 715 | 633 |
| | t | 0 | .0244 | .0538 | .0880 | .1273 | .1720 |
| | E | 13.36 | 8.95 | 6.41 | 4.79 | 3.68 | 2.88 |
| Lead 5 (0.120″) | v | 1369 | 1108 | 930 | 799 | 696 | 613 |
| | t | 0 | .0245 | .0541 | .0890 | .1293 | .1753 |
| | E | 10.60 | 6.95 | 4.89 | 3.61 | 2.74 | 2.12 |
| Lead 6 (0.110″) | v | 1375 | 1099 | 914 | 780 | 675 | 591 |
| | t | 0 | .0245 | .0546 | .0902 | .1316 | .1792 |
| | E | 8.24 | 5.26 | 3.64 | 2.65 | 1.99 | 1.52 |

**Model check [C].** Army Standard Metro, starting at Lowry's 0-ft velocity. Error at 120 ft
(40 yd) and 150 ft:

| Drag | Steel BB | Steel 2 | Steel 4 | Lead 2 | Lead 4 | Lead 6 |
|---|---|---|---|---|---|---|
| Loth21, local Re | 0.0 / +0.3 % | −1.9 / −2.3 % | −4.1 / −5.1 % | +2.9 / +3.9 % | +1.7 / +2.2 % | −0.4 / −0.6 % |
| Loth21 at Re 5e4 | +0.5 / +0.8 | −1.6 / −1.9 | −3.9 / −5.0 | +3.3 / +4.2 | +1.9 / +2.4 | −0.4 / −0.6 |
| GS | −1.5 (120 ft) | −3.7 | −6.1 | +1.6 | −0.1 | −2.6 |
| engine now | +0.6 | −1.3 | −3.4 | +3.3 | +2.0 | −0.2 |

Fitting a Cd multiplier on Loth 2021 to match Lowry at 150 ft [C] gives:

- steel BB 1.005, #1 0.982, #2 0.967, #4 0.932;
- lead #2 1.075, #4 1.039, #5 1.016, #6 0.991.

So Lowry's data imply effective drag within ±7 % of a smooth sphere. Lead runs 0–8 % above steel of
the same size, which is consistent with Lowry's point that firing deforms lead (§5.1).

### 2.2 Lowry & Garner, *Shotshell Ballistics for Windows*: #8 lead lag, drift and drop

- Source: screenshot on randywakeman.com/shotballistics.htm [P, software output].
- Conditions: #8 lead (0.090″) at 1377 ft/s muzzle velocity, "Std. Sea-Level Conditions: Temp = 59 °F,
  Bar = 29.54″". "Average deflection of pellets."

| yd | lag time (s) | drift in 10 mph crosswind (in) | drop (in) |
|---|---|---|---|
| 30 | 0.0287 | 5.0 | 1.4 |
| 40 | 0.0515 | 9.1 | 2.8 |
| 45 | 0.0657 | 11.6 | 3.8 |
| 50 | 0.0818 | 14.4 | 5.0 |
| 60 | 0.1202 | 21.1 | 8.3 |
| 70 | 0.1678 | 29.5 | 12.9 |
| 80 | 0.2261 | 39.8 | 19.3 |

- Lag = time of flight − range/MV. Drift = wind × lag (Didion's rule), which these numbers obey
  exactly: 14.67 ft/s × 0.0515 s = 0.755 ft = 9.06 in [C].
- The model [C] gives lag ≈ 8–12 % longer than Lowry with all three drag tables. For example, at 40 yd:
  - lag 0.0559 s (Loth) / 0.0577 s (GS);
  - drift 9.8 / 10.2 in.
- The "drop" column is smaller than free fall over the time of flight. At 40 yd, t = 0.0515 + 120/1377
  = 0.139 s, so g·t²/2 = 3.7 in, but the table says 2.8 in. I could not reproduce its definition, so do
  not use the drop column as a test.

### 2.3 Eley: striking velocities (m/s) of Eley standard-velocity cartridges

- Source: J. Spencer, "Shotgun barrels and ammunition: specifications & performance", 2001, Table 13,
  "source: Eley". The Eley source is *The Eley Shooter's Diary* 1992.
- The PDF is hosted by NJFF Bodø: https://www.njff.no/nordland/bodo/jegerproven/_/attachment/inline/1abfbf54-463a-4fda-9b1e-3133c57f37c2:b559d36b37b523360ea2275f68bf52cefc19d7d8/Hagleammunisjon,%20antall%20hagl,%20choker,%20treffbilder%20og%20forranhold%20.pdf [S].
- Conditions: English sizes. Observed velocity is 325 m/s (1070 ft/s), where "observed velocity" is
  the average over the first 20 yd.
  - Spencer: "An observed velocity of 1070 ft/s is approximately equal to a muzzle velocity of 1320 ft/s".
  - Elsewhere in the same document he gives 1350 ft/s.
- Atmosphere is not stated. Diameters (English, after Purdey 1936): BB 4.09, 3 3.25, 4 3.05, 5 2.79,
  6 2.59, 7 2.41, 9 2.03 mm.

| English size | 20 m | 25 m | 30 m | 35 m | 40 m | 45 m |
|---|---|---|---|---|---|---|
| BB | 284 | 269 | 254 | 238 | 226 | 212 |
| 3 | 272 | 253 | 236 | 219 | 203 | 188 |
| 4 | 269 | 249 | 231 | 214 | 197 | 181 |
| 5 | 265 | 244 | 224 | 205 | 188 | 171 |
| 6 | 261 | 239 | 219 | 199 | 180 | 163 |
| 7 | 257 | 234 | 212 | 191 | 172 | 154 |
| 9 | 247 | 220 | 196 | 171 | 150 | 128 |

**Model check [C].** Lead 11.1 g/cm³, ICAO, the same MV of 402 m/s (1320 ft/s) for all sizes. Error
at 40 m:

| Drag | BB | 3 | 5 | 6 | 7 | 9 |
|---|---|---|---|---|---|---|
| Loth21, local Re | +0.9 % | +0.9 % | +0.2 % | 0.0 % | 0.0 % | +1.7 % |
| GS | −0.5 % | −1.1 % | −2.1 % | −2.3 % | −2.5 % | −1.5 % |

- At 20–25 m, the Eley values for small shot are 3–10 % higher than the model with a common MV.
- With a per-size MV chosen to give exactly OV 325, the near ranges fit better and the far ranges
  worse (#9 off by +8 % at 40 m and +15 % at 45 m).
- So treat 30–40 m as the test window for sizes BB–7, with ±4 %. Eley's #9 at 45 m (128 m/s) is the
  least consistent point.

### 2.4 Gough Thomas: striking energies (ft-lb) of individual pellets

- Source: Spencer 2001, Table 14, "source: GT" (Gough Thomas's gun books) [S].
- Conditions: OV 1070 ft/s, "muzzle velocity of 1350 f/s", English sizes.

| English size | 30 yd | 35 | 40 | 45 | 50 | 55 | 60 |
|---|---|---|---|---|---|---|---|
| 3 | 4.48 | 3.92 | 3.43 | 2.99 | 2.59 | 2.23 | 1.94 |
| 4 | 3.54 | 3.08 | 2.66 | 2.30 | 1.97 | 1.68 | 1.42 |
| 5 | 2.60 | 2.23 | 1.90 | 1.61 | 1.36 | 1.14 | 0.93 |
| 6 | 2.03 | 1.71 | 1.44 | 1.20 | 1.01 | 0.82 | 0.67 |
| 7 | 1.52 | 1.27 | 1.06 | 0.86 | 0.70 | 0.57 | 0.45 |

**Model check [C].** Loth21 with MV solved per size for OV 1070 ft/s matches 30–45 yd within −4 to
+7 % in energy. The worst point is #7 at 45 yd: 0.92 vs 0.86 ft-lb. Beyond 50 yd the model is 5–30 % high: GT's small shot slows faster far out (for
example #7 at 60 yd, 0.59 vs 0.45 ft-lb). Use 30–45 yd only, with ±8 % energy (±4 % velocity).

### 2.5 H.P. White Laboratory: 40-yard velocities from shot-string tests

Source: S. E. Mayer, "Shot String Silliness", *Shooting Times*, 2018,
https://www.shootingtimes.com/editorial/ammunition_st_silly_122008wo/100155 [S]. Remington 11-87; choke,
payload and muzzle velocity not stated.

| Load | 40 yd velocity | 40 yd string length (100 % of pellets) |
|---|---|---|
| #4 HEVI-Shot | 920 ft/s | 11–12 ft |
| #4 lead | 803 ft/s | 11–12 ft |
| #4 steel | 699 ft/s | 8–9 ft |
| #2 high-velocity steel | 845 ft/s | 7–8 ft |

The muzzle velocity is unknown, so these are not usable as velocity tests. They are useful for string
length (§5.2).

The same article quotes the Lyman *Shotshell Reloading Handbook* (4th ed.): #9 shot at 820 ft/s at
20 yd and 625 ft/s at 40 yd [S].

- The model [C] gives only 553–563 ft/s at 40 yd from 820 at 20 yd, i.e. Lyman retains ~11 % more.
  Lowry's SBW #8 lag also implies less drag than a sphere.
- Other small-shot tables (Eley #9, Gough Thomas #7 far out) imply more drag than a sphere.
- So published small-shot (≤ 2.3 mm) tables disagree with each other by ±10 %. Do not test tightly below 2.4 mm.

### 2.6 Tom Roster: steel energy claims (do not use)

- *Shooting Sportsman*, "Does Speed Kill?" (https://shootingsportsman.com/?p=29907) [S].
- Claims for velocities measured at 3 ft:
  - No. 2 steel (0.150″) at 1300 ft/s keeps 2.18 ft-lb at 40 yd; 1450 → 2.40; 1500 → 2.48; 1550 → 2.55.
  - No. 1 steel (0.160″) at 1300 ft/s keeps 4.70 ft-lb.
- The model [C] gives 3.1–3.3 ft-lb for No. 2 at 1300 and 4.1–4.2 for No. 1.
- Lowry's own table (§2.1) gives No. 2 steel 3.55 ft-lb at 40 yd from 1399 ft/s.
- Roster's No. 1 / No. 2 ratio (2.16×) is physically implausible for a 7 % larger pellet.
- These numbers are internally inconsistent: UNVERIFIED, do not use.

Roster's 2012 lethality table is still useful for steel and HEVI-Shot densities and sizes (§3)
(https://tpwd.texas.gov/huntwild/wild/game_management/media/roster_nontoxic_shot.pdf) [P].

### 2.7 Maximum range: Journée's rule

**The rules as published**

- "If pellet diameter in inches is multiplied by 2,200 then the sum is the theoretical distance of shot
  travel in yards." The article's examples are UK No. 6 at 0.10″ → 220 yd and UK No. 5 at 0.11″ → 242 yd.
  - ShootingUK, "How far do shotgun pellets travel?", 2014, updated 2022,
    https://www.shootinguk.co.uk/news/how-far-do-shotgun-pellets-travel/ [S].
  - The same article says a computer model suggests a strong tailwind adds ~33 %, and that about 29°
    elevation is optimal.
- Sporting Shooter (AU) says a 16 km/h tailwind adds up to 15 %, and that buckshot carries 600–1000 m
  [S] (https://sportingshooter.com.au/ask-uncle-nick/maximum-range-of-shot-pellets/).
- Metric equivalent: 2200 yd/in = 79.2 m per mm [C].
- An RWS glossary rule of "shot diameter in mm × 100 = metres" was reported in a search summary. The
  page now returns 404: UNVERIFIED. That rule is 26 % longer than Journée's.

**What the rule is based on.** Journée's own experiments and calculations (1888/1902 book). Nothing
beyond secondary attributions: UNVERIFIED.

**Check [C].** 2-D point mass, ICAO, MV 400 m/s, best elevation:

| Pellet | Loth21 | GS | Journée rule |
|---|---|---|---|
| lead 2.0 mm | 186 m at 25° | 166 m at 24° | 158 m |
| lead 2.5 mm | 221 m | 200 m | 198 m |
| lead 3.0 mm | 254 m | 233 m | 238 m |
| lead 3.5 mm | 285 m | 265 m | 277 m |
| lead 4.0 mm | 315 m | 295 m | 317 m |
| steel 3.5 mm | 216 m | 199 m | (277 m, lead rule) |
| 00 buck 8.4 mm | 548 m at 28° | 536 m | 664 m |

- Journée's rule matches a sphere model within ±10 % for lead 2.5–4 mm.
- It does not apply to steel. Steel 3.5 mm reaches about 78 % of lead 3.5 mm.
- It overstates for buckshot in this model.
- The best angle is 24–28°, matching ShootingUK's 29°.

### 2.8 Buckshot downrange data

- I found no published, measured downrange velocity table for buckshot.
- Haag's AFTE Journal paper "The Exterior and Terminal Ballistics of 00 Buckshot" measured 00 buck with
  Doppler radar and an Oehler 43 and derived a BC. It is paywalled (https://afte.org/?p=15186), so its
  numbers are UNVERIFIED.
- Muzzle velocities only:
  - Winchester Super-X buffered buckshot: 12 ga 2¾″ 27 pellets of #4 buck at 1325 ft/s @3 ft;
    20 ga 2¾″ 20 pellets of #3 buck at 1200 ft/s [P, Winchester 2026 catalogue p. 42,
    https://online.flippingbook.com/view/721179895/42/; layout scrambled, pairing inferred].
  - Federal 00 buck 2¾″ 9 pellets: 1325 ft/s [S, retailer].
- Buckshot Re reaches the low-10⁵ range, so the subcritical sphere curve applies (§1.1).

### 2.9 Steel versus lead at equal size [C, consistency check]

Sizes in Lowry's table are given in inches, at about the same start velocity. At 40 yd:

- steel #2 676 ft/s vs lead #2 749 ft/s;
- steel #4 639 vs lead #4 715.

The steel-to-lead velocity ratio of ≈ 0.90 follows from the density ratio 7.86 / 11.1. The engine
should reproduce it automatically with a material-independent Cd.

---

## 3. Shot materials and sizes

### 3.1 Densities

| Material | Density g/cm³ | Source |
|---|---|---|
| Pure lead | 11.348 (20 °C) | Wikipedia "Lead" infobox [S, handbook value] |
| Lead with antimony | 0.5 % Sb ≈ 11.20; 2 % ≈ 11.13; 4 % ≈ 10.94; 6 % ≈ 10.76 | [C] back-calculated from SAAMI's "calculated pellet count per ounce" for Dust shot at each Sb %, ANSI/SAAMI Z299.2 p. 38 [P]. Rule of mixtures (Pb 11.34, Sb 6.69) gives 10.89 at 6 % [C]. |
| "Buffered lead" in Lowry's SD table | ≈ 11.10 | [C] from Lowry SD table [P, reproduced] |
| Steel / soft-iron shot | 7.86 (0.284 lb/in³) | SAAMI Z299.2 steel pellet-count note [P]; Roster 2012 ("∼7.86 g/cc, 90–95 DPH") [P] |
| Steel (Finnish retailer guide) | 7.9 | varuste.net guide [S] |
| Bismuth (with 3 % tin) | 9.6 (9.8 without tin); other sources 9.2–9.7 | *The Field* via PressReader [S]; Wakeman 9.6 [S]; Outdoor Life 9.7 [S]. 97 % Bi / 3 % Sn per Wikipedia "Shot (pellet)" [S]. |
| HEVI-Shot (tungsten–nickel–iron) | 12.0 | Roster 2012 table note [P]; American Hunter [S] |
| Tungsten-Matrix (Kent) | 10.8 (Chuck Hawks: 10.6) | Oxford Lead Symposium paper [S]; *The Field* [S]; chuckhawks.com [S] |
| TSS (Tungsten Super Shot) | 18 | Federal Heavyweight TSS page: "With a density of 18 g/cc" [P] |
| Zinc | 7.140 (20 °C) | Wikipedia "Zinc" [S, handbook value]. S&B sells "Zinc Shots" 12/70 30 g (S&B catalogue 2023 p. 88) [P]. |
| Tin | 5.8 (as printed; elemental white tin is ≈ 7.3: UNVERIFIED) | varuste.net guide [S] |
| Tungsten (pure, reference) | 19.3 | American Hunter [S] |

Antimony in practice: SAAMI tabulates lead shot at 0.5, 2, 4 and 6 % Sb, and buckshot as "Soft"
(0.5 %) and "Hard" [P]. A forum figure of 3 % Sb for typical hard field loads is UNVERIFIED.

### 3.2 US sizes (ANSI/SAAMI Z299.2, pp. 38–41) [P]

Lead and steel use the same nominal diameters.

| US | in | mm | Pb pellets/oz at 2 % Sb | Steel pellets/oz |
|---|---|---|---|---|
| 9 | 0.080 | 2.03 | 579 | 821 |
| 8½ | 0.085 | 2.16 | 483 | 685 |
| 8 | 0.090 | 2.29 | 407 | 577 |
| 7½ | 0.095 | 2.41 | 346 | 490 |
| 7 | 0.100 | 2.54 | 296 | 420 |
| 6 | 0.110 | 2.79 | 222 | 316 |
| 5 | 0.120 | 3.05 | 171 | 243 |
| 4 | 0.130 | 3.30 | 135 | 191 |
| 3 | 0.140 | 3.56 | 108 | 153 |
| 2 | 0.150 | 3.81 | 87 | 125 |
| 1 | 0.160 | 4.06 | 72 | 103 |
| B | 0.170 | 4.32 | 60 | 86 |
| BB | 0.180 | 4.57 | 50 | 72 |
| BBB | 0.190 | 4.83 | 43 | 61 |
| T | 0.200 | 5.08 | 37 | 53 |
| TT | 0.210 | 5.33 | 32 | 45 |
| F | 0.220 | 5.59 | 27 | 39 |
| FF | 0.230 | 5.84 | 24 | 35 |

**Buckshot (SAAMI):**

| Size | in | mm |
|---|---|---|
| No. 4 Buck | 0.240 | 6.10 |
| No. 3 | 0.250 | 6.35 |
| No. 2 | 0.270 | 6.86 |
| No. 1 | 0.300 | 7.62 |
| No. 0 | 0.320 | 8.13 |
| No. 00 | 0.330 | 8.38 |
| No. 000 | 0.360 | 9.14 |

**Tolerances (SAAMI):**

- diameter ±0.010″ (0.25 mm) for game shot, ±0.005″ for target shot, ±0.015″ for buckshot;
- pellet count per ounce ±10 %;
- shot weight +4 % / −7 % for game loads.

### 3.3 English sizes (Purdey 1936, via Spencer 2001) [S]

| English | in | mm | English | in | mm |
|---|---|---|---|---|---|
| LG | 0.360 | 9.14 | 1 | 0.143 | 3.63 |
| SG | 0.332 | 8.43 | 2 | 0.135 | 3.43 |
| Special SG | 0.298 | 7.57 | 3 | 0.128 | 3.25 |
| SSG | 0.269 | 6.83 | 4 | 0.120 | 3.05 |
| AAA | 0.203 | 5.16 | 5 | 0.110 | 2.79 |
| AA | 0.194 | 4.92 | 5½ | 0.107 | 2.72 |
| A | 0.180 | 4.57 | 6 | 0.102 | 2.59 |
| BBB | 0.170 | 4.32 | 6½ | 0.099 | 2.51 |
| BB | 0.161 | 4.09 | 7 | 0.095 | 2.41 |
| B | 0.154 | 3.91 | 7½ | 0.090 | 2.28 |
| | | | 8 | 0.087 | 2.21 |
| | | | 9 | 0.080 | 2.03 |

English No. *n* ≈ US No. *n*+1 for sizes 4–6 (English 4 = 3.05 mm = US 5; English 5 = 2.79 mm = US 6).

### 3.4 International equivalents (Eley, via Spencer 2001, Table 2) [S]

| England | mm | USA & Sweden | France | Belgium & Holland | Italy | Spain |
|---|---|---|---|---|---|---|
| LG | 9.1 | – | – | – | – | – |
| SG | 8.4 | 00 Buck | – | – | 9G | 11/0 |
| Sp.SG | 7.6 | 1 Buck | C2 | 12G | 9/0 | – |
| SSG | 6.8 | 3 Buck | C3 | – | – | – |
| AAA | 5.2 | 4 Buck | 5/0 | – | – | – |
| BB | 4.1 | Air rifle | 1 | 00 | 00 | 1 |
| 1 | 3.6 | 2 | 3 | – | 1 or 2 | 3 |
| 3 | 3.3 | 4 | 4 | – | 3 | 4 |
| 4 | 3.1 | 5 | 5 | – | 4 | 5 |
| 5 | 2.8 | 6 | 6 | 5 | 5 | 6 |
| 6 | 2.6 | – | – | 6 | 6 | – |
| 7 | 2.4 | 7½ | 7 | 7 | 7½ | 7 |
| 8 | 2.2 | – | 8 | 8 | – | 8 |
| 9 | 2.0 | 9 | 9 | 9 | 9½ | 9 |

**French "metric" series.** No. 1 = 4.0 mm, then −0.25 mm per number: No. 5 = 3.0, No. 9 = 2.0,
No. 2 = 3.75, No. 4 = 3.25, No. 6 = 2.75, No. 8 = 2.25 mm.

- Source: tirsportifchabris.fr blog [S], via a search summary.
- Historically there were Paris and Lyon series: UNVERIFIED.
- It matches the Eley table to ±0.1 mm.

**Italian.** Fiocchi labels "#5 2,9 mm" and "#3 3,3 mm" (Motonet FI listings [S]). Italian No. 5 is
therefore ≈ US 6, consistent with the Eley table.

**German / RWS–Rottweil labels in Nordic shops** (Motonet.fi listings [S]): #1 4.0, #2 3.7, #4 3.2,
#5 3.0, #6 2.7 mm.

### 3.5 Swedish and Finnish labelling

- **Sweden uses US numbers plus mm.**
  - The Eley/Spencer table heads the US column "USA & Sweden" [S].
  - Naturvårdsverket (Swedish EPA), "Jaga blyfritt med hagel"
    (https://www.naturvardsverket.se/vagledning-och-stod/jakt-och-vilt/jaga-blyfritt-med-hagel/) [P].
    For steel it advises to "gå upp två US-nummer i hagelstorlek, exempelvis från US 5 till US 3".
    It says the shot size goes up by "0,5 mm (två US-nr)". It gives the C.I.P. standard-steel limit as
    "US 4 (3,25 mm)".
  - Norma (Swedish maker) titles its products "Norma Special 36 12/70 No 5 / 3 mm", "No 3 / 3.5 mm",
    "Norma Jaktmatch 24 12/70 No 7 / 2.5 mm" [P, norma-ammunition.com].
  - Gyttorp "Röda Papp" is advertised "i US 3 og 5" (finn.no ad, via search) [S].
  - So a Swedish box shows "US 5 / 3,0 mm" or "No 5 / 3 mm". **Swedish practice does not use English
    numbering.**
  - European "US n" labels give rounded mm: US 4 = 3.25 (SAAMI nominal 3.30), US 5 = 3.0 (3.05),
    US 3 = 3.5 (3.56).
- **Finland: retailers show maker number plus mm.**
  - Examples: "Rottweil Waidmannsheil 12/70 36g #5 3,0 mm"; "Fiocchi PL32 12/70 32 g #5 2,9 mm";
    "UnA-Tungsten 12/76 #4 3,25 mm"; "Winchester Superspeed 12/76 #0 3,90 mm" [S, motonet.fi /
    karkkainen.com].
  - So the number on a Finnish box depends on the maker's national series. The mm value is the reliable field.
  - varuste.net's Finnish guide pairs mm with numbers: 2.4 = 7.5, 2.75 = 6, 3.0 = 5, 3.25 = 4, 3.5 = 3 [S].
  - The guide says lead was banned for waterfowl in Finland in 1996 [S].
- **"Steel two sizes larger."**
  - Naturvårdsverket: two US numbers, or +0.5 mm [P-ish].
  - For bismuth and copper a smaller step is enough (same source).
  - Roster's 1968–2014 lethality analyses recommend steel #3 for ducks where lead #4–#5 was used
    (Roster 2012 table) [P].

### 3.6 Steel limits (C.I.P., as reported by Naturvårdsverket) [P-ish]

Velocities are v2.5, measured 2.5 m from the muzzle.

**Standard steel:**

| Gauge | Max velocity | Max momentum | Max shot |
|---|---|---|---|
| 12 | 425 m/s | 12 N·s | 3.25 mm (US 4) |
| 16 | 390 m/s | 9.5 N·s | 3.0 mm |
| 20 | 410 m/s | 9.3 N·s | 3.0 mm |

**High-performance steel:**

| Chamber | Max velocity | Max momentum |
|---|---|---|
| 12/70 | 430 m/s | 13.5 N·s |
| 12/76 | 430 m/s | 15 N·s |
| 12/89 | 430 m/s | 19 N·s |
| 16/70 | 410 m/s | 12 N·s |
| 20/70 | 420 m/s | 11 N·s |
| 20/76 | 430 m/s | 12 N·s |

The C.I.P. decision text sits behind a login (bobp.cip-bobp.org), so it is UNVERIFIED at source.
shotguns.se gives the same 9.5/12 N·s for 70 mm steel and works an example: 30 g at 385 m/s = 11.55 N·s [S].

---

## 4. Typical muzzle velocities, their spread, and how they are measured

### 4.1 Where velocity is measured

- **US (SAAMI).**
  - Lead shot is measured in a full-choke test barrel at "instrumental velocity at 3 feet over 3 feet",
    with coils at 1.5 ft and 4.5 ft.
  - Non-lead shot uses an IC-choke barrel with coils at 3 ft and/or screens at "8 feet over 6 feet".
  - Slugs use coils at 3 ft and screens at "15 feet over 20 feet".
  - Source: ANSI/SAAMI Z299.2, Section III layouts, and the SAAMI reference-ammunition table
    (https://saami.org/wp-content/uploads/2024/10/Shotshell-Reference-Ammunition-Supplemental-Information-Table-Revised-2024-09-04.pdf) [P].
  - Velocities are lot means. Ammunition tested later "can be expected to produce velocities within a
    tolerance of ±90 fps" (±27 m/s) of the tabulated values [P].
- **C.I.P. / Europe.** v2.5, 2.5 m from the muzzle [P-ish, Naturvårdsverket; S, Sporting Gun
  "Forward thinking" 2022 via PressReader].
  - Makers print "V2.5", for example Rottweil Steel Game Speed 28 g 3.0 mm, V2.5 = 425 m/s
    [S, hunting-sport.com].
  - Others print "V0": Norma Special 36 g 12/70 V0 = 375 m/s (1230 ft/s) [P, norma-ammunition.com].
    Rottweil Waidmannsheil HV 36 g 2.7 mm V0 = 395 m/s [S, retailer]. Whether "V0" is extrapolated
    from v2.5 is UNVERIFIED.
- **UK.**
  - "Observed velocity" (OV) is the mean over the first 20 yd. OV 1070 ft/s ≈ MV 1320 ft/s
    [S, ShootingUK; Spencer].
  - Eley standard game loads have OV 1070 ft/s (325 m/s). High-velocity loads have OV 1120 ft/s (340 m/s) [S, Spencer citing Eley].
  - Modern Eley pages quote muzzle velocity. Grand Prix High Pheasant 30 g No. 6 is listed at
    "420 mps • 1377 fps" (search snippet; the page refused fetch, UNVERIFIED).

### 4.2 Typical values

| Load | Velocity | Where | Source |
|---|---|---|---|
| 12 ga 2¾″ 1¼ oz (35 g) lead #6, SAAMI reference | 1330 ft/s (405 m/s) | 3 ft | SAAMI ref. table [P] |
| 12 ga 3″ 1¼ oz steel #2, SAAMI reference | 1375 ft/s (419 m/s) | 3 ft | SAAMI ref. table [P] |
| 20 ga 2¾″ 1 oz (28 g) lead #6 | 1220 ft/s (372 m/s) | 3 ft | SAAMI ref. table [P] |
| 20 ga 3″ 1 oz steel #2 | 1330 ft/s | 3 ft | SAAMI ref. table [P] |
| 16 ga 2¾″ 1⅛ oz lead #6 | 1185 ft/s | 3 ft | SAAMI [P] |
| 1968 Patuxent test: 1¼ oz #4 lead / 1 oz #4 steel / 1¼ oz #6 lead | 1336 / 1375 / 1337 ft/s | 3 ft | Lowry Table 1 [P, reproduced] |
| Norma Special 12/70 36 g No 5 | 375 m/s | "V0" | Norma [P] |
| Rottweil Waidmannsheil HV 12/70 36 g 2.7 mm | 395 m/s | "V0" | retailer [S] |
| Rottweil Steel Game Speed 12/70 28 g 3.0 mm | 425 m/s | V2.5 | retailer [S] |
| Rottweil steel 12/70 32 g 3.0–3.75 mm / "normal proof" 2.6–3.25 mm 32 g | 400 / 375 m/s | V2.5 | Rottweil flyer via search [S]; flyer URL 404, UNVERIFIED |
| Rottweil 12/76 35 g 3.25 mm steel / 12/70 36 g 3.2 mm lead | 429 / 400 m/s | V2.5 | Rottweil flyer via search [S], UNVERIFIED |
| Federal Heavyweight TSS turkey 12 ga | 1000–1200 ft/s | "muzzle" | Federal [P] |
| Compak sporting example | 395 ± 5 m/s | 2.5 m | Decathlon via search [S], UNVERIFIED |

Summary: European 12 ga game loads of 28–36 g run about 375–410 m/s (v2.5), and steel 400–430 m/s
(C.I.P. caps it at 425/430). US lead field loads run 1220–1330 ft/s (372–405 m/s) at 3 ft, and US
steel 1300–1450 ft/s (396–442 m/s).

### 4.3 Speed lost in the first metres [C]

Loth21 drag, ICAO, true MV 400 m/s:

| Pellet | v at 1 m | v at 3 ft | v at 2.5 m | OV over 20 yd |
|---|---|---|---|---|
| lead 2.4 mm | 387.1 | 388.3 | 369.1 | 307 |
| lead 3.0 mm | 389.6 | 390.6 | 374.9 | 321 |
| lead 3.5 mm | 391.1 | 391.9 | 378.3 | 330 |
| steel 3.0 mm | 385.5 | 386.8 | 365.3 | 299 |
| steel 3.5 mm | 387.5 | 388.6 | 369.9 | 309 |
| steel 4.0 mm | 389.0 | 390.0 | 373.5 | 318 |
| TSS 2.25 mm (18 g/cm³) | 391.5 | 392.2 | 379.2 | 333 |
| 00 buck 8.4 mm | 396.2 | 396.6 | 390.7 | 368 |

So a supersonic pellet loses ≈ 2.5–3.5 % per metre. A v2.5 figure is ≈ 25–35 m/s below the true
muzzle velocity for birdshot. The engine should treat a C.I.P. v2.5 as a velocity at x = 2.5 m, not as
the muzzle velocity.

**Coil-chronograph bias (Lowry [P]).**

- A coil chronograph responds to the shot column's centre of mass. In 1969 Winchester found the 3 ft
  coil reading below the true velocity of the free-flying front pellets.
- The bias is "much larger with steel" and depends on choke.
- For No. 4 steel the measured 3-ft value was 99 ft/s below the true value. A true 3-ft velocity of
  1374 ft/s needed a 1415 ft/s muzzle velocity.
- Loads giving a 1275 ft/s coil reading had true 0-ft pellet velocities of 1351–1415 ft/s (Table 3).
  That is 6–11 % above the coil reading.

---

## 5. Other effects on one pellet's flight

### 5.1 Deformation of lead pellets

- Lowry [P, via Wakeman]: "Set-back deformation produced the large, saucer-shaped depressions in the lead
  pellets." Such distortion causes "losses in velocity" and reduced striking energy, which helped explain
  why 1 oz of steel #4 killed nearly as well as 1¼ oz of lead #4 at Patuxent in 1968
  (Table 1: 361 vs 389 birds bagged).
- Size of the effect, inferred [C]: Lowry's lead pellets need a Cd 0–8 % above smooth-sphere Loth 2021
  to match his table, while his steel needs −7 to 0 % (§2.1). Lead–steel offset: about +5 %.
- McCoy [P]: very rough spheres +10 % subsonic, +5 % supersonic. That is an upper bound for a lightly
  dented pellet.
- Hard (antimony) or plated shot deforms less (American Rifleman, "Pheasant loads on a budget" [S]). No
  controlled hard-versus-soft velocity study was found: UNVERIFIED.

### 5.2 Shot string in length and time at 30–40 m

- **H.P. White** (Shooting Times 2018 [S]): 100 % string length at 40 yd:

  | Load | Length | Time span [C] |
  |---|---|---|
  | #4 lead | 11–12 ft (3.4–3.7 m) | ≈ 14–16 ms |
  | #4 HEVI-Shot | 11–12 ft | ≈ 12.5 ms |
  | #4 steel | 8–9 ft (2.4–2.7 m) | ≈ 12–14 ms |
  | #2 HV steel | 7–8 ft (2.1–2.4 m) | ≈ 9 ms |

  Industry tests usually measure 80 % of the string.
- **Lowry** defines shot string as the shortest length containing 90 % of the pellets at 40 yd
  (TheEssentialEdLowry) [P]. Spark shadowgraphs show a full-choke string longer at 6 ft, equal at
  14 ft, and a cylinder-bore string at least as long by 18 ft [P].
- **Spencer** (citing Gough Thomas) [S]: "approximately half the shot is in the front third" of the string.
- **Clay Shooting USA** [S]: a target-load string at 40 yd is "six feet or a good bit longer" (≈5.5 ft
  with IC, a foot longer with Full, per one researcher). Not checked further.

### 5.3 Wind drift

- Pellets drift a lot. Lowry's #8 lead at 1377 ft/s drifts 9.1 in at 40 yd and 21.1 in at 60 yd in a
  10 mph (4.47 m/s) crosswind [P, §2.2]. That is ≈ 5 cm per m/s of crosswind at 36.6 m.
- Typical game pellets at 35 m in a 5 m/s crosswind [C, Didion lag rule with Loth21, MV 400 m/s]:
  - lead 2.4 mm 25 cm; lead 3.0 mm 20 cm; lead 3.5 mm 17 cm;
  - steel 3.0 mm 28 cm; steel 3.5 mm 24 cm; TSS 2.25 mm 16 cm;
  - 00 buck 7.5 cm.
- A rifle bullet drifts far less at that range. The engine's point-mass wind handling should reproduce
  Didion's rule (drift = W × lag) exactly for a flat shot.

### 5.4 The wad and the first metres

- Lowry [P]: "Only the front pellets are in free flight immediately after muzzle exit, and it is their
  initial velocity that determines the downrange performance of the whole shot charge."
- Front pellets "leave turbulent wakes", and trailing pellets in that wake meet less resistance and
  "begin to 'catch up'" (drafting).
- Full choke "squirts" the charge so pellets separate quickly. A cylinder-bore charge flattens and
  disperses at once.
- I found no number for wad separation distance: UNVERIFIED.
- For a single-pellet engine, the practical rules are:
  - start the pellet at the true front-pellet velocity, or treat C.I.P. v2.5 as the velocity at 2.5 m;
  - accept that a 3-ft coil figure under-reads by up to ~7 % (steel, full choke; Lowry).

### 5.5 Initial velocity spread within one shot

- No direct per-pellet measurement was found.
- Compton's UCL PhD thesis, "An experimental and theoretical investigation of shot cloud ballistics"
  (1996), measured pellet positions in transonic shot clouds. It could not be downloaded (Cloudflare):
  UNVERIFIED.
- Indirect evidence:
  - Front pellets fly 6–11 % faster than the coil-measured column mean (Lowry Table 3 against the
    1275 ft/s coil reading) [P].
  - Inverting the H.P. White string lengths with the sphere model [C]: a 5 % slower start arrives
    ≈ 4.6 ms later at 40 yd. So an 11.5 ft #4 lead string (~16 ms) matches a total start-velocity
    range of ≈ 15–17 %, or roughly a ±3–4 % SD if roughly normal.
  - Size scatter (SAAMI ±0.25 mm) and deformation also lengthen the string, so this is an upper bound.
- A modelling paper on shot dispersion (core.ac.uk 267013404) used a transverse velocity SD of 0.5 % of
  axial velocity, "arbitrary" by its own description [S, via search].

---

## 6. Slugs

**Brenneke official ballistic tables** [P, brenneke-ammunition.de product pages, e.g. https://www.brenneke-ammunition.de/en/details/1201620]:

| Slug | Mass | 0 m | 25 m | 50 m | 75 m | 100 m |
|---|---|---|---|---|---|---|
| Classic 12/70 | 31.5 g | 430 | 370 | 330 | 300 | 275 m/s (2912 → 1191 J) |
| Classic Magnum 12/70 | 31.5 g | 460 | 400 | 352 | 319 | 297 m/s (3335 → 1389 J) |
| K.O. 12/60 | 28.4 g | 380 | 334 | 303 | 282 | 265 m/s |
| Rubin Sabot 12/70 | 28 g | 468 | 417 | 374 | 340 | 316 m/s |
| Super Sabot 12/70 | 31.8 g | 429 | 385 | 350 | 323 | 304 m/s |
| Topas Sabot 12/70 | 20 g | 510 | 456 | 409 | 369 | 338 m/s |

Brenneke Classic 16/70 is 27 g at 415 m/s, and 20/70 is 24 g at 430 m/s (V0 only) [P]. Atmosphere and
measurement method are not stated.

**Fitted G1 BC [C].** Assuming ICAO, fitted separately to each downrange point:

| Slug | 25 m | 50 m | 75 m | 100 m |
|---|---|---|---|---|
| Classic | 0.071 | 0.073 | 0.071 | 0.067 |
| Classic Magnum | 0.080 | 0.079 | 0.079 | 0.080 |
| K.O. | 0.070 | 0.069 | 0.070 | 0.070 |
| Rubin Sabot | 0.097 | 0.097 | 0.097 | 0.098 |
| Super Sabot | 0.100 | 0.101 | 0.100 | 0.101 |
| Topas Sabot | 0.101 | 0.102 | 0.102 | 0.102 |

The near-constant BCs suggest Brenneke computed the tables with G1 (or similar). They are not an
independent proof that G1 fits slug drag.

**Winchester** [P, 2026 catalogue p. 42; text extraction is columnar and the pairing inferred by energy]:

- Super-X Rifled Slug (Foster) 12 ga 1 oz (438 gr): 1600 ft/s → 1161 at 50 yd → 953 at 100 yd;
  2488 → 1310 → 882 ft-lb. Catalogue BC 0.060.
  - My G1 fit with Army Standard Metro gives 0.060 at both 50 and 100 yd [C].
  - Other Super-X slug BCs printed on the page: 0.068, ¾ oz 0.040 (lead-free zinc), 1⅛ oz 0.045,
    ¾ oz 0.060, ¼ oz 0.058. Their row assignment is not certain.
- Copper Impact sabot 12 ga 1 oz, BC 0.190: 1600 / 1438 / 1296 / 1234 ft/s at 0/50/100/125 yd.
  - G1 + Army Standard Metro reproduces 1438 / 1296 / 1233 exactly [C].
  - So Winchester's slug tables are G1 calculations at Army Standard Metro.

**SAAMI reference slugs** [P]: 12 ga 2¾″ 1⅛ oz rifled at 1610 ft/s @3 ft and 1590 @15 ft; 1 oz
1¾″ 1200 / 1180; saboted 1⅛ oz 1345 / 1320. These are nominal values, "subject to change".

**Is G1 fair for slugs?**

- For Foster and Brenneke slugs over ≤ 100 m, the makers' own tables are G1 calculations, so a G1 BC
  reproduces them exactly.
- Whether real slug drag follows G1 is UNVERIFIED. No Doppler data was found.
- A forum post (gatewaytoairguns) says G1 is "not a very good fit" to slug drag [S, weak].
- Haag's AFTE work used Doppler only for buckshot.
- Practical advice: use G1 with the published BC for slugs, keep tests to ≤ 100 m, and expect several
  per cent error beyond.

---

## 7. Recommendations for the engine

### 7.1 Cd(Mach) table

Use **Loth et al. (2021) evaluated at Re = 5·10⁴** as the single 1-D sphere curve. The formula is in
§1.2-C and the points are computed from it [C]:

| M | Cd |
|---|---|
| 0.00 | 0.462 |
| 0.30 | 0.472 |
| 0.50 | 0.494 |
| 0.60 | 0.516 |
| 0.70 | 0.543 |
| 0.80 | 0.599 |
| 0.85 | 0.663 |
| 0.90 | 0.726 |
| 0.95 | 0.787 |
| 1.00 | 0.840 |
| 1.05 | 0.882 |
| 1.10 | 0.916 |
| 1.20 | 0.964 |
| 1.30 | 0.993 |
| 1.40 | 1.010 |
| 1.50 | 1.021 |
| 1.75 | 1.029 |
| 2.00 | 1.031 |
| 2.50 | 1.023 |
| 3.00 | 1.004 |

**Why this table**

- **It is citable.** A peer-reviewed correlation fitted to the ballistic-range sphere data (Bailey &
  Hiatt, Bailey & Starr, Charters & Thomas and others) and DNS. It sits at the middle of the pellet
  Reynolds band.
- **It fits the tables.**
  - It reproduces Lowry Table 3 within −4 … +4 % at 40 yd and −5 … +4 % at 50 yd, across steel BB–#4
    and lead #2–#6.
  - It reproduces Eley's table within ±1.7 % at 40 m for BB–#9 with a common 402 m/s MV.
  - It reproduces Gough Thomas energies within −4 … +7 % at 30–45 yd.
  - It reproduces Lowry's time of flight to 40 yd within −1 … +3.4 %.
- **GS** (Charters & Thomas 9/16″) is about 2–3 % slower at 40 yd for small shot. It is measured at
  3–10× pellet Reynolds numbers, and McCoy says small spheres have lower drag.
- **The current `SPHERE_DRAG`** is already close: it differs from the table above by ≤ 0.035. Changing
  it is a matter of provenance more than accuracy. If the team prefers to keep it, document it as
  "≈ Loth 2021 at Re 5·10⁴" and keep the tests below.
- **Transonic uncertainty.** Around M 0.85–1.1 the sources disagree by ±10 %: Krumins at Re 10⁴ and
  Parmar are lower, GS is higher. This is the main remaining uncertainty, and it does not move 35–45 m
  velocities by more than ~2–3 %.

### 7.2 Reynolds correction

- **Not needed.** Inside the pellet band (2·10⁴–1.2·10⁵), local-Re Loth versus Loth at a fixed 5·10⁴
  changes 35–45 m velocities by ≤ 1 % for #9, #6, BB and 00 buck. That is below the scatter between
  the published reference tables.
- **Do not model the drag crisis.** It sits at Re ≈ 2.5·10⁵, is never reached subsonically by
  birdshot, and is suppressed at the Mach numbers where buckshot reaches that Re.
- **If one is added later** (e.g. for airgun balls or very small shot, Re < 10⁴), use the full Loth
  2021 compression-dominated formula with the local Re and M.
- **Material and deformation.** Rather than a Reynolds term, consider an optional per-material drag
  factor: lead ≈ 1.03–1.05, steel/TSS/bismuth 1.00. This is inferred from Lowry (§2.1, §5.1) and is a
  judgement, not a published constant.

### 7.3 Inputs and conventions

- Densities: lead 11.1 (hardened, 2–4 % Sb), pure 11.35, steel 7.86, bismuth 9.6, HEVI-Shot 12.0,
  Tungsten-Matrix 10.8, TSS 18.0, zinc 7.14 g/cm³ (§3.1). The current `LEAD_DENSITY_KG_M3 = 11 340` is
  pure lead. Hardened shot is ~1–3 % lighter.
- Diameters: store mm.
  - Map US numbers to SAAMI nominal diameters with the §3.2 table. From #9 to FF each step is
    0.010″, e.g. #6 = 0.110″, #2 = 0.150″, BB = 0.180″.
  - Map European "US n" labels to the rounded mm the makers print (US 4 = 3.25, US 5 = 3.0, US 3 = 3.5).
  - Sweden uses US numbers. Finland prints the maker's number plus mm.
- Velocity reference point: let the user say whether a velocity is v0, v2.5 (C.I.P.), 3 ft (SAAMI) or
  OV over 20 yd (UK), and back-solve the muzzle velocity with the same drag. The differences are
  25–35 m/s for v2.5 and ~80 m/s for OV.

### 7.4 Reference tests and tolerances

All tests below fly a single sphere, flat fire, with the stated atmosphere.

1. **Lowry Table 3** (§2.1).
   - Army Standard Metro (ρ ≈ 1.203). Start at Lowry's 0-ft velocity. Densities: steel 7.86 g/cm³,
     lead 11.10 g/cm³. Diameters in inches as given.
   - Assert velocity at 60, 90 and 120 ft within **±5 %**, and 150 ft within **±6 %**.
   - Assert time of flight at 120 ft within **±4 %**.
   - Steel #4 is the hardest case: the model is −4 to −5 %.
2. **Eley Table 13** (§2.3).
   - ICAO, lead 11.1 g/cm³, English diameters, MV 402 m/s.
   - Assert velocity at 30, 35 and 40 m within **±4 %** for BB, 3, 4, 5, 6 and 7.
   - Optionally #9 at 40 m within ±5 %, and skip #9 at 45 m.
3. **Gough Thomas Table 14** (§2.4).
   - Back-solve MV for OV 1070 ft/s over 20 yd per size.
   - Assert energies at 30, 35, 40 and 45 yd within **±8 %**.
4. **Lowry SBW #8 lag and drift** (§2.2).
   - 1377 ft/s, 59 °F, 29.54 inHg.
   - Assert drift at 40 yd in a 10 mph crosswind = 9.1 in within **±15 %** (the model gives +8 to +12 %).
   - Assert that drift equals wind × lag to < 1 % (a solver-consistency check that needs no data).
5. **Steel/lead ratio** (§2.9). With equal diameter and start velocity, steel/lead velocity at 40 yd is
   0.89–0.91 (Lowry #2 and #4: 0.903 and 0.894).
6. **Journée maximum range** (§2.7).
   - Lead 2.5, 3.0, 3.5 and 4.0 mm at 400 m/s, best angle.
   - Range = 79.2 m × d(mm) within **±15 %**. The optimal angle lies between 22° and 30°.
   - Soft test: the rule is secondary-sourced.
7. **Slugs**, if the engine flies them with G1.
   - Brenneke Classic 31.5 g from 430 m/s with G1 BC 0.071 (ICAO): 370 / 330 / 300 / 275 m/s at
     25 / 50 / 75 / 100 m, each within **±2 %**. The model [C] gives 370 / 328 / 300 / 280.
   - Winchester 1 oz Foster from 1600 ft/s, BC 0.060, Army Standard Metro: 1161 ± 10 ft/s at 50 yd
     and 953 ± 10 ft/s at 100 yd.
   - Copper Impact, BC 0.190: 1438 / 1296 ft/s ± 1 %.

**Do not test against** Roster's steel energies (§2.6), the drop column of the SBW screenshot (§2.2),
or small-shot tables below 2.3 mm beyond ±10 % (Lyman #9 and Eley #9 disagree in opposite directions).

---

The [C] numbers came from a short point-mass sphere solver written for this research; it was not kept. The engine's own tests (`crates/ballistics-core/src/tests.rs`) check the same tables.
