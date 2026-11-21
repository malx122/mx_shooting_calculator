# The shot cloud in three dimensions: research for a per-pellet swarm model

Researched 2026-11-21. Builds on `docs/shotgun-research/patterns.md` (§3 radial distribution, §6 shot
string) and `flight.md` (§5 deformation, wad, speed spread). It does not repeat their sphere-drag or
σ(range) work.

**In the engine** (`ballistics_core::pellets::Swarm`, 2026-11-21): §10's model without the optional
parts (no size scatter, no per-pellet lateral scale, no steel strays). Soft lead keeps the defaults of
§10.2 (a = 3.5 %, b = 0.12): a closer fit to Lowry's table A overshoots Compton's 50 m lead lengths by
20–25 %, so the tests allow ±15 % on table A. Game shot (`Material::Lead`, hardened) uses a = 2 %,
b = 0.10; bismuth, tungsten and zinc are taken as hard lead. The spread's mean grows as the choke law
(`sigma_at`) gives; within it each pellet's grows with its own flight time in the share w of §10.3.

**Marks.** Every number carries a source key (listed in §12) and one of these marks:

- **[P]** read in the primary source (thesis, scanned article, paper, book text);
- **[S]** secondary account (magazine summary, review, a thesis quoting someone else);
- **[U]** unverified: from a search summary, an abstract I could not open, or a figure I could only
  name;
- **[D]** derived by me, with throwaway scripts (not kept) that flew pellets with the app's own
  `ballistics_core::pellets::fly` sphere-drag model. The engine's swarm
  (`crates/ballistics-core/src/pellets/swarm.rs`) and its tests redo the checks that matter.
- **[eye]** read off a plot by eye. Expect ±5 % of full scale.

---

## 0. Summary

