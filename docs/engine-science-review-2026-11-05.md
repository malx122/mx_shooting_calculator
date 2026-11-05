# Exterior-ballistics engine: science review, 2026-11-05

Scope: `crates/ballistics-core/src/` (solver, drag, atmosphere, stability,
dispersion, analysis) and the parts of `crates/ballistics-shot/src/lib.rs`
that feed it. The question is whether the physics and numerics are right,
compared with published formulations. Malformed input
(`core-software-review-2026-10-25.md`) and catalog BC values
(`bc-plausibility-review-2026-10-25.md`) are out of scope.

## Summary verdict

The engine is a sound, well-converged 3-DOF point-mass solver. The drag
tables match JBM's published G1/G7 tables point for point. The drag law,
units, atmosphere, RK4 integration, zeroing, full-vector wind, full-solve
shot angle, Coriolis/Eötvös, cant and the dispersion statistics are correct.
Where I checked them numerically they agree with independent results to well
under a millimetre at hunting ranges.

There is **one real physics defect**: the **aerodynamic jump has the wrong
sign** (SCI-01, high). A right-hand-twist bullet in a wind from the right
should jump **up**; the engine moves it down. With a 4 m/s crosswind the
resulting error is 6.9 cm at 300 m. Its tests, README and UI text repeat the
wrong direction.

The other findings are smaller:

- SCI-02: the meaning of BC bands is inconsistent between the documented
  format (steps) and the engine (interpolation).
- SCI-03: the forecast's sea-level pressure is turned into station pressure
  with the ISA (QNH) formula, but Open-Meteo reduces it with the actual
  temperature.
- SCI-04: gravity doesn't vary with latitude.
- SCI-05: humidity is left out of the speed of sound.

None of SCI-02 to SCI-05 reaches 1 cm at 300 m.

| ID | Severity | Where | What | Error at 300 m / 1000 m |
|---|---|---|---|---|
| SCI-01 | **High** | `solver.rs:424-443` (sign at `:442`), `tests.rs:367-399` | Aerodynamic jump sign reversed | 6.9 cm / 22.8 cm (4 m/s crosswind, .308) |
| SCI-02 | Medium | `ballistics-store/src/model.rs:405-412`, `docs/export-format.md:296` vs `solver.rs:73-130` | BC band semantics: documented as steps, computed as linear interpolation | 0.2 cm / 23 cm (Sierra-style bands) |
| SCI-03 | Low (medium in high or cold mountains) | `atmosphere.rs:33-37`, `ballistics-shot/src/lib.rs:36`, `app/src/weather.rs:144,428` | ISA inversion of model MSL pressure | 0.2 cm / 15 cm at a 1.1 % pressure error |
| SCI-04 | Low | `solver.rs:18`, `:754` | Gravity fixed at 9.80665, no latitude | 0.1 cm / 2.5 cm at 65° N |
| SCI-05 | Low | `atmosphere.rs:71-74` | Speed of sound ignores humidity | 0.04 cm / 2.1 cm at 30 °C, 100 % RH |

### What was verified and what was only reasoned

**Verified (run or computed).** All numbers come from a scratch copy of the
crate with switches added. The repo was not touched; the scratch files are
under the session scratchpad.

- Drag tables diffed against JBM's own `mcg1.txt` and `mcg7.txt` (Wayback
  Machine copies) and py-ballisticcalc's tables: identical.
- RK4 step convergence: 0.5 ms against 0.05 ms.
- Linear against monotone-cubic (PCHIP) table interpolation.
- Zero convergence.
- Crosswind against Didion's lag formula.
- Shot angle against the rifleman's rule.
- Coriolis and Eötvös signs and the flat-fire bound.
- Cant against a physically rotated bore.
- Spin drift against Litz's published example.
- Aerodynamic jump sign and size against an independent rigid-body
  simulation of a spinning, statically unstable projectile.
- Extreme spread in σ by Monte Carlo.
- Sensitivity of the trajectory to gravity, humidity in the speed of sound,
  pressure, BC (±1.8 %) and BC-band semantics.
- Open-Meteo `pressure_msl` against `surface_pressure` at 8 sites (live
  query, 2026-11-05).
- `cargo test -p ballistics-core`: 43 passed.

**Reasoned only.**

- Unit constants: checked by hand.
- Miller's atmospheric correction: its exact standard pressure is in the
  paper, which I could not read.