1. **The cloud is a cigar with a blunt, dense front and a long, thin tail.** Down range, the
   longitudinal pellet density is skewed (Rayleigh-shaped), not Gaussian.
   - Lowry's 40-yd figures for a 1¼ oz #4 load: half the pellets lie within 58 in of the front pellet,
     80 % within 88 in, 90 % within 105 in and 95 % within 120 in [P Lowry79, Table A].
   - A Rayleigh curve with σ = 49 in (1.25 m) reproduces all eight of Lowry's numbers to within
     1 in [D].
   - Compton found the Rayleigh to be the best visual fit to the arrival profiles at 20–50 m.
     Its peak lies at about 2, 4, 5 and 7 ms behind the front at 20, 30, 40 and 50 m (#3 lead)
     [P Compton96 §3.3.4, Fig 3.16, eye].
   - Laterally the swarm is a radial Gaussian, the same in x and y, and Poisson in small cells
     [P Compton96 §3.4].
2. **Where a pellet flies depends on its position in the string.** The well-formed pellets fly at
   the front and in the centre of the pattern; the deformed ones fly at the back and on the outside
   [P Compton96 §3.2.1, §3.5; P Brister76 p. 190; Journée via S Compton96].
   - The dissent: Lowry wrote that a pellet's place in the pattern is "for the most part unrelated" to
     its place in the string [P Lowry79].
   - No source gives a correlation coefficient. **[gap]**
3. **Speeds within one shot.** No measured histogram of pellet start speeds exists in the public
   literature I could find. **[gap]**
   - Compton's leading-edge and trailing-edge fits put the trailing edge 7–15 % slower than the
     leading edge at 20 m [P Compton96 Fig 4.4, eye].
   - Fitting the engine to Lowry's and Compton's string lengths gives effective free-flight start
     speeds that fall short of the front pellet by a half-normal amount [D, §2.4]:
     - soft lead: scale ≈ 3.5 %, so a mean shortfall of 2.8 % and a 95th percentile of 6.9 %;
     - plus a half-normal excess drag of scale ≈ 0.12, so a mean of +10 % and a 95th percentile
       of +24 %.
   - Lowry's "front pellets 6–11 % faster than the coil" is **not** the spread of the pellets
     behind them. The coil reads a column that has not yet broken up.
4. **What lengthens the string down range is drag differences, not speed differences.**
   - A pure start-speed spread gives a lag that nearly stops growing after 30 m. A drag spread gives
     a lag that grows about 2.2× from 40 to 65 yd [D].
   - Lowry's 80 % strings grow 1.96× from 40 to 65 yd [P Lowry79 Table C]. Compton finds the
     "in-flight effects" dominate the length and the choke has no consistent effect at 20–50 m
     [P Compton96 §3.3.2, §5].
5. **String lengths at 40 yd.**
   - Lowry's 80 % length: 79 in for standard 1¼ oz #4 lead, 56 in for buffered 1½ oz #4, 55 in for
     1⅛ oz #2 steel [P Lowry79].
   - Total length: 11–12 ft for #4 lead, 8–9 ft for #4 steel [S HPW via Shooting Times].
   - Choke matters near the muzzle (full choke is longer at 6 ft) but not by 14–18 ft
     [P Lowry shadowgraphs].
6. **Deformation.**
   - Badly deformed pellets have 20–30 % more drag than sound ones [S Donovan67 via Compton96].
   - Compton's trailing-edge lead decelerates 22 % harder than his leading-edge lead; for steel the
     difference is 5 % [D from P Compton96 eq. 4.21/4.22].
   - Tighter chokes deform soft shot more; steel is barely deformed [P Compton96 §3.2.1].
   - No measured share of deformed pellets was found. **[gap]**
7. **Clumping.**
   - Within rings of equal density, counts in 100-mm cells follow Poisson [P Compton96 Fig 3.33].
   - Compton's 10-shot counts in 5-, 10- and 30-in circles fit Poisson scatter plus a shot-to-shot σ
     variation of about 4 %. Their *mean* counts in 5 and 10 in are 37 % and 28 % above a Gaussian
     fitted to the 30-in count, which suggests a slightly peaked core [D from P Compton96 Table 3.14].
   - Gough Thomas's patch counts are slightly patchier than Poisson (patterns.md §3.4).
   - No real clustering parameter can be justified.
8. **Shot-to-shot variation.**
   - The pattern σ varies by 5–10 % between shots of one batch in Compton's raw numbers; about 4 % is
     left once the noise from a finite number of pellets is removed [P/D Compton96].
   - The front pellets' velocity varies by 1.1 % for lead and 1.7 % for steel between shots
     [P Compton96 Table 3.2].
   - No measurement of how far the pattern centre wanders from shot to shot was found. **[gap]**
9. **The wad and the gas.**
   - The wad cannot be told apart from the shot at 1 m and separates from 2 m on.
   - At 8 m it is 0.2–2.5 m behind the front of the shot, very unevenly from shot to shot. Some steel
     wads stayed close to the shot because pellets had embedded in their base [P Compton96 §3.3.1].
   - Gas leaking past fibre wads can fuse pellets into clumps ("balled shot") [S ShootingUK].
10. **Crossing targets.**
    - Lowry computed 3–6 % pattern loss at 55 mph and 40–65 yd for 1979 loads [P Lowry79 Table C].
    - Brister measured stationary-versus-moving losses of 7.4 % for a buffered hard-shot load and
      20.5 % for a soft magnum [P Brister76].
    - Burrard, with 1920s soft shot at 40 yd, lost 30 % on average with improved cylinder and 13–17 %
      with full choke [S Burrard via Brister76].
    - Leads at 40 mph: about 7 ft at 40 yd and about 18 ft at 70 yd [P Brister76].

A per-pellet model built on these numbers is in §10. Its main parameters, calibrated on Lowry,
reproduce Compton's independent Rayleigh peak times to within 1 ms [D].

---

## 1. Sources: what was read and what was not

| Source | Status |
|---|---|
| **D. J. Compton, *An experimental and theoretical investigation of shot cloud ballistics*, PhD, UCL, Nov 1996, 278 pp.** | **Read in full [P]** from the archived UCL PDF (396689.pdf; the live UCL and CORE copies sit behind Cloudflare). This is the main source of this note. |
| Compton, Radmore & Giblin, "A stochastic model of the dynamics of an ensemble of spheres", *Proc. R. Soc. A* 453:1717–1731 (1997), doi:10.1098/rspa.1997.0092 | Not read. It is thesis ch. 5, which was read. |
| Compton, Giblin & Radmore, "Measurements on an ensemble of spheres in the transonic velocity regime", *IEE Proc. Sci. Meas. Technol.* 145:8–12 (1998), doi:10.1049/ip-smt:19981642 | Not read. It is thesis ch. 2–3. |
| Giblin & Compton, *A ballistics measurement system to assist the development and evaluation of non-toxic shot*, DoE report, Mar 1996 | Not found. Partly reproduced in BASC 2008 (patterns.md). |
| **E. D. Lowry, "The Effect of a Shot String?", *American Rifleman*, Nov 1979, pp. 36–39** | **Read [P]** as a scan on randywakeman.com. It contains Tables A, B and C. |
| Lowry, spark shadowgraphs (Winchester), Figs 3–4 on randywakeman.com | Images viewed [P]. They carry no scale bar. |
| Lowry, "Aerodynamic performance of lead and iron shotshell loads", Olin, Feb 1970; Lowry, MSc thesis 1981 | Not found. Cited by Compton. |
| **Davidson, Thomson & Birkbeck, "Computational modelling of the shot pattern of a sporting shotgun", *Sports Engineering* 5:33–42 (2002)** | **Read [P]** from Glasgow eprints. |
| **Bob Brister, *Shotgunning: The Art and the Science* (Winchester Press, 1976)** | **Read [P]**: "The Shot String Story", pp. 275–292, and related passages. Text from an archive.org community upload. |
| W. F. Donovan, *Retardation and condition of shot pellets in free flight*, BRL TN 1653 (1967) | Not found on DTIC. Known only through Compton [S]. |
| Oberfell & Thompson, *The Mysteries of Shotgun Patterns* (1957/1960) | Not online. Secondary accounts only. |
| Burrard, *The Modern Shotgun* | Restricted lending on archive.org. Known only through Brister and Lowry [S]. |
| Journée, *Tir des fusils de chasse* | Not found. Known only through Compton [S]. |
| Boyer, Marshall, Trzicak & Rowe, "Experimental evaluation of the distribution of the pellets in shotgun pellet patterns", *FSI* 42:51–59 (1989), doi:10.1016/0379-0738(89)90198-9 | Citation confirmed. Abstract not obtainable [U]. |
| Walsh, AFTE 2007 (pellet distribution and deformation) | Product page only [U]. See patterns.md. |
| Leeming & Szmelter, "Factors affecting the dispersion of shotgun pellets in short-range combat" (Argos, 2006); Szmelter & Leeming 2000 (CFD of pellet wakes) | Abstract only. It has no numbers. |
| Doppler radar or high-speed studies by Weibel, Infinition, Hornady or Federal | **Nothing quantitative found.** A Weibel radar was used for Pinnacle loads, but the account gives no numbers [S Wildfowl/Brezney]. Federal FliteControl was filmed, qualitatively [S F&S 2014]. |
| Military or police buckshot studies | Nothing with distribution statistics found. AAFS 2020: 00-buck spread against distance R² = 0.97 [S]. Buffered against unbuffered 00 buck: see patterns.md §4. |

---

## 2. Pellet speeds within one shot (Q1)

### 2.1 What was measured

- **Lowry**, quoted by Compton [S Compton96 §1.5.2]:
  - Lowry filmed the arrival of every pellet through a 30-in aperture at 6500 frames/s, at 20–50 yd.
  - From those films he fitted deceleration constants to the "leading edge" (the first 10 % of
    pellets to arrive) and to the "trailing edge" (90 % arrived).
  - Only extreme chokes were tested, with few rounds.
- **Lowry 1979** [P]:
  - The "average pellet velocity" at 40 yd was 738 ft/s for standard 1¼ oz #4, 775 ft/s for buffered
    1½ oz #4 and 707 ft/s for 1⅛ oz #2 steel (Table C).
  - "The pellets in the tail of the shotstring deliver much less energy … the difference in velocity
    is too small to make a substantial difference."
  - He names the main cause of stringing: "the way the pellets individually leave the protection of
    the undispersed shot column during the first few yards of travel."
  - Variation in pellet size and shape is "only a partial cause".
- **Compton, leading and trailing edges of the main cloud**, from cube-law fits over 20–50 m
  [P Fig 4.4, eye ±10 m/s]:

  | Load | 20 m, leading / trailing | 50 m, leading / trailing |
  |---|---|---|
  | 36 g #4 lead (3.09 mm) | 255 / 228 m/s (−11 %) | 165 / 150 (−9 %) |
  | 24 g #7½ lead (2.32 mm) | 250 / 213 (−15 %) | 148 / 127 (−14 %) |
  | 30 g #4 steel (3.33 mm) | 255 / 238 (−7 %) | the two cross at about 42 m |
  | 28 g #8 zinc (2.19 mm) | about 225 / about 170 (about −24 %) | about 100 / about 82 |

- **Compton, near-muzzle leading-edge velocity at 2.5 m** (10 shots, SE in brackets)
  [P Table 3.6]:

  | Load | 0.000″ | 0.020″ | 0.040″ |
  |---|---|---|---|
  | 32 g #3 steel | 361 (0.8) | 363 (2.2) | 369 (1.8) m/s |
  | 28 g #1 steel | 430 | 431 | 437 |
  | 30 g #3 zinc | 389 | 390 | 397 |
  | 28 g #5 zinc | 418 | 420 | 426 |

  - So 0.040″ gives +1.5–2 % at the leading edge. Compton calls the choke effect "minor".
  - The lead rows of this table did not survive text extraction.
- **Compton, choke and downrange speed**: a 0.040″ choke gives leading-edge velocities "typically
  5 %" higher at 20–50 m than 0.000″, for lead, steel and zinc. Compton puts this down to slipstreaming
  in the longer, narrower column [P §3.3.2, Fig 3.8].
- **Compton, size variation**: pellet diameter varies by **2.7 % (lead), 1.8 % (steel), 2.8 %
  (bismuth), 3.9 % (zinc)** (sample SD / mean, mass-produced loads) [P Table 3.1].
  - Pellet count varies by 1.0–1.4 %, load weight by 0.9–1.1 % and powder by 0.6–1.0 %.

### 2.2 Shape of the distribution

- **Longitudinal density.** Compton fitted Gaussian, Rayleigh, Gaussian·t, Gaussian·t² and
  Maxwell–Boltzmann curves to the energy-calibrated arrival profiles [P §3.3.4].
  - The Rayleigh "visually gave the best fit".
  - The Gaussian had the smallest residual, but it is symmetric, "unlike the nature of the
    longitudinal distribution".
  - Compton's conclusion: the longitudinal distribution is "best described by a Rayleigh
    distribution". The effect of choke on it at 20–50 m is minimal.
- **Lowry's Table A** (40 yd, 12 ga 2¾″ 3¾ dram eq., 1¼ oz #4) [P]:

  | | 50 % | 80 % | 90 % | 95 % |
  |---|---|---|---|---|
  | Distance from the front pellet that includes this share | 58 in | 88 | 105 | 120 |
  | Shortest length that includes this share | 43 | 80 | 100 | 117 |
  | Rayleigh, σ = 49.1 in, from the front [D] | 58 | 88 | 105 | 120 |
  | Rayleigh, σ = 49.1 in, shortest [D] | 43 | 80 | 100 | 117 |

  - The Rayleigh fits exactly. Lowry also notes that 60 filmed strings "all had the same
    characteristic shape with dimensions in the same proportion" [P].
  - The 80 % shortest length "excludes, roughly, the front 4 % and the rear 16 %" [P].
- **Spencer, after Gough Thomas**: "approximately half the shot is in the front third" of the
  string (patterns.md §6) [S].
  - The Rayleigh puts 50 % in the first 1.18σ of roughly 3.5σ, which is the front third [D].
  - Brister's photo caption agrees: "heavy concentration of pellets in front; lagging pellets in
    rear" [P Brister76 p. 289].

### 2.3 Why the coil-versus-front gap is not the speed spread

- Lowry's Table 3 (flight.md §2.1): front pellets are 6–11 % faster than the 3-ft coil reading [P].
- At 3 ft the column has not broken up: "only the front pellets are in free flight immediately after
  muzzle exit, and it is their initial velocity that determines the downrange performance of the
  whole shot charge" [P Lowry/Wakeman].
- The pellets behind slipstream and catch up. This is Lee's two-sphere drag data via Compton §1.5.1:
  - a trailing sphere in line has much less drag out to 3.5 diameters;
  - spheres abreast have up to +50 % drag at contact.
- So the *effective* free-flight start speeds have a much smaller spread than the coil gap. §2.4
  fits it at about 3 % for soft lead.

### 2.4 What the string lengths imply: effective speed and drag spreads [D]

**Method.**

- The engine's sphere-drag model flew pellets with a start-speed shortfall `dv` and a drag factor
  `f`. The drag factor was emulated by dividing the density by `f` .
- I sampled `dv = a·|z1|` and `f − 1 = b·|z2|` with z ~ N(0,1), both half-normal. Deformation and
  lagging only ever slow a pellet.
- I then fitted `a` and `b` to the measured lengths.
- The sum of two half-normals has zero density at the front and a long tail, as the Rayleigh does.
- Lag per unit cause, for #4 lead (3.30 mm, v0 = 430 m/s), in metres behind the front pellet:

  | Cause | 20 yd | 30 yd | 40 yd | 65 yd |
  |---|---|---|---|---|
  | 1 % slower start | 0.13 | 0.17 | 0.20 | 0.25 |
  | +10 % drag | 0.29 | 0.55 | 0.87 | 1.92 |

  - **A speed spread makes a string that stops growing. Only a drag spread makes one that keeps
    growing.**

**Fits.**

| Data | a (speed, half-normal scale) | b (drag, half-normal scale) | Fit |
|---|---|---|---|
| Lowry Table C, standard 1¼ oz #4 soft lead, L80 at 40–65 yd | 3.25 % | 0.14 | rms 1.2 %. Also reproduces Table A: model 54/87/106/122 against 58/88/105/120 |
| Lowry, buffered 1½ oz #4 | 1.5 % | 0.11 | rms 0.8 % |
| Lowry, 1⅛ oz #2 steel | 0.25 % | 0.11 | rms 2.8 % |
| Compton Fig 3.10, 32 g #5 lead, 20–50 m* | 4.0 % | 0.10 | rms 3 % |
| Compton Fig 3.10, 32 g #5 steel, 20–50 m* | 6.5 % | 0.02 | rms 2 % |

\* Compton's "length" runs from the first to the last pellet of the main cloud, excluding strays. I
took it as 1.79 × the shortest 90 % length, which is the Rayleigh ratio of the 0.1–99.9 % span to the
shortest 90 %. This scale is my assumption. It is supported by the agreement of his Rayleigh peak
times (σ_t ≈ 5 ms at 40 m) with Lowry's σ ≈ 1.25 m.

**Reading.**

- For soft lead the two data sets agree: a ≈ 3–4 % and b ≈ 0.10–0.14.
- For steel they **conflict**:
  - Lowry's computed lengths keep growing out to 65 yd, which needs a drag spread.
  - Compton's measured steel strings start long and grow slowly (2.5 → 4.3 m from 20 to 50 m), which
    needs a speed spread.
  - Compton explains it: steel is "fairly spherical after firing", so it has little drag variation.
    The longer steel string at 20 m comes from the "greater volume of pellets in flight" [P §3.3.2].
  - Lowry's Table C values beyond 40 yd may themselves be computed.
  - **I would trust Compton for steel.** Use a ≈ 5 % and b ≈ 0.04 as a compromise, and widen the
    tolerances (§10.7).

### 2.5 Answer to Q1, in short

- **Spread:**
  - soft lead: a mean shortfall of about 2.8 % (SD ≈ 2.1 %) behind the front pellets in effective
    start speed, plus about +10 % (SD ≈ 7 %) mean drag excess;
  - buffered or hard lead: about half the speed spread;
  - steel: speed spread about 4–5 % with little drag spread, but the sources conflict.
- **Shape:** skewed, with a hard front edge and a tail (Rayleigh-like). It is not symmetric.
- **Leading against trailing:**
  - at 20 m the trailing edge is about 7 % slower for steel, 11–15 % for lead and about 24 % for
    zinc [P Compton, eye];
  - the 95th-percentile pellet of the fitted lead model is 12 % slower at 20 m and 19 % slower at
    50 m [D, `vel.rs`].
  - Compton's edges converge with range for steel and #4 lead, which the model does not reproduce.
    Compton's per-edge cube-law fits are themselves model-dependent.

---

## 3. Correlation and the cloud's shape (Q2)

### 3.1 Evidence

**For a correlation (front and centre against back and outside):**

- **Compton's bubble-wrap catch** at 40 yd, 0.000″ and 0.040″ chokes, lead, steel, zinc and bismuth
  [P §3.2.1]:
  - The worst-shaped pellets were "in general … striking the outer section of the target, whereas the
    more spherical pellets were found in the pattern centre."
  - Bismuth fragments were spread evenly over the pattern; whole bismuth pellets that were more
    deformed lay at the edge.
- **Compton's 3-D reconstruction** of several hundred clouds (32 detector panels of 30 × 30 cm)
  [P §3.5]:
  - "No one shot cloud is the same as another."
  - But a trend held "irrespective of shot size, material, choke or range": "the pellets arriving
    first (leading edge) are located in the centre of the pattern … and the trailing edge pellets
    appear at the outer regions."
  - His conclusion: "the worst deformed pellets travel at the rear of the shot cloud in outer regions
    of the pattern, and the well-shaped pellets travel at the leading edge in the centre."
- **Journée**, via Compton [S]:
  - He colour-coded layers of the load. Pellets loaded at the *rear* of the cartridge landed "on
    average more dispersed", in the outer regions.
  - He also found that pellets "moved around in the shot cloud with no constant order".
- **Brister** [P p. 190]: deformed shot "tend to go wide of the rest of the pattern, usually trail the
  central shot string".
  - Deformed pellets from the pattern edge, fired into a penetration box, lost "around 15-percent
    penetration, and some pellets lost 30-percent" compared with undeformed centre pellets.
- **Davidson 2002**, DEM simulation: "most of the pellets on the outside of the shot string …
  diverge", producing the stray pellets [P].
- **Donovan**, via Compton [S]: more deformation gave "a greater initial spread".

**Against:**

- **Lowry 1979** [P]: "The location of each pellet impact on the pattern, moreover, is for the most
  part unrelated to its longitudinal location in the shotstring."
- **Walsh 2007**, via patterns.md [U]: deformation was not related to landing position (#4 lead at
  10–15 m). He did find that pellets from the outside of the column land at the edge.
- **Steel**: Compton found its strays *ahead* of the main cloud, not behind [P §3.2.2].

**Quantified?** No source gives a correlation coefficient or a conditional distribution. **[gap]**

### 3.2 Shape

**Near the muzzle** [P Lowry shadowgraphs, Figs 3–4]:

- At 4 ft, a cylinder bore gives a short, wide pancake behind the wad, while a full choke gives a long
  column.
- At 6 ft the full-choke string is "the much longer one". At 14 ft the difference has "diminished".
  At 18 ft "the cylinder-bore string is already at least as long".
- In the photos the full-choke cloud at 14–18 ft is a thin cigar a few inches across. The cylinder
  cloud has already opened into a round disc. **[eye; no scale]**

**Down range:**

- Lowry: "a cigar-shaped cloud" [P].
- Kanstrup: over 2 m long and about 1 m across at 25 m, "elliptic" [S, patterns.md].
- Roster: 3–5 ft across and 6–8 ft long; nontoxic shot at 30–40 yd about 7 ft long and 4 ft across
  [S MT Outdoors].
- Compton: dense front in the centre, sparse trailing rim. In 3-D that is a cone or teardrop with
  its point forward and a bowl-shaped, wider tail [P Fig 3.34, 3.36].
- Shapes vary strongly from shot to shot.

**Proportions** [D]:

- At 40 yd the lateral SD is about 0.25 m for a 70 % pattern, and the longitudinal SD is about 0.8 m
  (Rayleigh SD = 0.655 × 1.25 m).
- So the cloud is about 3× longer than wide in SD terms: a cigar.
- The total length is 11–12 ft for soft lead, against a "spread" of about 3.5 ft (85 % circle).

---

## 4. String length against choke, material, size, velocity and wad (Q3)

### 4.1 Lengths by range

| Range | Load | Length | Definition | Source |
|---|---|---|---|---|
| **20 yd** | Federal 12 ga 3″, 1450 fps | 42 in | not stated | [S F&S] |
| | 12 ga 3½″, 1500 fps | 49 in | | |
| | 12 ga dove, 1235 fps | 55 in | | |
| | 20 ga, 1210 fps | 46 in | | |
| | 20 ga dove, 1165 fps | 57 in | | |
| | .410, 1135 fps | 66 in | | |
| | steel #3, 1600 fps | 61 in | | |
| | lead #6, 1500 fps | 55 in | | |
| **20 m** | Compton 32 g #5 lead / steel / bismuth / zinc | 2.1 / 2.5 / 2.8 / 2.9 m | main cloud, first to last | [P Fig 3.10, eye] |
| | Compton 36 g #4 lead (0.000″ / 0.040″) | 1.8 / 2.3 m | | [P Fig 3.9, eye] |
| | 28 g #7½ lead | 2.6 / 2.2 m | | |
| | 28 g #1 steel | 1.9 / 2.3 m | | |
| | 30 g #4 zinc | about 2.2 m | | |
| **25 m** | Danish summary | over 2 m | | [S Kanstrup] |
| **30 m** | Compton #5 lead / steel / bismuth / zinc | 3.3 / 3.1 / about 3.4 / about 3.9 m | main cloud | [P Fig 3.10, eye] |
| **30–40 yd** | nontoxic | about 7 ft | | [S Roster, MT Outdoors] |
| **40 yd** | standard 1¼ oz #4 | 79 in | 80 % | [P Lowry79] |
| | buffered 1½ oz #4 | 56 in | 80 % | |
| | 1⅛ oz #2 steel | 55 in | 80 % | |
| | Winchester full choke, ordinary hunting loads | about 11 ft; over 9 ft for the middle 90 % | | [P Brister76 p. 276] |
| | #4 lead / #4 Hevi-Shot | 11–12 ft | 100 % | [S HPW] |
| | #4 steel | 8–9 ft | 100 % | |
| | #2 HV steel | 7–8 ft | 100 % | |
| | 1⅛″ (3.2 mm) hardened steel balls, cylinder or full choke | 50 in | 80 % | [P Lowry79] |
| | Quayle 1928, spark photographs | 12 ft | front to back | [P Brister76] |
| | Burrard 1920s, computed | 4.5 ft full choke, 5.5 ft IC | "main bulk" | Brister calls it about half the real length [P Brister76] |
| | Olin 1920s | up to 25 ft | | [P Brister76] |
| | Compton, 40 m, #5 lead / steel / bismuth / zinc | 4.5 / 3.65 / 3.85 / 4.9 m | main cloud | [P eye] |
| 45 / 50 / 55 / 60 / 65 yd | standard #4 lead | 94 / 109 / 124 / 139 / 155 in | 80 % | [P Lowry79 Table C] |
| | buffered #4 lead | 67 / 79 / 91 / 104 / 118 in | 80 % | |
| | #2 steel | 67 / 80 / 95 / 112 / 129 in | 80 % | |
| 50 m | Compton #5 lead / steel / bismuth / zinc | 5.7 / 4.3 / 4.4 / about 5.9 m | main cloud | [P eye] |
| 70 yd | most efficient loads, on Brister's 16-ft board at 40 mph | "almost the length of my 16-foot board" | | [P Brister76] |

### 4.2 Effects

**Choke:**

- Near the muzzle a full choke makes a longer string. The cylinder-bore string has caught up by
  14–18 ft [P Lowry].
- Lowry's Figure 3 caption: "the cylinder choke charges string out more as ranges increase" [P].
- Compton, from 150 batches: "no specific relationship has been found between choke constriction and
  shot cloud length" at 20–50 m [P].
- Compton, ch. 5 fit for 28 g #4 steel: tightening 0.010″ → 0.030″ cuts the transverse launch
  variance by 62 % (A from 2.29 to 0.87 × 10⁻⁴). It raises the longitudinal launch variance by 17 %
  (α from 4.85 to 5.69 × 10⁻⁵) and the in-flight diffusion by 20 % (B) and 7 % (β) [P Table 5.1].
- Soft lead loses more density to stringing through open chokes. With very hard shot there is
  "little difference" between chokes [P Brister76].
- Burrard [S via Brister76]:
  - average crossing-target loss of 30 % with IC against 13 % (p. 276) or 17 % (p. 286) with full
    choke, at 40 yd;
  - 1920s soft shot, 12 ga, No. 6, mostly at 30 mph.
- An oversized ("Russian") choke made the string about one third longer with skeet loads that had no
  shot cup [P Brister76, photo caption in the choke chapter].

**Material and hardness:**

- Steel strings are shortest; Roster says about half as long as lead [P Compton96; P Lowry79; S HPW;
  S Roster].
- Bismuth fragments trail several metres behind by 35 m [P Compton96].
- Zinc is longest [P Compton96].
- Buffered or plated hard lead is about 30 % shorter than soft lead (56 against 79 in at 40 yd)
  [P Lowry79].
- Flour-buffered 10 ga 2 oz #2 at 50 yd and 40 mph: 117 against 76 pellets in the 30-in circle [P
  Brister76].

**Pellet size:**

- "The smaller the shot the more the stringing" [P Brister76].
- Compton: #7½ lead about 6.5–6.8 m at 50 m, against 4.6–4.9 m for #4 lead [P eye].

**Velocity:**

- No clean controlled data.
- Federal's faster premium loads were shorter at 20 yd (42–49 in against 55–57 in for dove loads)
  [S F&S]. Hull, buffering and shot hardness also differ between those loads.
- Brister: high-velocity soft-shot loads "strung out halfway across the board at 50 yards" [P].

**Wad and buffer:**

- Buffering or cushioning gives the shortest lead strings [P Brister76; P Lowry79].
- Lowry: stringing comes mostly from how pellets leave the column, more than from size and shape
  [P].

---

## 5. Deformation (Q4)

**Shares.** No count of the share of deformed pellets was found. **[gap]**

- Davidson 2002: firing without striking a solid target shows "only a very small number of pellets
  are permanently deformed during the internal ballistics" [P]. No number is given, and the
  load was 30 g No. 6.
- Lowry: set-back gives "large, saucer-shaped depressions in the lead pellets" [P via flight.md].
- Compton: the fired lead, zinc and bismuth show "greater variation in shape and size" than unfired
  pellets. Steel stayed "approximately spherical" [P §3.2.1, Figs 3.1–3.4].