- Interpolation error between samples.
- Saturation over ice against over water.
- Whether maker BCs labelled "Metro" or "ICAO" need converting (see 2.3).
- The dispersion and hit-chance maths, apart from the extreme-spread
  constants.

Standard test loads used below:

- **.308:** 11.7 g, G1 0.45, 790 m/s, 11" twist, 45 mm sight height, 100 m
  zero, Sg 2.14.
- **6.5:** 9.3 g, G7 0.315, 820 m/s, 8" twist, Sg 1.57.

Both are the loads in `tests.rs`, flown in an ICAO standard atmosphere.

---

## 1. Drag tables (`drag.rs`)

**(a) Code.** G1 has 79 points and G7 has 84, each Mach 0–5 against Cd
(`drag.rs:78-158`, `:163-248`). Lookup is linear interpolation, held
constant beyond the ends (`drag.rs:61-75`). A custom curve uses the same
interpolation (`drag.rs:56-58`).

**(b) Published.** JBM's G1 and G7 drag functions, as Mach against Cd
(`jbmballistics.com/ballistics/downloads/text/mcg1.txt` and `mcg7.txt`; the
site now serves only its apps, so I used the Wayback Machine copies,
`web.archive.org/web/2020id_/…/mcg1.txt`). These tables derive from the BRL
standard projectile data (McCoy, *Modern Exterior Ballistics*, ch. 5) and
are also the ones in py-ballisticcalc (`py_ballisticcalc/drag_tables.py`).

**(c) Verdict: correct.**

- Both tables are identical to JBM's, every Mach and every Cd (scripted
  diff).
- **Interpolation.** I swapped linear interpolation for monotone cubic
  (Fritsch–Carlson PCHIP). Drop changed by 0.13 cm (G1) and 0.04 cm (G7) at
  1000 m, and by 0.3 cm at 1200 m: negligible.
- **Ends of the table.**
  - Below Mach 0 is never reached.
  - Above Mach 5 is out of a rifle's envelope (validation allows up to
    2500 m/s, about Mach 7, where holding Cd constant is crude but
    irrelevant).
  - Flights stop at 20 m/s, about Mach 0.06, so the subsonic end is in
    range.

## 2. BC and retardation (`solver.rs:45-69`, `:745-759`)

### 2.1 Drag law and units

**(a) Code.**

- Retardation is `a = −ρ·π/(8·BC)·Cd(M)·|v_air|·v_air`, with BC in kg/m²
  (`solver.rs:745-753`).
- The BC is converted from lb/in² with 703.06958 (`solver.rs:21`, `:59`).
- With a custom curve, BC = m/d² (`solver.rs:58`).
- Mach = airspeed / local speed of sound (`solver.rs:752`).

**(b) Published.** The drag force is F = ½ρv²(πd²/4)·i·Cd_G(M), and
BC = m/(i·d²). That gives a = ρv²·π·Cd_G/(8·BC) (McCoy ch. 5; Litz,
*Applied Ballistics*, ch. 2). One lb/in² is 0.45359237 kg / 0.0254² m² =
703.0696 kg/m².

**(c) Verdict: correct.**

- The custom-curve branch is the same law with i = 1.
- The existing test `matches_an_independent_reference_solver` agrees with
  py-ballisticcalc 3.1.1 to within 0.5 cm at 1000 m.

### 2.2 Velocity-banded BCs

The engine is fine; the conflict is between the documented format and the
code. See **SCI-02**.

### 2.3 Reference atmosphere (Metro or ICAO)

**(a) Code.** No reference density appears anywhere. The Cd form is used
with the **actual** local density and the actual speed of sound. That is
mathematically the same as py-ballisticcalc, which multiplies the ratio
ρ/ρ_ICAO by a drag constant normalised to ρ_ICAO = 0.076474 lb/ft³.

**(b) Published.** frfrogspad's BC notes
(<https://frfrogspad.com/bcdata.htm>) say Metro-referenced BCs (Sierra,
Hornady, Barnes, Winchester) should be multiplied by 0.982 for an
ICAO-based program. They list Berger, Lapua and Nosler as ICAO. The factor
is 1.2034/1.2250, the ratio of the two standard densities.

**(c) Verdict: correct formulation, with a possible ambiguity in the input
data.**

- With a Cd table and the actual density, the reference density cancels out
  of the physics. A BC is then simply m/(i·d²) against the JBM Cd table.
- The 1.8 % only matters if a maker converted its measurements with one
  standard density while its retardation tables implied another. That can't
  be settled from the code side.