**What makes it worse:**

- **Choke**: "with the exception of steel pellets … the greater the constriction, the worse their
  shape became" (0.000″ against 0.040″) [P Compton96].
- **Hardness and buffering**: deformation is reduced by harder, plated or buffered shot
  [P Brister76; P Lowry79].
  - Antimony levels for "hard" shot (Roster, per manufacturers' practice): buckshot ½–1 %, BB–2:
    1–2½ %, 4–6: 3–4 %, 7–8: 5–6 % [S Shotgunlife].
  - Under 0.5 % is "soft" (SAAMI, as cited) [S].
- **Payload relative to bore**: "the longer and heavier the payload … relative to the size of the
  bore — the greater the shot deformation" [P Brister76].
- **Velocity**: no controlled data (patterns.md §4).

**Drag of deformed pellets:**

- Donovan, BRL TN 1653 (1967), via Compton [S]:
  - slight damage gives a "small increase in C_D";
  - **major deformation gives 20–30 % higher drag**;
  - some pellets in the cloud had *lower* drag than the front pellets, from slipstreaming.
- Compton's deceleration constants, cube law, 1/k₃ against d in mm [P eq. 4.21–4.22]:

  | | Leading edge | Trailing edge |
  |---|---|---|
  | Lead | 1/k₃ = 5280 d − 310 | 1/k₃ = 4280 d − 140 |
  | Steel | 1/k₃ = 3780 d − 70 | 1/k₃ = 3790 d − 690 |

  - Lead's trailing edge decelerates **22 % harder** than its leading edge for d = 2–4 mm [D].
  - Steel's trailing edge decelerates about **5 % harder** at 3 mm [D].
- The fitted drag-spread scale b ≈ 0.10–0.14 for soft lead (§2.4) [D]:
  - about 10 % of pellets would have more than +20 % drag;
  - about 1 % would have more than +30 % drag.
  - This is consistent with Donovan's band for "major deformation".

**Do deformed pellets fly wider?** Yes, by every qualitative source except Walsh
[P Compton96; P Brister76; S Donovan; S Journée] (§3.1). No size of the effect was found.

**Measured flier shares.** None found. **[gap]**

- Compton: the first and last arrival times (t₁, t₄, which include strays) vary at least twice as
  much as the edges of the main cloud, "due to the erratic occurrence of the stray pellets" [P §3.2.2].
- Compton's radial fits: in compact patterns, the counts in circles over 900 mm (beyond about 1.65σ)
  "start to visually stray" above the Gaussian line, but stay within its error bars [P §3.4.2].
- Brister: pellets "ranging 8 to 12 feet behind the front pellets" on long shots [P].

---

## 6. Clumping and radial density (Q5)

### 6.1 Radial shape

- **Compton**, from "over 2000 patterns" [P §3.4.2]:
  - x and y are each Gaussian and equal to within about 1 % (the standard error). The SDs agree to
    within 8 %.
  - The radial density fits a Gaussian with χ² probability above 99 % "for every combination of
    pellet size, material, range and choke".
  - Open chokes (below 0.020″), and longer ranges, are "less defined". Lowry had also stated that
    open-choke patterns stray from the Gaussian.
  - Journée found the vertical spread greater than the horizontal, but the 'creux' (blown, cartwheel)
    patterns of the old fibre-wad cartridges may explain that. Compton did not see it consistently.
- **σ against choke**, 28 g #7½ lead at 40 yd, 10 patterns each [P Table 3.8]:

  | Choke | 0.000″ | 0.010″ | 0.020″ | 0.030″ | 0.040″ | 0.050″ |
  |---|---|---|---|---|---|---|
  | σx (mm) | 329 | 296 | 262 | 243 | 240 | 244 |
  | σy (mm) | 339 | 297 | 263 | 242 | 244 | 236 |
  | % in 30 in | 35.1 | 48.6 | 61.0 | 68.6 | 68.4 | 70.1 |

  - The percentage stops rising above 0.030″ for every material.
  - The open-choke σ is biased low because the 1.5-m paper misses pellets.
- **σ against flight time**: the radial σ is proportional to flight time, not range [P eq. 3.23–3.24].
  - Fits, with σ in mm and t in s:
    - #BB lead 0.010″: σ = 2560t + 1.2;
    - #BB lead 0.030″: σ = 2180t − 5.5;
    - #3 steel 0.010″: σ = 1440t + 10.3;
    - #3 steel 0.030″: σ = 1500t − 5.0.
  - Compton found the linear law "not … affected by velocity".
  - [D] For typical shot, t/t₄₀ at 20/30/50/60 yd is 0.41/0.68/1.37/1.79 (#6 lead, 400 m/s). The
    engine's Full-choke law gives 0.42/0.69/1.35/1.75. **Compton's σ ∝ t is essentially the engine's
    existing q-law for tight chokes**, and gives it a physical form for §10.
- **A slightly peaked core?** [D] Compton's Table 3.14 (36 g #4 lead,
  0.030″, 40 yd, 10 shots):

  | Circle | Mean count | Gaussian fitted to the 30-in count | Excess |
  |---|---|---|---|
  | 5 in | 9.2 | 6.7 | +37 % |
  | 10 in | 32.6 | 25.4 | +28 % |
  | 30 in | 143.7 | (fitted) | |

  - The 10-in excess is about 3 standard errors.
  - A lognormal *per-pellet* scale mixture σᵢ = σc·exp(0.37 z) fits all three counts exactly. But it
    predicts a 20/30-in ratio of 0.656, above Gough Thomas's 0.57–0.62 for Full.
  - A mixture spread of s ≈ 0.20–0.25 is the most that Gough Thomas allows (20/30 = 0.61–0.62).
  - Compton does not say how his 5- and 10-in circles were centred. Placing them on the densest spot
    would inflate the counts.
  - **Treat the peaked core as possible but unproven.**

### 6.2 Clustering

- **Compton, Poisson test** [P Fig 3.33]:
  - Patterns split into 5 equal-area rings × 12 segments, each the area of a 100-mm circle.
  - P(0), P(1), P(2) and P(3) against the mean count followed the Poisson lines for 36 g #4 lead,
    0.000–0.050″, at 40 yd. Appendix E repeats this for other loads.
  - **No sign of local clustering.**
- **Compton, Table 3.14 dispersion** [D]:
  - The 30-in count SD is 9.56 against a binomial 6.7. The excess is explained by a σ CV of about
    4.5 %.
  - The 10-in SD is 6.92. Simulated Poisson plus 4 % σ CV gives 5.1, and 6 % gives 5.6.
  - The 5-in SD is 4.02 against a simulated 2.6.
  - Variance index at 5 in: 16.2/9.9 = 1.6. With 10 shots, χ²₉ gives p ≈ 0.1. **Not significant.**
- **Gough Thomas's empty 5-in patches**: real patterns sit at or just above a Poisson Monte Carlo at
  150–250 pellets (patterns.md §3.4, tentative).
- **Compton's view**: clumping "needs to be investigated as there is no complete mathematical
  explanation" [P §6.3].
- **Mechanisms that do make clumps**:
  - balled shot fused by hot gas leaking past fibre wads [S ShootingUK, fourten.org.uk];
  - "blown", patchy cylinder patterns at 10 m in the DEM [P Davidson02].

---

## 7. Shot-to-shot variation (Q6)

| Quantity | Value | Source |
|---|---|---|
| SD of pellet x and y coordinates, 10 patterns, 40 yd | CV 5–10 %, "fairly constant between material and independent of choke and range" | [P Compton96 §3.2.3, Fig 3.6] |
| Same, after removing finite-pellet sampling noise | **≈ 3.5–4.5 %**. From Table 3.14 (30-in excess over binomial) and Table 3.8 (σ SEs of 3.6–5.4 mm on 236–339 mm at n = 10, minus √(1/2N) ≈ 3.7 %) | [D] |
| 30-in count CV, 40 yd | 5–12 % (Fig 3.6c). Table 3.14 gives 6.6 % (154 147 156 149 134 141 147 123 149 137) | [P] |
| Count CV against circle size | about 3× larger in a 10-in circle than in a 30-in one (21 % against 6.6 %); 44 % in 5 in | [P] |
| % in 30 in, SD per shot, #7½ lead | 3.2–4.7 points (SE 1.0–1.5 at n = 10) | [P Table 3.8 ×√10] |
| Leading-edge velocity CV near the muzzle | lead 1.1 % (0.6–2.1), steel 1.7 % (1.0–2.5), bismuth 1.6 %, zinc 1.2 % | [P Table 3.2] |
| Flight-time CV, 20–50 m, main-cloud front t₂ / back t₃ | lead 1.2 / 1.5 %, steel 1.8 / 1.5 %, bismuth 1.5 / 2.0 %, zinc 1.4 / 1.5 % | [P Table 3.4] |
| No. 7 lead and BB lead | see patterns.md §3.5 | [S BASC 2008] |
| Oberfell & Thompson | about 10 % variation (same shell brand and gun) | [S Chuck Hawks] |
| Weldin 1927 | 19–95 % (4000 patterns) | [S Wakeman] |
| 15 shots, 1¼ oz #2 steel, Modified | 96–132 in 30 in | [S Wakeman] |
| Reliable average | 10 shots; the average converges after 5 | [P Compton96 §3.2.4] |

**Pattern centre against point of aim:** no shot-to-shot measurement found. **[gap]**

- The sampling floor is σ/√N: about 1.7 cm at 40 yd for 200 pellets [D].
- Muzzle-velocity variation shifts the vertical impact by only about 3 mm at 40 yd [D: 1.5 % on v
  gives about 3.4 % on the ≈ 10 cm drop].
- Wakeman's "up to 4–5 in at 40 yd" refers to the crossing-target shift, not shot-to-shot scatter
  [S].

---

## 8. The wad and the gas (Q7)

- **Separation** [P Compton96 §3.3.1, Fig 3.7, eye; 36 g #3 lead at 0.030″, 32 g #3 steel at
  0.020″, 10 shots each]:
  - The skyscreen silhouettes show wad and shot "inseparable at 1 m". The wad becomes distinguishable
    "from 2 m onwards".
  - At 8 m the lead wads were about 0.6–2.5 m behind the front of the shot (3–14 ms).
  - The steel wads were 0.2–2.1 m behind. Three of them separated only "very gradually", 0.2–0.4 m
    at 8 m.
  - Some steel wads were recovered with pellets embedded in the base, "giving them extra momentum".
- **Other accounts of separation:**
  - The cup petals open at about 1 m [P Davidson02].
  - The Remington Power Piston released its load "a few feet from muzzle" [P Brister76 p. 288
    caption].
  - Federal FliteControl holds the shot "several feet" [S F&S 2014].
- **Does the wad disturb the pattern?**
  - Compton: wads within one batch varied (design, cushioning, number and depth of cuts). "The
    variation in the wads had a great effect on the patterns."
  - He saw wads travel "with the shot for a long period", "spin off at strange trajectories … possibly
    spilling the pellets" [P].
  - Not quantified.
  - The engine can treat this as part of the shot-to-shot σ variation.
- **Gas.** Gas leaking past fibre wads fuses pellets: "balled shot", seen as clumps on the plate [S].
- **Old patterns.** The old 'creux' (blown, cartwheel) patterns with an empty centre came from
  pre-plastic cartridges [S Compton96 on Journée].
- **Muzzle gas.** Compton notes, citing Farrar & Leeming, that muzzle gas can slightly accelerate a
  projectile over a few calibres. He ignores it as small [P].
- **Chamber length** matters [P Compton96 Table 3.10]:
  - 2¾″ cartridge, 28 g #7½, 0.030″, % in 30 in: 82.4 % in a 2½″ chamber, 75.3 % in 2¾″, 79.4 % in
    3″;
  - σx 198 / 219 / 203 mm.
  - Brister: the chamber effect "isn't enough to matter" [S Wakeman].

---

## 9. Crossing targets (Q8)

### 9.1 Measured or computed losses

| Source | Conditions | Result |
|---|---|---|
| **Lowry 1979, Table C** [P] (computed from filmed strings) | 80 ft/s (55 mph), 12 ga 2¾″, 40 / 45 / 50 / 55 / 60 / 65 yd | **Loss in points**: standard 1¼ oz #4: 5.1 / 5.4 / 5.1 / 4.5 / 3.9 / 3.4. Buffered 1½ oz #4: 3.0 / 4.0 / 4.4 / 4.3 / 4.0 / 3.6. Steel 1⅛ oz #2: 3.4 / 4.7 / 5.6 / 6.0 / 5.4 / 4.1. **Pattern level, %**: standard 72 / 58.2 / 46.2 / 36.4 / 28.8 / 23.0; buffered 87 / 75.0 / 62.3 / 50.7 / 41.0 / 33.2; steel 85 / 73.9 / 62.4 / 51.8 / 42.8 / 29.1 |
| Lowry, Table B [P] | loss against "shotstring factor" F = L80 × v_target/v_pellet (in) | F 4: 0.6–1.4 points. F 8: 2.4–5.6. F 12: 5.0–11.6. F 15: 7.2–16.7. Higher pattern levels lose more. |
| Lowry, worst case [S Wakeman] | 55 mph ducks at 65 yd | 3.4–4.1 % |
| Lowry, worst case [S Wakeman] | 40 yd | #2 steel 3.4 %, buffered lead 3 % |
| Lowry's moving-sheet illustration [P] | 55 mph | the 30-in circle becomes a 30 × 36¾ in ellipse; 75 % → 70 % |
| Burrard [S via Brister76] | 30–40 mph, 30–40 yd, No. 6 | average loss 30 % (IC) and 13–17 % (Full) at 40 yd. "Very little elongation" of the pattern. |
| **Brister** [P] | 16-ft board, 40 mph | Hard-shot loads at 40 yd were indistinguishable from stationary. Roster's buffered Lubaloy 1½ oz lost **7.4 %** against **20.5 %** for a soft magnum ("stringing some of its shot 7 feet behind"). No. 6 HV at 50 yd lost **9 %**. No. 4 HV duck load at 50 yd: 85 → 61 pellets (50 % → 36 %). 3″ 20 ga 1¼ oz #4 at 40 yd: 56 % moving. A heavy 12 ga magnum at 50 yd: 33 %, then 8 % on the next pair, "difficult to determine how much … is due to variation of the shell". |
| Brister, via Outdoor Life [S] | 10 ga 1⅝ oz, 60 yd, 40 mph | 74 % still, 54 % moving |
| Federal, via Field & Stream [S] | modern loads, 40 mph bird at 40 yd | about 5 % density loss. Brister's 1976 lead magnums lost about 30 %. |

### 9.2 Leads

| Target and range | Lead | Source |
|---|---|---|
| 80 ft/s at 40 yd | 10 ft | [P Lowry79] |
| 70 ft/s at 40 yd, #2 steel 1365 fps, flight time 0.126 s | 8.82 ft | [P Lowry/Wakeman] |
| 40 mph at 40 yd | "about 7 feet" | [P Brister76] |
| 40 mph at 70 yd | "approximately 18 feet" | [P Brister76] |
| Engine check, #4 lead | 0.122–0.130 s to 40 yd, so 7.2–7.6 ft at 40 mph and 9.8–10.4 ft at 80 ft/s | [D] |

### 9.3 Model against Lowry

- A Gaussian pattern smeared by a Rayleigh string, with the circle re-centred, gives **about half** of
  Lowry's Table B and C losses at every row [D]. For example:

  | Row | Model | Lowry |
  |---|---|---|
  | 40 yd standard | 2.4 | 5.1 |
  | 40 yd steel | 1.6 | 3.4 |
  | 65 yd standard | 1.8 | 3.4 |

- The ratio is 0.47–0.53 throughout, so it is probably a difference in definition. I could not
  resolve it. **[open]**

---

## 10. A model for the engine

The model keeps what the engine already does (sphere drag per pellet; a radial-Gaussian pattern
whose σ(R) is set by the choke's p40, patterns.md §8) and adds structure per pellet. All random
draws are per pellet *i* within shot *s* unless marked "per shot".

### 10.1 Per shot

| Draw | Value | Source |
|---|---|---|
| Front (free-flight) muzzle velocity `V_s = V·(1 + ε_v)` | ε_v ~ N(0, cv_v): cv_v = 1.1 % lead, 1.7 % steel, 1.6 % bismuth, 1.2 % zinc | [P Compton96 Table 3.2] |
| Pattern scale `k_s = exp(ε_σ)` | ε_σ ~ N(0, cv_σ): cv_σ = 4 % for small lead shot (No. 9–4), 5 % steel (no separate figure; Compton says it is material-independent), 15–20 % for BB and larger | [D from P Compton96]; [D from S BASC, patterns.md §3.5] |
| String scale `j_s` on a and b (optional) | lognormal, CV ≈ 10 % | [D, weak: from the t₂/t₃ CVs in Compton Table 3.4, assuming correlation 0.5–0.8] |
| Pattern-centre offset | **0 beyond sampling** (no data). Allow a user-set point-of-impact offset. | [gap] |

### 10.2 Per pellet: start speed, drag and the string

```
v_i  = V_s · (1 − a · |z1_i|)
Cd_i = Cd_sphere(M, Re) · (1 + b · |z2_i|)        z1, z2 ~ N(0,1), independent
d_i  = d · (1 + c_d · z4_i)                       optional size scatter
```

| Shot type | a | b | c_d |
|---|---|---|---|
| Soft lead (≤ 0.5 % Sb, unbuffered) | 3.5 % (3.25–4.0) | 0.12 (0.10–0.14) | 2.7 % |
| Hard, plated or buffered lead | 1.5–2 % | 0.10 (0.08–0.11) | 2.7 % |
| Steel | 5 % (0.25–6.5, sources conflict) | 0.04 (0.02–0.11) | 1.8 % |
| Bismuth, tungsten, other | no data: use hard lead; bismuth c_d = 2.8 % | | |

Sources: [D §2.4]; c_d from [P Compton96 Table 3.1].

- `v_i` is the pellet's effective free-flight start speed. The front pellets (z1 ≈ 0) fly at the
  engine's muzzle velocity, which is already defined as the front-pellet velocity (flight.md §5.4).
- **The string is not drawn. It emerges** by flying every pellet with its own `v_i`, `Cd_i` and
  `d_i`: each pellet's arrival time t_i(R) is its place in the string.
- The size scatter c_d overlaps partly with b, but it is small (a deceleration CV of only about 3 %).

### 10.3 Per pellet: direction

The lateral position at range R uses one 2-D unit Gaussian vector `g_i` for the whole flight, so a
pellet keeps its side of the pattern:

```
r_i(R) = k_s · σ40 · m_i · g_i · [ (1 − w)·R/R40 + w·t_i(R)/t̄40 ]
```

- **w = min(1, q/0.33)**, with q from the existing choke law: 0 for cylinder and 1 for Full.
  - For tight chokes this is Compton's σ ∝ flight time [P eq. 3.24], which matches the engine's q-law
    to about 1 % for typical shot [D, §6.1].
  - For cylinder it is the constant angle that fits Gough Thomas.
  - t̄40 is the swarm's mean flight time to 40 yd. σ40 comes from p40 as now; re-solve it numerically
    if `m_i` is used.
- **The correlation comes free.** A pellet with more drag has a larger t_i(R), so it lands both later
  *and* wider. With w = 1 and b = 0.12, a 95th-percentile-drag pellet is about 5 % wider at 40 yd
  [D]. This matches the qualitative direction of Compton, Brister and Journée.
- **Optional, for a stronger link:**
  - `m_i = exp(s·z3_i − s²)`, a per-pellet lateral scale. s ≤ 0.2 keeps Gough Thomas's 20/30-in
    ratio, and s = 0 by default.
  - z3 is correlated with z2 (drag) at ρ ≈ 0.3–0.5 for soft lead and 0 for steel.
  - **ρ and s are assumptions.** No source quantifies them (§3.1, §6.1).
  - A Gaussian copula like this keeps the swarm's marginal lateral distribution, apart from the mild
    peaking that s > 0 adds.

### 10.4 Fliers

- **No separate flier population.** Fliers emerge from the tails of b, and of s if used:
  - about 1 % of soft-lead pellets get more than +30 % drag and trail about 2.5–4 m at 40 yd [D];
  - Compton found Gaussian fits valid to about 1.65σ, with excess only within the error bars [P].
- No measured flier share exists to justify more. **[gap]**
- Optional, for steel: Compton's strays arrive *ahead* of the cloud [P]. A tiny fraction (under 1 %)
  with z1 = 0 and a σ multiplier of 2 would mimic this. **[assumption]**

### 10.5 Clumping

- **Default: none.** Pellet positions are independent given the shot's σ, which is a Poisson process.
- Justified by Compton's cell counts [P Fig 3.33] and by the Table 3.14 dispersion [D, §6.2].
- Do **not** add a cluster process unless a Gough Thomas patch test (patterns.md §3.4) fails by more
  than its range.
- "Balled shot" (fibre wad, gas blow-by) is a defect mode, not a default.

### 10.6 Hits on moving targets

- Fly each pellet to the target plane. Test it against the target's position at that pellet's own
  t_i: `x_T(t_i) = x_T0 + v_T·t_i`.
- This reproduces Lowry's tilted "cylinder" (Fig 4) without any special string term. It replaces the
  smear approximation of patterns.md §8.2.6.

### 10.7 Tests

| # | Test | Reference data | Tolerance |
|---|---|---|---|
| 1 | **String shape**, 40 yd, 1¼ oz #4 soft lead (3.30 mm, front ≈ 440 m/s) | Lowry Table A: from the front pellet, 50 / 80 / 90 / 95 % within 58 / 88 / 105 / 120 in; shortest intervals 43 / 80 / 100 / 117 in | ±12 % on each. The defaults give 51 / 80 / 97 / 112 [D]. |
| 2 | **L80 at 40 yd** | Lowry Table C: standard 79 in, buffered 56, steel 55 | ±20 %. Defaults: 73 / 47–57 / 57 [D]. |
| 3 | **Growth 40 → 65 yd**, soft lead | Lowry: L80 79 → 155 in (×1.96) | ratio 1.6–2.2. Defaults: ×1.92 [D]. |
| 4 | **Growth 20 → 50 m** (main-cloud length taken as 1.79 × the shortest 90 %) | Compton Fig 3.10: lead #5 2.1 / 3.3 / 4.5 / 5.7 m; steel #5 2.5 / 3.1 / 3.7 / 4.3 m | ±20 % at each range for lead. Defaults: 2.1 / 3.3 / 4.9 / 6.6, so +16 % at 50 m. Steel ±25 %. Defaults for steel are 2.0 / 2.7 / 3.4 / 4.1; **do not** also demand Lowry's steel growth beyond 40 yd. |
| 5 | **Time profile**, #3 lead (3.3 mm, 440 m/s) | Compton Fig 3.16: Rayleigh peaks about 2, 4, 5, 7 ms behind the front at 20 / 30 / 40 / 50 m | ±1.5 ms. Defaults: 2.2 / 3.2 / 5.8 / 7.8 [D]. |
| 6 | **Leading against trailing velocity** at 20 m | Compton Fig 4.4: −7 % (steel) to −15 % (small lead) for the trailing edge | Speed of the 90–95th-percentile-lag pellet relative to the front: −6 % to −16 % for lead, −4 % to −10 % for steel. |
| 7 | **Lateral law** (existing tests stay) | Gough Thomas Tables A and C; plus Compton Table 3.8: #7½ lead at 40 yd gives 35 / 49 / 61 / 69 / 68 / 70 % for 0.000–0.050″, flat above 0.030″ | ±5 points from 0.010″ up. 0.000″ only as "below 45 %" (paper loss). |
| 8 | **σ ∝ t** for w = 1 | Compton eq. 3.23–3.24: σ(R)/σ(40 yd) equals t(R)/t(40 yd) | within 3 % at 20–60 yd |
| 9 | **Shot-to-shot counts**, 36 g #4 lead (≈ 209 pellets), 0.030″, 40 yd, circles centred on each pattern's own centre | Compton Table 3.14: 30 in 143.7 ± 9.6; 10 in 32.6 ± 6.9; 5 in 9.2 ± 4.0 | SD of the 30-in count 7.5–12.5. The 10-in SD within ±35 % (10-shot sample). Mean 10- and 5-in counts: the model will run **20–30 % low** unless s > 0. Report this, don't fail it. |
| 10 | **Poisson cells** | Compton Fig 3.33: in 100-mm cells of equal mean density, ln(1/P(0)) ≈ mean | within ±0.3 for means up to 4 |
| 11 | **Patches** | Gough Thomas empty 5-in patches (patterns.md §3.4) | inside the GT range, or at most 1 below it |
| 12 | **Crossing loss** (pattern level in a re-centred 30-in circle, best spot) | Lowry Table C: 3.0–6.0 points | model 0.4–1.2 × Lowry (expect about 0.5×, §9.3). Brister's and Burrard's larger losses (7–33 %, old soft loads, single shots) are an upper bound only. |
| 13 | **Leads** | Lowry: 10 ft at 80 ft/s and 40 yd. Brister: 7 ft at 40 mph and 40 yd; 18 ft at 70 yd. | ±10 % at 40 yd; ±20 % at 70 yd (the load is uncertain) |

---

## 11. Gaps and open points

1. **A per-pellet start-speed histogram.** Every number in §2.4 is an inverse fit to string lengths.
   A radar or high-speed study that tracks individual pellets would settle a and b. Compton proposed
   exactly that and I found nothing published since.
2. **A correlation coefficient between lateral position and lag or deformation.** It is qualitative
   only, and Lowry dissents.
3. **The steel conflict**: Lowry's lengths keep growing out to 65 yd; Compton's grow slowly.
4. **Flier share and deformed share**: no counts found. Donovan's BRL TN 1653 (1967) would be the
   primary for deformation drag, but it is not on DTIC as far as I could find.
5. **Shot-to-shot movement of the pattern centre**: no data.
6. **Lowry Table B against a Gaussian × Rayleigh model**: a constant factor of about 2 is
   unexplained.
7. **The peaked core** (Compton Table 3.14) against Gough Thomas's 20/30-in ratios: unresolved.
8. **Boyer et al. 1989** (FSI 42:51–59), reportedly finding non-Gaussian 00-buck and No. 2 patterns:
   abstract not obtained.
9. **Buckshot**: no 3-D structure data. BB-size shot-to-shot σ CV of 15–20 % (BASC) is the only
   large-shot number.

---

## 12. Sources

- **[Compton96]** D. J. Compton, *An experimental and theoretical investigation of shot cloud
  ballistics*, PhD thesis, University College London, Dept. of Electronic and Electrical Engineering,
  Nov 1996. https://discovery.ucl.ac.uk/id/eprint/1382490/
  - Read from the archived PDF:
    http://web.archive.org/web/20151026055942/http://discovery.ucl.ac.uk/1382490/1/396689.pdf
  - Sections used: §1.3.4–1.5, §2.6, §3.2–3.6, §4.2, §4.4, §5.2, §6.
  - Through it [S]: Donovan W.F., BRL TN 1653 (1967); Lee K.C., *Aeronaut. Q.* 1978; Journée,
    *Tir des fusils de chasse*; Lowry 1970 Olin report.
- **[CRG97]** Compton, Radmore & Giblin, *Proc. R. Soc. Lond. A* 453:1717–1731 (1997),
  doi:10.1098/rspa.1997.0092 (not read). **[CGR98]** Compton, Giblin & Radmore, *IEE Proc. Sci. Meas.
  Technol.* 145:8–12 (1998), doi:10.1049/ip-smt:19981642 (not read).
- **[Lowry79]** E. D. Lowry, "The Effect of a Shot String?", *American Rifleman*, Nov 1979,
  pp. 36–39, Tables A–C. Scan: https://www.randywakeman.com/lowryshotstring.pdf
- **[Lowry shadowgraphs]** https://randywakeman.com/EdLowryOnShotshellBallistics.htm (Figs 1, 3, 4)
- **[Lowry/Wakeman]** https://www.randywakeman.com/TheEssentialEdLowry.htm ;
  https://www.randywakeman.com/ShotshelPelletBehavior.htm
- **[Davidson02]** R. Davidson, R. D. Thomson & A. E. Birkbeck, "Computational modelling of the shot
  pattern of a sporting shotgun", *Sports Engineering* 5:33–42 (2002),
  doi:10.1046/j.1460-2687.2002.00089.x.
  http://eprints.gla.ac.uk/726/1/Sports_Engineering5%281%2933-42.pdf
  - Field trial: 30 g No. 6, Baikal SxS, cylinder against full choke, 5–20 m.
  - 90 % pattern diameter, Full against Cyl: 110 / 280 / 380 / 540 mm against 170–180 / 370 / 580 /
    715 mm at 5 / 10 / 15 / 20 m (Fig 5 [eye], one shot per point).
- **[Brister76]** Bob Brister, *Shotgunning: The Art and the Science*, Winchester Press 1976,
  "The Shot String Story" pp. 275–292, and pp. 149–151, 189–190, 288.
  https://archive.org/details/shotgunning-the-art-and-the-science
- **[Outdoor Life]** https://www.outdoorlife.com/guns/bob-brister-shotguns/
- **[F&S]** Field & Stream, "The Truth About Shotgun Ammo" (Federal: Carlson, Moser), as cited in
  patterns.md. **[F&S 2014]**
  https://www.fieldandstream.com/blogs/gun-nuts/2014/04/high-speed-video-shooting-federal-shotgun-loads/
  (search summary only).
- **[HPW / Shooting Times]**
  https://www.shootingtimes.com/editorial/long-shot-string-vs-short-shot-string/100155 (via
  patterns.md).
- **[MT Outdoors / Roster]**
  https://fwp.mt.gov/binaries/content/assets/fwp/montana-outdoors/2013/shotgunclinic.pdf (via
  patterns.md).