- Size if it applies (BC ×0.982 or ×1.018, .308 G1): 0.3 cm at 300 m,
  3.3 cm at 600 m, 24 cm at 1000 m. For the 6.5 G7: 0.16, 1.8 and 13 cm.
- Not listed as a defect. Truing (`ballistics-shot/src/lib.rs:290-341`)
  absorbs it.

## 3. Atmosphere (`atmosphere.rs`)

### 3.1 Density

**(a) Code.** Humid air is treated as an ideal-gas mixture,
ρ = (p − e)/(R_d·T) + e/(R_v·T), with R_d = 287.058, R_v = 461.495, and
e = RH × the Buck (1996) saturation pressure over water
(`atmosphere.rs:55-69`).

**(b) Published.** Buck (1996), as given on
<https://en.wikipedia.org/wiki/Arden_Buck_equation>:
P_s = 6.1121·exp[(18.678 − T/234.5)·(T/(257.14 + T))] hPa over liquid water.
CIPM-2007 (Picard et al., *Metrologia* 45, 149) adds an enhancement factor
of about 1.004 on e and a compressibility of Z ≈ 0.9996. ICAO Doc 7488 gives
ρ₀ = 1.2250 kg/m³ at 288.15 K and 101325 Pa.

**(c) Verdict: correct.**

- The coefficients are Buck's exactly.
- Leaving out CIPM's enhancement factor and compressibility costs under
  0.05 % in density.
- Below 0 °C the formula over water is used. That is the WMO convention for
  reported RH, and the vapour term is tiny there anyway.
- Standard day: 101325/(287.058·288.15) = 1.22498 kg/m³ (test at
  `atmosphere.rs:82-86`).

### 3.2 Station pressure and altitude

**(a) Code.**

- `station_pressure` = P_sl·(1 − 2.25577e-5·h)^5.25588
  (`atmosphere.rs:35-37`). That is the ISA troposphere, with
  2.25577e-5 = 0.0065/288.15 and 5.25588 = g·M/(R·L).
- `at_height` lapses temperature by 6.5 K/km and pressure by (T/T₀)^5.25588
  (`atmosphere.rs:41-49`). The flight uses it at the bullet's height above
  the muzzle (`solver.rs:762-765`, `:790`).

**(b) Published.** ICAO Doc 7488 (ISA). QNH is defined by inverting ISA, so
for a QNH this is exact.

**(c) Verdict.**

- Correct for QNH and for the in-flight lapse.
- **Not** exact for the model mean-sea-level pressure that the app actually
  fetches from Open-Meteo. See **SCI-03**.

### 3.3 Speed of sound

**(a) Code.** a = √(1.4·287.058·T), dry air (`atmosphere.rs:71-74`). At
15 °C this gives 340.29 m/s, as ISA does.

**(b) Published.** For humid air, γ and R are those of the mixture. Cramer
(1993, *JASA* 93, 2510) gives about +0.3 % at 20 °C and 50 % RH, and about
+0.7 % at 30 °C and saturation.

**(c) Verdict: acceptable approximation.** See **SCI-05** (low).

## 4. Integration (`solver.rs:745-818`)

**(a) Code.**

- **Equations of motion:**
  r̈ = −(ρπ/8BC)·Cd(M)·|v−w|·(v−w) − g·ŷ − 2Ω×v.
- **Integrator:** classical RK4 with a fixed step of 0.5 ms
  (`solver.rs:22`).
- Density and speed of sound are evaluated once per step, at the start
  (`:790`).
- Samples are interpolated linearly between steps (`:467-486`).
- Gravity is 9.80665 m/s² (`:18`).

**(b) Published.** These are the point-mass equations of McCoy ch. 7 and
Litz ch. 2. RK4 has a global error of O(dt⁴).

**(c) Verdict: correct and converged.**

- **Step size (verified).** A step of 0.05 ms changes drop by under
  0.001 cm at 1200 m, for both loads. The density frozen within a step is
  part of this check, since its error also shrinks with the step.
- **Interpolation between samples (reasoned).** Samples are 0.2–0.4 m apart
  and the curvature is g/v² ≈ 1.5e-5 per m. The midpoint error, (1/8)y″h²,
  is therefore about 3e-7 m.
- **Gravity.** It is constant, with no latitude term. See **SCI-04** (low).
- **Vertical wind.** Not modelled. That is acceptable for hunting.

## 5. Zeroing (`solver.rs:626-693`)