- **[Kanstrup]** Kanstrup & Thune Andersen 2010, Dansk Jagtakademi (via patterns.md).
- **[Chuck Hawks]** https://chuckhawks.com/shot_string.htm (Wakeman; O&T, Weldin, Burrard summaries).
  **[Wakeman imprecision]**
  https://www.randywakeman.com/ShotgunPatternsandtheirSpectacularImprecision.htm
- **[Shotgunlife]** https://shotgunlife.com/go/3z8q (Roster on antimony).
- **[ShootingUK]** https://www.shootinguk.co.uk/gear/shot-balling-how-it-can-affect-your-shooting-performance-and-how-to-prevent-it/ ;
  https://www.fourten.org.uk/tw_Blown_%20Patterns%20and%20_Balled%20Shot_.pdf (search summaries).
- **[Wildfowl/Brezney]** https://www.wildfowlmag.com/editorial/tips_strategies_wf_georgia_0509/280849
  (Weibel radar, qualitative).
- **[Win. patent]** US 8,171,849 B2 (cut-wire against spheres; core "generally associated with a
  Gaussian shot distribution").
- **[Deng]** S. Deng, "The Shotgun Pellets Interior Ballistics Analysis by Discrete Element Method",
  16th Int. LS-DYNA Conf. (2020), archived PDF; source of the Boyer 1989 citation.
- **[Boyer89]** D. A. Boyer, L. D. Marshall, L. M. Trzicak & W. F. Rowe, *Forensic Sci. Int.*
  42(1–2):51–59 (1989), doi:10.1016/0379-0738(89)90198-9 [U].
- **[Leeming & Szmelter 2006]**
  https://www.argospress.com/articles/2006/factors-affecting-the-dispersion-of-shotgun-pellets-in-short-range-combat
  (abstract only).
- Earlier notes: `docs/shotgun-research/patterns.md`, `flight.md` (GT/Spencer, BASC 2008,
  Kerkhoff 2024, AAFS abstracts, Walsh 2007).