**(a) Code.**

- The secant method on the bore elevation, stopping at |drop − zero
  height| < 1 µm.
- The zero is flown in the zero-day atmosphere and powder temperature,
  level, in calm air, without Coriolis.
- The zero range and height are measured from the scope along the line of
  sight (`sight_position`, `:736`).
- The windage zero cancels spin drift at the zero range (`:676-680`).

**(b) Published.** This is the usual practice (Litz ch. 5; JBM's "zero
range" input).

**(c) Verdict: correct (verified).**

- Drop at 100 m after zeroing is −2e-5 mm (.308) and −3e-5 mm (6.5).
- Using the zero-day atmosphere is the right physics: the sights stay put
  when the weather changes.
- Leaving Coriolis out of the zero costs under 0.1 cm at a 100 m zero.

## 6. Shot angle (`solver.rs:397-403`, `:718-721`, `:767-780`)

**(a) Code.**

- A full solve: the bore is raised by inclination + zero angle.
- Gravity stays world-vertical.
- Results are projected onto the inclined line of sight.
- The air thins with the bullet's height.

**(b) Published.** Litz (*Applied Ballistics*, ch. on incline fire) says the
rifleman's rule (cos θ × horizontal range) is an approximation that
under-holds, and that a full solve is preferred.

**(c) Verdict: correct (verified).** At ±30°:

| Range | Full solve | Rifleman's rule | Level |
|---|---|---|---|
| 300 m | 1.312 mrad (down), 1.320 mrad (up) | 1.276 mrad | 1.695 mrad |
| 600 m | 4.83 / 4.87 mrad | 4.51 mrad | 5.79 mrad |

The difference between up and down comes from the air thinning with height,
as it should.

## 7. Wind (`solver.rs:162-180`, `:745-748`)

**(a) Code.** The wind vector is horizontal and goes into the drag term as
airspeed v − w (head, tail and cross components alike). 0° is from the
target and 90° is from the right; it blows toward −z, that is, leftward.
The shot crate maps clock 3 to 90° (`ballistics-shot/src/lib.rs:195`).

**(b) Published.** McCoy ch. 7 (wind in the relative velocity). The lag
rule (Didion; McCoy eq. 8.x), Z = W·(t − X/V₀), is the flat-fire result.

**(c) Verdict: correct (verified).** .308 with 4 m/s from the right:

| Range | Engine | Didion's lag formula |
|---|---|---|
| 300 m | −21.90 cm | −21.90 cm |
| 1000 m | −329.33 cm | −329.35 cm |

The hold is into the wind, which is right.

## 8. Spin drift, aerodynamic jump, Coriolis, cant

### 8.1 Spin drift (`stability.rs:38-46`, applied at `solver.rs:767-776`)

**(a) Code.** SD = 1.25·(Sg + 1.2)·t^1.83 inches, using the muzzle Sg in the
shot conditions. It is rightward for a right-hand twist. Because the zero
cancels it at the zero range, the table shows drift relative to the zero.

**(b) Published.** Litz, *Applied Ballistics for Long Range Shooting*,
p. 528 (quoted on the AccurateShooter forum, "spin drift calculation"):
"Drift = 1.25(Sg+1.2)·tof^1.83", with Sg from the Miller rule. Litz's
worked example is Sg 1.8 and t 1.6 s, giving 8.9 in. The formula gives 8.87.

**(c) Verdict: correct.**

- .308: 1.5 cm at 300 m and 38 cm at 1000 m.
- 6.5: 1.1 cm and 22 cm.
- The rigid-body simulation in 8.2 also drifts right for a right-hand twist
  (an independent check of the sign).

### 8.2 Aerodynamic jump (`solver.rs:424-443`, applied at `:404-410`)

**(a) Code.**

- Size: (0.01·Sg − 0.0024·L + 0.032) MOA per mph of crosswind, with L the
  length in calibres. That is Litz's fit and is right.
- Direction: `Some(-twist * moa_per_mph * from_right_mph * RAD_PER_MOA)`,
  where `from_right_mph` is positive for wind from the right. So with a
  right-hand twist, **wind from the right moves the impact down**. The doc
  comment (`:427-428`), the test at `tests.rs:367-399`, README.md:983-984
  and the tooltip at `app/src/calculator.rs:1255-1256` all say the same.

**(b) Published.** The fit is Litz, *Applied Ballistics* (2nd ed.,
pp. 75-83). On the direction:

- **For "wind from the right gives up".** The ballistics-engine crate
  (`docs.rs/ballistics-engine/0.33.0/src/ballistics_engine/aerodynamic_jump.rs`)
  states "per Litz a right-twist bullet jumps UP for a crosswind from the
  right and DOWN for one from the left". Forum summaries of Litz's fit
  agree (longrangehunting "Aerodynamic Jump and Ballistic Solutions": "Y is
  down for wind from the left (right twist)"; The Stalking Directory: "for
  wind from the right … deflection … to the LEFT and UPWARDS"). So do
  Sager's guide and longrangeshooting.org.
- **For "wind from the right gives down".** Burris's help page says the
  opposite. It also says, correctly, that a wind from the right pushes the
  nose to the left.

**Independent check (verified).** I simulated a rigid, spinning,
statically unstable projectile from first principles:

- A .30 calibre bullet, I_x = 6e-8 and I_y = 5.4e-7 kg·m², C_Mα = 2.8,
  C_Lα = 2.5, 800 m/s, 10" twist, Sg 1.58.
- Overturning moment, lift and gyroscopic precession, with a small amount
  of pitch damping.
- Its sign conventions were validated by reproducing rightward spin drift
  under gravity.

Results:

- In a 10 mph crosswind, a right-hand twist gives **+0.32 MOA (up) for wind
  from the right** and −0.32 MOA (down) for wind from the left. A left-hand
  twist reverses both.
- Litz's fit for the same bullet gives 0.38 MOA, so the **size** agrees and
  the **sign** is opposite to the engine's.

Physically: wind from the right gives nose-left yaw. With a right-hand spin
the yaw arms turn clockwise (as seen from behind), and the integrated lift
ends up 90° clockwise from the initial yaw, that is, up.

**(c) Verdict: wrong sign.** See **SCI-01**.

### 8.3 Coriolis and Eötvös (`solver.rs:722-730`, `:755-757`)

**(a) Code.** The full −2Ω×v term, with Ω in the local frame (x downrange,
y up, z right, right-handed):

Ω·(cos φ·cos A, sin φ, −cos φ·sin A).

Here φ is latitude and A is the azimuth from north. Ω = 7.292115e-5 rad/s
(sidereal).

**(b) Published.** McCoy ch. 7 (Coriolis acceleration in the local frame).
In the northern hemisphere the deflection is to the right, and eastward
shots go high (Eötvös) — Wikipedia, "External ballistics", citing McCoy.

**(c) Verdict: correct (verified).** I projected Ω onto the frame axes by
hand and ran the 6.5 at 65° N:

| Direction | Effect | 300 m | 1000 m |
|---|---|---|---|
| North | Rightward | 0.77 cm | 10.1 cm |
| East | Up | 0.36 cm | 4.7 cm |
| West | Down | 0.36 cm | 4.7 cm |

The flat-fire upper bound Ω·X·t·sin φ is 0.80 and 11.4 cm; the engine is
lower because it integrates the slowing bullet exactly.

### 8.4 Cant (`solver.rs:321-357`)

**(a) Code.** The cant is applied after the flight:

- The bore's own geometry, (r·right, r·up − h), is rotated by the cant.
- The gravity and wind displacement stays in the world frame.
- The correction is solved in the rifle's frame.

**(b) Published.** Litz (ch. on cant) and McCoy: canting rotates the
line-of-sight/bore offset and the launch angle about the line of sight;
gravity and wind don't rotate.

**(c) Verdict: correct (verified).** I compared it with a physical flight
whose launch vector and scope offset were both rotated about the line of
sight. They agree to ≤ 0.001 cm at 2°, 5° and 10° out to 1000 m, and to
0.008 cm at 30° and 1000 m. A right cant hits right and low, which is the
right sign.

## 9. Stability (`stability.rs:12-36`)

**(a) Code.**

- Miller: Sg = 30·m / (t²·d³·l·(1 + l²)), with m in grains, d in inches,
  and t and l in calibres.
- Velocity factor: (v/2800 ft/s)^(1/3).
- Atmospheric factor: ((T°F + 460)/519)·(29.92/P inHg), using station
  pressure.

Sg feeds spin drift and aerodynamic jump; the UI uses it for stability
advice.

**(b) Published.** Miller, "A New Rule for Estimating Rifling Twist",
*Precision Shooting*, March 2005; summarised at
<https://en.wikipedia.org/wiki/Miller_twist_rule>, which gives the formula,
the units and f_v = ∛(v/2800). The atmospheric factor is the ideal-gas
density ratio (Sg ∝ 1/ρ).

**(c) Verdict: correct.**

- **Standard pressure.** Wikipedia gives the constant 30 for "59 °F,
  750 mmHg". 750 mmHg is 29.53 inHg, against the 29.92 in the code, a 1.3 %
  difference in Sg. I couldn't read Miller's paper to settle it.
  - The effect is under 0.3 cm on spin drift at 1000 m and nil on the jump.
  - 29.92 is what JBM's and Berger's calculators commonly use.
  - Not a defect.
- **Length of tipped bullets.** The rule takes the bullet length as
  entered. For polymer-tipped bullets the Courtney/Miller correction (a
  tip-length allowance) isn't applied, so such bullets come out
  conservative, with Sg too low. This is a modelling note, not an error.

## 10. Powder temperature, energy, time, dispersion, hunting analyses

**Powder temperature** (`solver.rs:155-160`; the fit is in
`ballistics-store` `model.rs:863`).

- (a) Linear, in m/s per °C, from a reference temperature.
- (b) Litz and powder makers describe sensitivity as roughly linear over the
  hunting range.
- (c) Correct.

**Energy and time of flight.**

- (a) E = ½mv² with ground speed (`solver.rs:360-362`). Time is the
  integration time, interpolated (`:479`).
- (c) Correct.

**Dispersion** (`dispersion.rs`).

- (a) Model:
  - Independent normal errors in aim, muzzle velocity, wind speed, wind
    direction and range.
  - Linear sensitivities by finite differences, with the sights kept as
    zeroed.
  - The spreads are added in quadrature, per axis (`:85-130`).
  - Aim σ = extreme spread / range / 3.07 for 5 shots (`:12`, `:33-35`).
  - Hit chance is the integral of a bivariate normal on a ±4σ grid
    (`:152-180`).
- (b) The extreme spread of n shots from a circular normal with per-axis σ
  averages 2.41σ (n = 3) and 3.07σ (n = 5) (Taylor & Grubbs, BRL, 1975).
- (c) Correct.
  - **Verified:** Monte Carlo with 400 000 trials gives 2.408σ (n = 3),
    3.069σ (n = 5) and 3.81σ (n = 10).
  - The range-error term is the miss of a hold set for r when the target is
    at r + d, which is the right quantity.
  - The existing test checks the hit chance against the Rayleigh formula.
  - **Limits (reasoned):** cross-axis correlation is ignored (wind direction
    and jump couple the axes slightly), and the sensitivities are linear.
    Both are acceptable at these sizes.

**Point-blank, ethical limit, lead** (`analysis.rs`).

- Point-blank range: bisection on the elevation until the peak of the path
  equals the zone's radius. That is the definition of maximum point-blank
  range, and it is correct.
- Ethical limit: the velocity from v = √(2E/m), interpolated along the
  path. Correct.
- Lead: the target's speed × time of flight. Correct.

## 11. Units and constants

All checked by hand and correct:

| Constant | Value | Where |
|---|---|---|
| Grains per kg | 15432.358 (1 gr = 64.79891 mg) | `stability.rs:7` |
| Metres per inch | 0.0254 | |
| ft/s per m/s | 3.28084 | |
| mph per m/s | 2.236936 | `solver.rs:23` |
| inHg per hPa | 0.02953 | `stability.rs:10` |
| BC: kg/m² per lb/in² | 703.06958 | `solver.rs:21` |
| Earth's rotation | 7.292115e-5 rad/s | |

**MOA.** π/10800 rad, true MOA (`solver.rs:24`,
`ballistics-shot/src/lib.rs:21`). That is 2.660 cm, or 1.047", at 100 yd.
Shooter's MOA (1"/100 yd) isn't offered. That is fine: almost all scopes
use true MOA.

**Mil.** 1/1000 rad (`ballistics-shot/src/lib.rs:346`). 1 mil = 3.4377 MOA
(test at `:718-725`).

**Clicks.** The angle divided by the click value, rounded
(`ballistics-shot/src/lib.rs:391`).

---

## Findings

### SCI-01 — Aerodynamic jump has the wrong sign (High)

**Where.**

- `crates/ballistics-core/src/solver.rs:442`: the leading `-` in
  `Some(-twist * moa_per_mph * from_right_mph * RAD_PER_MOA)`.
- The doc comment at `:427-428`.
- `tests.rs:367-399` (`a_crosswind_from_the_right_throws_a_right_hand_twist_bullet_down`)
  asserts the wrong direction.
- README.md:983-984 and `crates/app/src/calculator.rs:1255-1256` describe
  it.

**What.** With a right-hand twist, the engine lowers the impact for wind
from the right and raises it for wind from the left. Litz's convention (as
transcribed by the sources in 8.2) and an independent first-principles
simulation both give the opposite: wind from the right gives **up**.

**Reproduction (verified).** The .308 test load, 4 m/s wind from 90°,
right-hand twist. The engine's jump is −1.14 cm at 100 m, −3.43 cm at
300 m, −6.85 cm at 600 m and −11.42 cm at 1000 m. The true value has the
same size and the opposite sign, so the hold error is twice that:

| Range | Error |
|---|---|
| 100 m | 2.3 cm |
| 300 m | 6.9 cm |
| 600 m | 13.7 cm |
| 1000 m | 22.8 cm |

The hold is wrong whenever there is a crosswind and the jump is on, which is
the default (`ballistics-shot/src/lib.rs:79`).

**Fix.**

- Remove the minus: `Some(twist * moa_per_mph * from_right_mph * RAD_PER_MOA)`.
- Flip the doc comment, the test's assertions (`from_right > 0`,
  `from_left < 0`; for a left-hand twist, `< 0`) and the README and tooltip
  wording ("wind from the right lifts it").
- Add a release note: holds in a crosswind move by about 0.8 MOA per 10 mph.

The size formula is right and needs no change.

### SCI-02 — BC-band meaning differs between the format and the engine (Medium)

**Where.**

- `crates/ballistics-store/src/model.rs:405-412` and
  `docs/export-format.md:296`: "`velocity_mps`: From this speed down to the
  next band's" — a step function, constant within each band.
- Against `crates/ballistics-core/src/solver.rs:73-75` and `:115-130`, and
  README.md:299: linear interpolation between the points, held constant
  beyond the ends.

**What.** There are three conventions in play:

- The published format describes steps.
- The engine interpolates. Strelok's multi-BC also interpolates, per user
  reports, so imports agree with the engine.
- Sierra, the main maker publishing bands, gives steps whose velocity is
  each band's **lower** bound ("0.xxx at 2600 fps and above, 0.yyy between
  2100 and 2600 …"; see frfrogspad's BC notes and Sierra's listings).

A hunter who types Sierra's bands as printed gets interpolated BCs that are
too high inside every band.

**Reproduction (verified).** An illustrative 168 gr .30 bullet at 823 m/s
with Sierra-style bands: 0.462 above 2600 fps, 0.447 in 2100–2600, 0.424 in
1600–2100, 0.405 below. Entered as points at the band edges, the engine
gives these drops; flown as Sierra's steps, it gives a different set:

| Range | Engine (interpolated) | Sierra's steps | Error |
|---|---|---|---|
| 300 m | −45.63 cm | −45.85 cm | 0.2 cm |
| 600 m | −313.9 cm | −317.2 cm | 3.3 cm |
| 1000 m | −1380.8 cm | −1404.0 cm | 23 cm |

**Fix.** Pick one meaning and use it everywhere.

1. The simplest is to keep interpolation (it is smooth and matches Strelok
   imports). Then:
   - Correct the field doc comment (it becomes the published description)
     and regenerate the format docs with
     `MXSC_UPDATE_DOCS=1 cargo test -p ballistics-store export_format`.
   - Add UI help saying the BC applies *at* the speed given and that
     Sierra's bands are best entered at each band's mid-speed.
2. Or add an explicit step mode for Sierra-style data.

### SCI-03 — Sea-level pressure from the forecast is reduced with ISA (Low; medium in high or cold mountains)

**Where.**

- `crates/ballistics-core/src/atmosphere.rs:33-37` (ISA inversion).
- Used by `crates/ballistics-shot/src/lib.rs:36` for `Pressure::SeaLevel`.
- The app fills it from Open-Meteo's `pressure_msl`
  (`crates/app/src/weather.rs:144`, `:428`).

**What.** The ISA formula is exact for a QNH, but Open-Meteo's
`pressure_msl` is a model value reduced with the actual temperature.

**Reproduction (verified, live Open-Meteo query, 2026-11-05).** ISA from
`pressure_msl` against Open-Meteo's own `surface_pressure` at the same
elevation:

| Site | Elevation | Error |
|---|---|---|
| Luleå | 15 m | +0.01 % |
| Åre | 396 m | +0.10 % |
| Kiruna | 551 m | +0.23 % |
| Davos | 1546 m | +0.23 % |
| Jotunheimen | 1894 m | +0.50 % |
| Denver | 1597 m | −0.44 % |
| Lhasa | 3649 m | −1.72 % |

The hypsometric reduction, P_st = P_msl·exp(−g·h / (R_d·(T₂ₘ + 0.0065·h/2))),
reproduces Open-Meteo's surface pressure to within 0.01 % at all 8 sites.

By the same formula (reasoned), a −15 °C winter day at 1000 m gives about
+1.1 %. Effect of a 1.09 % pressure error on the .308: 0.18 cm at 300 m,
2.1 cm at 600 m, 14.8 cm at 1000 m.

**Fix.**

- For forecast weather, request `surface_pressure` (passing `&elevation=`
  the hunter's altitude, so Open-Meteo downscales to it), or reduce
  `pressure_msl` hypsometrically with the reported temperature.
- Keep the ISA path for a QNH typed in by hand.

### SCI-04 — Gravity doesn't vary with latitude (Low)

**Where.** `crates/ballistics-core/src/solver.rs:18` and `:754`.

**What.** g = 9.80665 is standard gravity (its value near 45°). The WGS-84
(Somigliana) normal gravity is
g(φ) = 9.7803253·(1 + 0.0019318527·sin²φ)/√(1 − 0.0066943800·sin²φ),
which gives 9.8229 at 65° N (+0.16 %) and 9.7803 at the equator (−0.27 %).

**Reproduction (verified, .308, zero and shot both at g(65°)).** Drop
changes by 0.10 cm at 300 m, 0.61 cm at 600 m and 2.5 cm at 1000 m. For the
6.5: 0.09, 0.49 and 1.8 cm.

**Fix.** Use g(φ) when a latitude is known: the Coriolis input, or the
weather location. Optionally subtract 3.086e-6·h for altitude.

### SCI-05 — Speed of sound ignores humidity (Low)

**Where.** `crates/ballistics-core/src/atmosphere.rs:71-74`.

**What.** Humidity is correctly in the density, but the speed of sound uses
dry air. Humid air is faster (lower molar mass), so the Mach number is
slightly overstated.

**Reproduction (verified).** With the mixture γ and R:

| Conditions | Speed of sound | Effect on drop |
|---|---|---|
| 30 °C, 100 % RH | 351.42 m/s against 349.04 (+0.68 %) | .308: 0.04 cm at 300 m, 0.5 cm at 600 m, 2.1 cm at 1000 m. 6.5: 1.9 cm at 1000 m |
| 15 °C, 50 % RH | +0.14 % | 0.5 cm at 1000 m |
| −20 °C | — | Nil |

The largest effect is in the transonic region.

**Fix.** Compute a from the mixture:
x = e/p; M = (1−x)·M_d + x·M_v; γ from c_p/R = 3.5(1−x) + 4.0x;
a = √(γ·R·T/M). Or use the virtual temperature,
T_v = T/(1 − 0.378·e/p), with γ = 1.4, which is within 0.05 %.

---

## Not verified / could not check

- **Litz's book itself** (aerodynamic-jump direction, pp. 75-83, and the
  spin drift formula on p. 528). I relied on transcriptions in the
  ballistics-engine crate and forum quotes, plus my own rigid-body
  simulation for the sign. McCoy §12.9 (crosswind jump) wasn't accessible
  either.
- **Miller's 2005 paper**, so the standard pressure behind the constant 30
  (29.92 inHg or 750 mmHg; worth 1.3 % in Sg) is unconfirmed.
- **Metro/ICAO labels on maker BCs.** Whether a given maker's published BC
  needs the 0.982/1.018 conversion in a Cd-form solver depends on the
  maker's own software. The size is given in 2.3.
- **The JBM online calculator** (the site is gone). I compared tables
  against the archived JBM files. Trajectories were compared only through
  the existing py-ballisticcalc test and my own checks.
- **Strelok.** Its multi-BC interpolation is from user reports, not
  official docs. The Strelok import's "temperature modifier" as % per 15 °C
  (`ballistics-store/src/strelok.rs:829-832`) was not checked against
  Strelok's documentation.
- **Open-Meteo's `pressure_msl` method.** I inferred it from the data (the
  hypsometric reduction matches to 0.01 %); it is not documented.
