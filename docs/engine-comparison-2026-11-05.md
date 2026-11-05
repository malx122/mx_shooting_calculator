# Engine comparison with other ballistic calculators — 2026-11-05

The exterior-ballistics engine (`crates/ballistics-core`, version 0.9.1, branch
`fix/live-test-findings` at `7b15255`) compared, number by number, with
independent calculators across 13 scenarios: nine cartridges, G1 and G7, 40–60 mm
sights, 100 m and 200 yd zeros, ICAO, hot and high, and Nordic winter air,
crosswinds and a 45° wind, ±30° shots, spin drift and Coriolis, out to 1500 m
(2200 m for the transonic check). The engine itself was not changed.

## Summary

- **No discrepancy found where our engine differs from every reference.**
  Wherever the inputs can be matched exactly, the references agree with us to
  within their own rounding.
- **py-ballisticcalc 3.1.1** (open-source, offline cross-check) agrees with our
  engine to within **0.06 cm and 0.001 mil (drop and drift) up to 500 m**, 0.09 cm
  out to 1500 m and 0.35 cm out to 2200 m (subsonic). Velocity agrees within 0.15 m/s and time of flight
  within 0.12 ms. That holds for G1 and G7, cold, hot and high, humid air, a 45° wind, a 200 yd zero and
  Coriolis. The only gaps are two documented differences in convention:
  shooting angle (CMP-04, 0.7 cm) and spin drift at the zero range (CMP-05,
  0.02 mil).
- **Federal Premium's web calculator** (independent commercial solver, G1
  only), given the same air, agrees within **1.7 cm / 0.035 mil (drop) and
  0.7 cm / 0.024 mil (drift) up to 500 m**, and within 1.3 m/s everywhere. Beyond
  500 m its drop scatters irregularly (up to 5.5 cm at 1000 m and 17 cm, 0.11
  mil, at 1500 m) while its velocity and drift still match: noise in Federal's
  own solver (CMP-03). In cold air Federal differs by up to 3.7 cm and 7 m/s at
  500 m, because it does not correct the speed of sound for temperature (CMP-02).
  Our engine is right there, and py-ballisticcalc agrees with us.
- **The biggest practical difference is an input convention, not the engine:**
  Federal (like most US ammunition tables) calls the Army Standard Metro day
  (59 °F, 29.53 inHg, 78 % RH, ρ ≈ 1.203 kg/m³) "standard". Our ICAO day
  (15 °C, 1013.25 hPa, dry, ρ = 1.225) is 1.8 % denser, which gives 2.3 cm more
  drop for a .308 at 500 m and 12 cm (0.12 mil) more for a 6.5 CM at 1000 m
  (CMP-01).
- **Aerodynamic jump** (coordinator's question): no reference I could use
  models it, so its sign could not be checked against a calculator (CMP-06).
  Our engine sends a right-hand-twist bullet **down** in wind from the right
  and **up** in wind from the left. The commonly published rule (Litz) is the
  reverse. All comparisons in this report ran with **aerodynamic jump off**.

| Reference | Up to 500 m: largest \|Δ\| drop / drift / velocity | Beyond 500 m |
|---|---|---|
| py-ballisticcalc 3.1.1 (same inputs) | 0.06 cm (0.001 mil) / 0.05 cm (0.001 mil) / 0.15 m/s | 0.09 cm / 0.04 cm / 0.07 m/s to 1500 m; 0.35 cm (0.002 mil) to 2200 m |
| Federal Premium web, matched air, normal temperatures | 1.7 cm (0.035 mil) / 0.7 cm (0.024 mil) / 1.1 m/s | 16.9 cm (0.11 mil) / 1.3 cm (0.013 mil) / 1.3 m/s (CMP-03) |
| Federal Premium web, −20 °C cases | 3.7 cm (0.075 mil) / 3.0 cm (0.061 mil) / 7.2 m/s (CMP-02) | – |

Δ is always *ours − reference*. Drop is relative to the line of sight
(negative = below the crosshair). Drift is positive to the right. The mil
values are milliradians (Δcm·10/range m).

## Findings

| ID | Severity | Whose difference | One line |
|---|---|---|---|
| CMP-01 | Low (documentation) | Input convention | "Standard day" means ASM in Federal's and US makers' tables, ICAO in ours: 1.8 % density difference |
| CMP-02 | None for us | Federal | Federal ignores temperature in the speed of sound (Mach); we and py-ballisticcalc don't |
| CMP-03 | None for us | Federal | Federal's drop has an irregular ±0.5 % scatter beyond 500 m; its velocity and drift match ours |
| CMP-04 | None (we are right) | py-ballisticcalc | On a slope py-ballisticcalc keeps the sight offset vertical; we rotate it with the rifle (0.67 cm at 50 mm, 30°) |
| CMP-05 | Info | Convention | Our zero removes the zero-range spin drift with the windage zero; py-ballisticcalc leaves it (0.021 mil) |
| CMP-06 | High; confirmed by Hornady 4DOF | Ours vs literature | Aerodynamic jump direction: we go down in wind from the right with a right-hand twist; Litz says up. On by default in the app |
| CMP-07 | Info | Federal | Federal's metric mode rounds wind to whole mph (5 m/s → 11 mph = 4.917 m/s, −1.7 %) |

### CMP-01 — "Standard atmosphere" means different air (input convention, low)

*Evidence.* At the same nominal "standard day", Federal's .308 150 gr / G1 0.400 /
2800 fps table (100 yd zero, 1.5 in sight, 10 mph) gives 1775 fps at 500 yd. Our
engine gives 1761 fps with ICAO air, but **1777 fps with ASM air** (999.92 hPa,
78 % RH). The ASM run also reproduces every drop and drift value of that table
to within 0.1 in (−57.93 vs −57.9 in at 500 yd). At 2000 m altitude
Federal's numbers match ASM pressure scaled by the barometric formula, at
78 % RH. All the Federal comparisons below therefore use that air for our run.
With matched air, our engine and Federal's agree (see the C01–C12 tables).

*Size of the convention gap* (our engine, ICAO dry vs ASM, no wind):

| Case | Range | Drop ICAO | Drop ASM | Δ | Velocity ICAO / ASM |
|---|---:|---:|---:|---:|---:|
| .308 150 gr G1 0.338, 859.5 m/s | 300 m | −44.40 cm | −44.07 cm | 0.33 cm (0.011 mil) | 605.1 / 609.2 m/s |
| | 500 m | −198.69 cm | −196.43 cm | 2.26 cm (0.045 mil) | 465.3 / 471.0 m/s |
| 6.5 CM 143 gr G7 0.315, 823 m/s | 500 m | −167.14 cm | −166.20 cm | 0.93 cm (0.019 mil) | 598.1 / 601.8 m/s |
| | 1000 m | −1049.98 cm | −1037.65 cm | 12.3 cm (0.123 mil) | 409.2 / 415.5 m/s |

*Cause.* This is not an engine error. Our drag term is ρ·π·Cd(M)·v²/(8·BC) with
the actual density, which is the same formulation Federal's numbers and
py-ballisticcalc imply. With the same density they agree. What differs is
which air a "standard day" input stands for. The app's default conditions are
15 °C, 1013.25 hPa, 50 % RH (ρ = 1.221), close to ICAO. When a hunter checks
the app against a printed US box table, the app shows slightly more drop
beyond 300 m.

*Suggestion.* No engine change. Possibly say in the help text that US
ballistics tables use Army Standard Metro (59 °F, 29.53 inHg, 78 %). Whether makers' BCs
"referenced to ASM" need a 1.8 % conversion before use with actual density is
a question for the literature review running in parallel. Numerically, Federal
treats its BCs exactly as our engine does.

### CMP-02 — Federal does not correct the speed of sound for temperature (reference simplification)

*Evidence.* In the two −20 °C cases Federal is 5–7 m/s slower at 500 m, with
2.6–3.7 cm more drop and 2.8–3.0 cm more drift. The −20 °C cases are
`C02` (.308 180 gr) and `C09` (.243 100 gr). In every 15 °C and 30 °C case the
same comparison stays within 1 m/s. I
tested hypotheses one at a time with a minimal independent point-mass solver
(`scripts/mini.py`). It reproduces our engine exactly when given the engine's ρ and speed of sound:

| C09, 500 m | Velocity | Drop | Drift (11 mph) |
|---|---:|---:|---:|
| Federal | 516 m/s | −152.7 cm | 88.9 cm |
| Ideal-gas ASM air, a(−20 °C) = 319.0 m/s (as our engine) | 523.2 | −150.1 | 86.1 |
| Same density, **a fixed at 340.3 m/s (15 °C)** | 515.2 | −152.3 | 89.0 |

| C02, 500 m | Velocity | Drop | Drift |
|---|---:|---:|---:|
| Federal | 487 m/s | −212.7 cm | 87.7 cm |
| a(T) as our engine | 492.9 | −209.0 | 84.7 |
| **a fixed at 15 °C** | 486.4 | −211.7 | 87.6 |

With the speed of sound held at its 15 °C value, Federal's cold-air numbers
are reproduced to within its rounding. A pure density error would have to be
+2 % on top of the ideal gas law (1.403 vs 1.3755 kg/m³ fits the velocity) and
has no physical basis. In the hot case (30 °C, 2000 m) the two effects
nearly cancel, so that case cannot tell the hypotheses apart.

*Conclusion.* Federal's simplification. Physically the Mach number must use
the local speed of sound, and our engine and py-ballisticcalc both do.
Nothing to change.

### CMP-03 — Federal's drop scatters beyond 500 m (reference numerical noise)

*Evidence.* Δdrop (ours − Federal) does not grow smoothly. For C05 it runs
−1.0, +1.5, +2.6, +1.8, +5.5 cm at 600–1000 m; for C12 (G1) +4.3, +3.8, +2.4,
+8.1, +14.1, +16.9 cm at 1000–1500 m. Over the same rows Federal's velocity is
within 1.3 m/s of ours and its drift within 1.3 cm. A range offset cannot
explain this: at 1500 m it would need 3.7 m, which would shift the drift by
2.4 cm, and the drift agrees within 0.2 cm. Federal's imperial mode shows the
same pattern (C05 at 1100 yd: 1.8 in), so metric conversion isn't the cause either.
py-ballisticcalc, an independent RK4 code, matches our drop within 0.1 cm at
the same ranges. The scatter is therefore inside Federal's solver (probably a
coarse step or output sampling). It reaches 0.11 mil at 1500 m and at most
0.055 mil out to 1000 m.

### CMP-04 — Sight offset on uphill/downhill shots (py-ballisticcalc simplification; ours correct)

*Evidence.* Cases C07u/C07d (±30°): py-ballisticcalc is 0.64–0.69 cm higher
at every range. That equals sight height × (1 − cos 30°) = 5.0 × 0.134 =
0.67 cm. Its RK4 engine starts the bullet at (0, −sight height, 0) in the
level frame whatever the look angle (`engines/rk4.py`, `range_vector`). Our
engine puts the bore one sight height below the scope, perpendicular to the
line of sight (`Flight::new`, `sight_position = sight_up × sight_height`),
which is how a scope sits on a tilted rifle. Velocities and times agree within
0.1 m/s / 0.1 ms. Ours is the physically correct geometry. The difference is
constant (0.07 mil at 100 m, 0.013 mil at 500 m) and only matters close in.

### CMP-05 — Spin drift at the zero range (convention)

*Evidence.* Case C12s (.338 LM, spin drift on): py-ballisticcalc's drift is
larger by 0.21, 0.43, 0.64, 1.06, 1.70, 2.13, 2.56, 3.19 cm at 100–1500 m.
That is exactly linear, 0.021 mil, and equals our spin drift at 100 m. Our
`zero_angles` sets a windage angle that cancels the spin drift at the zero
range, as when the rifle's windage is zeroed on paper. py-ballisticcalc
doesn't. Add that line back and the two agree within 0.01 cm. Both use Litz's
1.25·(Sg+1.2)·t^1.83 formula. Coriolis agrees too: in C12s (65° N,
shooting east) drop within 0.07 cm to 1500 m. In separate 6.5 CM G7 runs at
60° N, 1000 m: east −1044.43 vs −1044.46 cm, west −1055.54 vs −1055.58 cm,
north 9.660 vs 9.660 cm horizontal. Ours fits a hunter who zeroed at 100 m. Worth one
sentence in the documentation, because Strelok/AB-style apps that don't
cancel the zero-range drift will show a constant ~0.02 mil offset.

### CMP-06 — Aerodynamic jump direction (reversed; confirmed by Hornady 4DOF)

*What our engine does* (6.5 CM 143 gr G7 0.315, 823 m/s, 1.43 in, 1:8 RH,
Sg ≈ 1.6, 5 m/s full crosswind, `aerodynamic_jump = true`):

| Wind | 300 m | 500 m | 1000 m |
|---|---:|---:|---:|
| From the right (90°, 3 o'clock) | −3.35 cm (**down** 0.112 mrad) | −5.57 cm | −11.15 cm |
| From the left (270°, 9 o'clock) | +3.34 cm (**up**) | +5.57 cm | +11.15 cm |

The size matches Litz's fit, (0.01·Sg − 0.0024·L + 0.032) MOA per mph:
0.035 MOA/mph × 11.2 mph = 0.39 MOA = 0.115 mrad.

*References.* py-ballisticcalc and Federal don't model aerodynamic jump (no
jump code in py-ballisticcalc 3.1.1). Hornady 4DOF does, but its page opens
behind an age-confirmation dialog, which I did not answer on the user's
behalf (see Method). So the sign could not be checked numerically.

*Literature.* The commonly published rule (Litz, Applied Ballistics) is that
with a right-hand twist, wind from the right throws the bullet **up** and wind
from the left **down**. That is the opposite of our engine, and it matches the
science reviewer's reading of `solver.rs` `aerodynamic_jump`. If the rule
holds, the error is twice the jump: 0.22 mrad for a 5 m/s full-value
crosswind, 6.7 cm at 300 m. That is above the 0.1 mil scatter seen between
calculators, and `ballistics_shot::Conditions::default()` turns jump **on**.
Recommendation: confirm the sign from the primary source (the literature
review), or run one crosswind case in Hornady 4DOF by hand; then fix the sign
with a test.

*Hornady 4DOF check (added afterwards, run by hand, 2026-11-05).* After the
user confirmed the age question, one case was run in Hornady 4DOF
(hornadyfourdofiframeprod.azurewebsites.net, the calculator behind
hornady.com/4dof). The inputs were: Hornady 6.5 mm 143 gr ELD-X, 2700 ft/s,
1:8 in twist (spin drift goes right, so right-hand), 0.264 in bore, 1.5 in
sight height, 100 yd zero, 29.92 inHg, 59 °F, 50 % humidity, and a 10 mph
wind at 90°.

| Range (yd) | Wind drift (in) | Aerodynamic jump (in) | No-wind drop (in) | Total come-up (in) | Gyro Sg |
|---:|---:|---:|---:|---:|---:|
| 100 | −0.6 | +0.3 | 0.0 | −0.3 | 1.66 |
| 200 | −2.3 | +0.6 | −3.8 | 3.2 | 1.79 |
| 300 | −5.4 | +0.9 | −13.7 | 12.8 | 1.93 |
| 500 | −16.0 | +1.5 | −54.6 | 53.1 | 2.28 |

The wind drifts the bullet left, so 90° is wind from the right. The jump
column is positive, and the come-up is 0.9 in smaller than the no-wind drop
at 300 yd, so 4DOF throws the bullet **up**. That is Litz's rule and the
opposite of our engine; **CMP-06 is confirmed**.

The size is 0.29 MOA for 10 mph. Litz's fit gives about 0.35 MOA for this
bullet. 4DOF's own Sg is 1.54 at the muzzle.

*Setting used.* Every comparison table below has aerodynamic jump **off**
(`aerodynamic_jump = false`), so jump doesn't affect the drop comparisons.

### CMP-07 — Federal rounds metric wind to whole mph (reference input quirk)

5 m/s becomes 11 mph (4.917 m/s): the drift is 1.7 % low (57.7 vs 58.8 cm at
500 m in a probe). With 4.4704 m/s (exactly 10 mph) Federal's drift matches
ours within 0.2 cm. Every Federal comparison below re-runs our engine at the
rounded wind.

## Method

### Our engine: harness

A separate cargo project depends on the crate by path and prints a range
table for one scenario given as `key=value` arguments:

- Source: `/private/tmp/claude-501/-Users-dro-dev-ballistic-calculator/ee6308e0-d91e-4121-9b9a-2cd6c7a550ac/scratchpad/engine-compare/harness/src/main.rs`
  (listed in full in the appendix, since the scratchpad is temporary).
- Driver scripts: `…/engine-compare/scripts/` — `cases.py` (the matrix),
  `ours.py`, `pbc.py` (py-ballisticcalc), `federal.py` / `federal_batch.py`
  (Federal web form), `compare_federal.py`, `mini.py` (hypothesis solver),
  `transonic.py`, `tables.py` (builds the tables below). Raw results:
  `…/engine-compare/results/` (`pbc_vs_ours.json`, `federal_vs_ours.json`,
  `federal/*.html` the raw Federal answers, `transonic.json`).

What the harness feeds `ballistics_core::solver::solve` (Scenario fields):

- Bullet: mass from grains (/15 432.358), diameter in inches, BC in lb/in²,
  `DragModel::G1/G7`, no BC bands, no drag curve. `length_m` is set only for
  the spin drift case. Without a length the engine has no stability factor, so
  spin drift and aerodynamic jump are off whatever their flags say.
- Rifle: sight height (mm), zero range (m), zero height 0, twist (in), right-hand.
- Load: muzzle velocity in m/s; powder sensitivity 0.
- **Zero atmosphere = shot atmosphere** unless stated. JBM-style calculators
  zero in the shot's air; the app normally zeroes in the rifle's recorded zero
  weather.
- Shot: atmosphere as **station pressure**, temperature and RH. Wind uses
  `from_degrees`: 0 = headwind, 90 = from the right, 270 = from the left. Shot
  angle positive uphill. Coriolis only where stated.
- `spin_drift` and `aerodynamic_jump` both **false** except case C12s (spin
  drift on, jump off).
- Output: `Solution::at(range)` at the slant range: drop and drift (cm),
  elevation/windage correction (mrad, MOA), velocity, energy, time of flight.

### References used

| Reference | Version / URL | How | Limits |
|---|---|---|---|
| py-ballisticcalc | 3.1.1 (pip, LGPL), RK4 engine, 2026-11-05 | Offline, `Calculator.set_weapon_zero` then `fire`, rows read with `get_at('distance'/'slant_distance')`. Wind mapped: their 0° = from behind, 90° = from the left, so ours θ → theirs 180°+θ. Angles in `Unit.MRad` (their `Unit.Mil` is the 1/6400 NATO mil) | Not a web calculator, and it shares the point-mass model, so it checks the implementation rather than the physics. No aerodynamic jump |
| Federal Premium ballistics calculator | https://www.federalpremium.com/ballistics-calculator, form `BallisticCalculator-Submit`, 2026-11-05 | Plain form POST (metric, "Handload", G1 BC), as the page's own form does; session cookie and the page's CSRF token; ~20 requests, ≥3–5 s apart | G1 only; temperature and altitude only (no pressure or RH input); full crosswind only; no shot angle, spin drift or Coriolis; output 0.1 cm / 1 m/s |

### References tried and not used

- **JBM Ballistics** (`jbmballistics.com/cgi-bin/jbmtraj-5.1.cgi`): gone. The
  site answers 404 and says its ballistics calculators "have been retired and
  are no longer available".
- **shooterscalculator.com**: answered HTTP 406 to a scripted request (it
  blocks automated use). Skipped.
- **Hornady 4DOF / Standard** (hornady.com/4dof, a Blazor WebAssembly app):
  can only be driven in a browser. On hornady.com it first asks the visitor to
  confirm they are 18 or older. Answering that is an attestation on the
  user's behalf, so I stopped there. A human can run CMP-06 in it in a few
  minutes.
- **1967spud.com** ballistics suite: HTTP 403 to scripted requests. Skipped.
- **GunWerks, Norma** calculator URLs found by search: 404. **ballistics.app**:
  TLS certificate mismatch. Not pursued. **Applied Ballistics** web and mobile
  solvers: account or app based, not tried.

### Test matrix

Bullet data are typical published maker figures (Hornady, Nosler, Norma,
Berger). The comparison does not depend on their exactness, since every
calculator gets the same numbers. Ranges are in metres. Every case was run
in our engine and py-ballisticcalc; the G1 level cases also went to Federal.
For C11 and C12, Federal got the same bullets with their published G1 BCs
(0.597 and 0.818).

## Per-case tables

### C01: .308 Win 150 gr Hornady InterLock SP, ICAO standard day, 5 m/s crosswind

Inputs: 150 gr, Ø 0.308 in, G1 BC 0.338, MV 859.5 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -3.33 | -3.33 | +0.00 | 769.0 | 769.1 | 0.1230 | 0.1230 |
| 200 | -11.67 | -11.67 | -0.00 | -0.000 | -14.09 | -14.08 | -0.00 | 684.3 | 684.4 | 0.2609 | 0.2609 |
| 300 | -44.40 | -44.40 | -0.00 | -0.000 | -33.62 | -33.62 | -0.00 | 605.1 | 605.1 | 0.4163 | 0.4163 |
| 500 | -198.69 | -198.69 | -0.00 | -0.000 | -106.03 | -106.03 | -0.00 | 465.3 | 465.3 | 0.7938 | 0.7938 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 150 gr, G1 BC 0.338, MV 859.5 m/s, sight 5.0 cm, zero 100 m, 15.0 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 3.2 | 3.3 | -0.1 | 770.6 | 771 | -0.4 | 0.1229 |
| 200 | -11.6 | -11.7 | +0.1 | +0.005 | 13.6 | 13.7 | -0.1 | 687.3 | 687 | +0.3 | 0.2603 |
| 300 | -44.1 | -44.2 | +0.1 | +0.004 | 32.4 | 32.7 | -0.3 | 609.2 | 609 | +0.2 | 0.4148 |
| 400 | -103.1 | -102.8 | -0.3 | -0.008 | 61.2 | 61.1 | +0.1 | 536.8 | 536 | +0.8 | 0.5897 |
| 500 | -196.4 | -197.5 | +1.1 | +0.021 | 101.8 | 102.3 | -0.5 | 471.0 | 471 | +0.0 | 0.7887 |

### C02: .308 Win 180 gr Nosler Partition, Nordic winter −20 °C, 45° wind

Inputs: 180 gr, Ø 0.308 in, G1 BC 0.474, MV 798.6 m/s, sight 45 mm, zero 100 m, -20 °C, 1013.25 hPa station, RH 50 %, wind 5 m/s from 45°. Aerodynamic jump off. ρ and speed of sound: `rho 1.3940 kg/m3, a 318.96 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -2.05 | -2.05 | +0.00 | 729.2 | 729.2 | 0.1310 | 0.1311 |
| 200 | -14.01 | -14.01 | +0.00 | +0.000 | -8.58 | -8.58 | +0.00 | 663.5 | 663.4 | 0.2748 | 0.2748 |
| 300 | -50.40 | -50.40 | +0.01 | +0.000 | -20.24 | -20.25 | +0.01 | 601.1 | 601.0 | 0.4332 | 0.4332 |
| 500 | -211.30 | -211.35 | +0.05 | +0.001 | -62.27 | -62.32 | +0.04 | 487.0 | 486.9 | 0.8030 | 0.8031 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 180 gr, G1 BC 0.474, MV 798.6 m/s, sight 4.5 cm, zero 100 m, -20 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 2.8 | 2.6 | +0.2 | 730.6 | 729 | +1.6 | 0.1309 |
| 200 | -13.9 | -14.2 | +0.3 | +0.013 | 11.7 | 12.1 | -0.4 | 666.2 | 663 | +3.2 | 0.2743 |
| 300 | -50.0 | -50.5 | +0.5 | +0.015 | 27.6 | 28.3 | -0.7 | 605.0 | 600 | +5.0 | 0.4318 |
| 400 | -113.1 | -113.8 | +0.7 | +0.019 | 51.5 | 52.7 | -1.2 | 547.1 | 542 | +5.1 | 0.6056 |
| 500 | -209.0 | -212.7 | +3.7 | +0.075 | 84.7 | 87.7 | -3.0 | 492.9 | 487 | +5.9 | 0.7983 |

### C03: 6.5×55 156 gr Norma Oryx, hot and high (30 °C, 2000 m)

Inputs: 156 gr, Ø 0.264 in, G1 BC 0.348, MV 780 m/s, sight 50 mm, zero 100 m, 30 °C, 794.95 hPa station, RH 30 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 0.9080 kg/m3, a 349.04 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -2.75 | -2.75 | -0.00 | 717.3 | 717.3 | 0.1337 | 0.1337 |
| 200 | -14.12 | -14.12 | -0.00 | -0.000 | -11.46 | -11.45 | -0.00 | 657.4 | 657.5 | 0.2793 | 0.2793 |
| 300 | -51.00 | -50.99 | -0.00 | -0.000 | -26.93 | -26.93 | -0.00 | 600.6 | 600.6 | 0.4385 | 0.4385 |
| 500 | -212.21 | -212.20 | -0.01 | -0.000 | -81.91 | -81.90 | -0.01 | 497.0 | 497.0 | 0.8048 | 0.8048 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 156 gr, G1 BC 0.348, MV 780 m/s, sight 5.0 cm, zero 100 m, 30 °C, altitude 2000 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (784.5 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 2.6 | 2.4 | +0.2 | 718.7 | 719 | -0.3 | 0.1336 |
| 200 | -14.0 | -14.3 | +0.3 | +0.013 | 11.0 | 11.1 | -0.1 | 660.1 | 660 | +0.1 | 0.2787 |
| 300 | -50.7 | -50.9 | +0.2 | +0.008 | 25.8 | 25.9 | -0.1 | 604.4 | 604 | +0.4 | 0.4371 |
| 400 | -114.2 | -113.7 | -0.5 | -0.012 | 47.9 | 47.5 | +0.4 | 551.7 | 551 | +0.7 | 0.6103 |
| 500 | -210.1 | -211.4 | +1.3 | +0.027 | 78.3 | 79.0 | -0.7 | 502.5 | 502 | +0.5 | 0.8002 |

### C04: 6.5×55 140 gr soft point, 200 yd zero, 40 mm sight

Inputs: 140 gr, Ø 0.264 in, G1 BC 0.465, MV 808 m/s, sight 40 mm, zero 182.88 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 270°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 5.47 | 5.47 | +0.00 | +0.000 | 2.60 | 2.61 | -0.00 | 744.1 | 744.1 | 0.1290 | 0.1290 |
| 200 | -2.81 | -2.81 | -0.00 | -0.000 | 10.86 | 10.86 | -0.00 | 683.1 | 683.1 | 0.2692 | 0.2692 |
| 300 | -32.18 | -32.18 | +0.00 | +0.000 | 25.50 | 25.50 | -0.00 | 625.0 | 625.0 | 0.4223 | 0.4223 |
| 500 | -171.59 | -171.60 | +0.01 | +0.000 | 77.55 | 77.56 | -0.01 | 518.1 | 518.1 | 0.7739 | 0.7739 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 140 gr, G1 BC 0.465, MV 808 m/s, sight 4.0 cm, zero 182.88 m, 15.0 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 5.5 | 5.5 | -0.0 | -0.005 | 2.5 | 2.3 | +0.2 | 745.2 | 745 | +0.2 | 0.1289 |
| 200 | -2.8 | -2.9 | +0.1 | +0.005 | 10.5 | 10.4 | +0.1 | 685.3 | 685 | +0.3 | 0.2688 |
| 300 | -32.0 | -32.1 | +0.1 | +0.003 | 24.6 | 24.6 | -0.0 | 628.1 | 628 | +0.1 | 0.4212 |
| 400 | -86.1 | -85.4 | -0.7 | -0.018 | 45.6 | 45.1 | +0.5 | 573.8 | 573 | +0.8 | 0.5878 |
| 500 | -170.2 | -170.8 | +0.6 | +0.013 | 74.6 | 74.9 | -0.3 | 522.7 | 522 | +0.7 | 0.7705 |

### C05: 6.5 Creedmoor 143 gr ELD-X, G1 0.623, to 1000 m

Inputs: 143 gr, Ø 0.264 in, G1 BC 0.623, MV 823 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -1.88 | -1.88 | -0.00 | 774.5 | 774.5 | 0.1253 | 0.1253 |
| 200 | -11.37 | -11.37 | +0.00 | +0.000 | -7.73 | -7.73 | +0.00 | 727.7 | 727.7 | 0.2585 | 0.2585 |
| 300 | -41.30 | -41.30 | +0.00 | +0.000 | -17.93 | -17.93 | +0.00 | 682.5 | 682.4 | 0.4004 | 0.4004 |
| 500 | -167.41 | -167.42 | +0.01 | +0.000 | -53.13 | -53.14 | +0.01 | 596.9 | 596.8 | 0.7138 | 0.7138 |
| 800 | -575.45 | -575.49 | +0.04 | +0.000 | -150.78 | -150.80 | +0.02 | 482.1 | 482.1 | 1.2736 | 1.2737 |
| 1000 | -1050.85 | -1050.94 | +0.09 | +0.001 | -252.55 | -252.59 | +0.04 | 417.3 | 417.2 | 1.7202 | 1.7202 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 143 gr, G1 BC 0.623, MV 823 m/s, sight 5.0 cm, zero 100 m, 15.0 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 1.8 | 1.8 | +0.0 | 775.4 | 775 | +0.4 | 0.1252 |
| 200 | -11.3 | -11.3 | -0.0 | -0.002 | 7.5 | 7.2 | +0.3 | 729.3 | 729 | +0.3 | 0.2582 |
| 300 | -41.1 | -41.1 | -0.0 | -0.001 | 17.3 | 17.2 | +0.1 | 684.9 | 685 | -0.1 | 0.3997 |
| 400 | -91.9 | -91.9 | +0.0 | +0.000 | 31.7 | 31.9 | -0.2 | 641.9 | 642 | -0.1 | 0.5505 |
| 500 | -166.5 | -166.6 | +0.1 | +0.003 | 51.1 | 51.2 | -0.1 | 600.6 | 600 | +0.6 | 0.7116 |
| 600 | -268.3 | -267.3 | -1.0 | -0.017 | 76.1 | 75.6 | +0.5 | 560.9 | 560 | +0.9 | 0.8839 |
| 700 | -401.3 | -402.8 | +1.5 | +0.021 | 107.2 | 107.7 | -0.5 | 523.1 | 522 | +1.1 | 1.0685 |
| 800 | -570.3 | -572.9 | +2.6 | +0.032 | 144.9 | 145.8 | -0.9 | 487.2 | 487 | +0.2 | 1.2667 |
| 900 | -780.7 | -782.5 | +1.8 | +0.020 | 189.8 | 190.0 | -0.2 | 453.7 | 453 | +0.7 | 1.4794 |
| 1000 | -1038.8 | -1044.3 | +5.5 | +0.055 | 242.4 | 243.0 | -0.6 | 422.7 | 422 | +0.7 | 1.7080 |

### C06: 6.5 Creedmoor 143 gr ELD-X, G7 0.315, to 1000 m

Inputs: 143 gr, Ø 0.264 in, G7 BC 0.315, MV 823 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -1.86 | -1.86 | -0.00 | 774.8 | 774.8 | 0.1252 | 0.1252 |
| 200 | -11.36 | -11.36 | +0.00 | +0.000 | -7.68 | -7.68 | +0.00 | 728.1 | 728.1 | 0.2584 | 0.2584 |
| 300 | -41.25 | -41.25 | +0.00 | +0.000 | -17.83 | -17.83 | +0.00 | 683.1 | 683.1 | 0.4002 | 0.4002 |
| 500 | -167.14 | -167.15 | +0.01 | +0.000 | -52.79 | -52.80 | +0.01 | 598.1 | 598.1 | 0.7131 | 0.7131 |
| 800 | -574.09 | -574.12 | +0.03 | +0.000 | -150.15 | -150.16 | +0.01 | 481.0 | 481.0 | 1.2724 | 1.2724 |
| 1000 | -1049.99 | -1050.03 | +0.03 | +0.000 | -254.10 | -254.09 | -0.01 | 409.2 | 409.2 | 1.7233 | 1.7233 |

### C07u: .30-06 180 gr Nosler Partition, 30° uphill

Inputs: 180 gr, Ø 0.308 in, G1 BC 0.474, MV 823 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, shot angle +30°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 1.02 | 1.69 | -0.67 | -0.067 | 0.00 | 0.00 | +0.00 | 759.1 | 759.1 | 0.1265 | 0.1266 |
| 200 | -7.73 | -7.07 | -0.67 | -0.033 | 0.00 | 0.00 | +0.00 | 698.4 | 698.4 | 0.2639 | 0.2639 |
| 300 | -33.97 | -33.31 | -0.66 | -0.022 | 0.00 | 0.00 | +0.00 | 640.6 | 640.6 | 0.4134 | 0.4134 |
| 500 | -152.94 | -152.30 | -0.64 | -0.013 | 0.00 | 0.00 | +0.00 | 534.3 | 534.3 | 0.7554 | 0.7554 |

### C07d: .30-06 180 gr Nosler Partition, 30° downhill

Inputs: 180 gr, Ø 0.308 in, G1 BC 0.474, MV 823 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, shot angle -30°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 1.02 | 1.69 | -0.67 | -0.067 | 0.00 | 0.00 | +0.00 | 760.0 | 760.1 | 0.1264 | 0.1264 |
| 200 | -7.69 | -7.02 | -0.67 | -0.034 | 0.00 | 0.00 | +0.00 | 699.7 | 699.7 | 0.2636 | 0.2635 |
| 300 | -33.79 | -33.11 | -0.68 | -0.023 | 0.00 | 0.00 | +0.00 | 641.9 | 641.9 | 0.4128 | 0.4128 |
| 500 | -151.93 | -151.24 | -0.69 | -0.014 | 0.00 | 0.00 | +0.00 | 534.6 | 534.5 | 0.7543 | 0.7543 |

### C08: 9.3×62 286 gr, low BC heavy bullet, 60 mm sight

Inputs: 286 gr, Ø 0.366 in, G1 BC 0.33, MV 720 m/s, sight 60 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -4.42 | -4.42 | -0.00 | 636.5 | 636.5 | 0.1477 | 0.1477 |
| 200 | -18.37 | -18.37 | -0.00 | -0.000 | -18.83 | -18.83 | -0.00 | 558.9 | 558.9 | 0.3154 | 0.3154 |
| 300 | -68.34 | -68.35 | +0.00 | +0.000 | -45.16 | -45.16 | +0.00 | 488.1 | 488.0 | 0.5070 | 0.5070 |
| 500 | -305.57 | -305.60 | +0.03 | +0.001 | -141.64 | -141.68 | +0.05 | 374.0 | 373.8 | 0.9777 | 0.9778 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 286 gr, G1 BC 0.33, MV 720 m/s, sight 6.0 cm, zero 100 m, 15.0 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 4.3 | 4.5 | -0.2 | 637.9 | 638 | -0.1 | 0.1476 |
| 200 | -18.2 | -18.1 | -0.1 | -0.007 | 18.1 | 18.0 | +0.1 | 561.6 | 561 | +0.6 | 0.3147 |
| 300 | -67.8 | -68.1 | +0.3 | +0.010 | 43.5 | 44.0 | -0.5 | 491.7 | 492 | -0.3 | 0.5050 |
| 400 | -158.2 | -157.8 | -0.4 | -0.009 | 82.2 | 82.2 | +0.0 | 429.9 | 430 | -0.1 | 0.7228 |
| 500 | -301.9 | -303.6 | +1.7 | +0.035 | 136.1 | 136.5 | -0.4 | 378.1 | 378 | +0.1 | 0.9712 |

### C09: .243 Win 100 gr InterLock BTSP, −20 °C, 200 yd zero, 40 mm sight

Inputs: 100 gr, Ø 0.243 in, G1 BC 0.405, MV 902 m/s, sight 40 mm, zero 182.88 m, -20 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.3943 kg/m3, a 318.96 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 4.25 | 4.26 | -0.00 | -0.000 | -2.89 | -2.89 | +0.00 | 815.0 | 815.0 | 0.1167 | 0.1167 |
| 200 | -2.32 | -2.32 | -0.00 | -0.000 | -12.12 | -12.13 | +0.01 | 733.8 | 733.7 | 0.2460 | 0.2460 |
| 300 | -27.18 | -27.19 | +0.01 | +0.000 | -28.68 | -28.70 | +0.02 | 657.5 | 657.4 | 0.3900 | 0.3900 |
| 500 | -151.28 | -151.32 | +0.04 | +0.001 | -89.11 | -89.16 | +0.05 | 518.9 | 518.7 | 0.7326 | 0.7327 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 100 gr, G1 BC 0.405, MV 902 m/s, sight 4.0 cm, zero 182.88 m, -20 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 4.2 | 4.2 | +0.0 | +0.004 | 2.8 | 2.9 | -0.1 | 816.2 | 815 | +1.2 | 0.1166 |
| 200 | -2.3 | -2.3 | -0.0 | -0.000 | 11.7 | 11.9 | -0.2 | 735.9 | 732 | +3.9 | 0.2456 |
| 300 | -27.0 | -27.3 | +0.3 | +0.009 | 27.8 | 28.6 | -0.8 | 660.5 | 656 | +4.5 | 0.3891 |
| 400 | -74.4 | -74.6 | +0.2 | +0.006 | 52.1 | 53.1 | -1.0 | 589.5 | 583 | +6.5 | 0.5493 |
| 500 | -150.1 | -152.7 | +2.6 | +0.053 | 86.1 | 88.9 | -2.8 | 523.2 | 516 | +7.2 | 0.7294 |

### C10: .223 Rem 55 gr V-MAX, fast and light, 60 mm sight

Inputs: 55 gr, Ø 0.224 in, G1 BC 0.255, MV 988 m/s, sight 60 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -3.72 | -3.72 | -0.00 | 857.8 | 857.9 | 0.1087 | 0.1087 |
| 200 | -7.44 | -7.44 | -0.00 | -0.000 | -15.91 | -15.91 | -0.00 | 739.4 | 739.4 | 0.2343 | 0.2342 |
| 300 | -32.98 | -32.98 | -0.00 | -0.000 | -38.53 | -38.52 | -0.01 | 630.7 | 630.8 | 0.3807 | 0.3807 |
| 500 | -168.96 | -168.94 | -0.03 | -0.001 | -126.43 | -126.40 | -0.02 | 445.8 | 445.8 | 0.7589 | 0.7589 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 55 gr, G1 BC 0.255, MV 988 m/s, sight 6.0 cm, zero 100 m, 15.0 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 3.6 | 3.5 | +0.1 | 860.1 | 860 | +0.1 | 0.1085 |
| 200 | -7.4 | -7.4 | +0.0 | +0.002 | 15.3 | 15.3 | +0.0 | 743.5 | 743 | +0.5 | 0.2336 |
| 300 | -32.6 | -32.7 | +0.1 | +0.002 | 37.1 | 37.2 | -0.1 | 636.4 | 636 | +0.4 | 0.3790 |
| 400 | -82.4 | -82.3 | -0.1 | -0.002 | 71.3 | 71.2 | +0.1 | 538.9 | 539 | -0.1 | 0.5498 |
| 500 | -166.3 | -167.8 | +1.5 | +0.031 | 121.1 | 121.5 | -0.4 | 453.1 | 452 | +1.1 | 0.7524 |

### C11: .300 Win Mag 200 gr ELD-X, G7 0.301, hot and high, to 1000 m

Inputs: 200 gr, Ø 0.308 in, G7 BC 0.301, MV 869 m/s, sight 50 mm, zero 100 m, 30 °C, 794.95 hPa station, RH 30 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 0.9080 kg/m3, a 349.04 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -1.34 | -1.34 | -0.00 | 829.9 | 829.9 | 0.1178 | 0.1178 |
| 200 | -9.25 | -9.25 | +0.00 | +0.000 | -5.49 | -5.49 | -0.00 | 791.7 | 791.7 | 0.2411 | 0.2411 |
| 300 | -34.16 | -34.16 | +0.00 | +0.000 | -12.64 | -12.64 | -0.00 | 754.5 | 754.5 | 0.3705 | 0.3705 |
| 500 | -137.50 | -137.50 | +0.00 | +0.000 | -36.86 | -36.86 | +0.00 | 683.2 | 683.2 | 0.6491 | 0.6491 |
| 800 | -456.59 | -456.59 | -0.00 | -0.000 | -101.78 | -101.78 | -0.00 | 583.8 | 583.8 | 1.1242 | 1.1242 |
| 1000 | -809.18 | -809.17 | -0.02 | -0.000 | -167.92 | -167.90 | -0.02 | 521.7 | 521.8 | 1.4866 | 1.4866 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 200 gr, G1 BC 0.597, MV 869 m/s, sight 5.0 cm, zero 100 m, 30 °C, altitude 2000 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (784.5 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 1.3 | 1.5 | -0.2 | 830.5 | 831 | -0.5 | 0.1177 |
| 200 | -9.2 | -9.3 | +0.1 | +0.004 | 5.3 | 5.6 | -0.3 | 793.1 | 793 | +0.1 | 0.2409 |
| 300 | -34.1 | -34.1 | +0.0 | +0.001 | 12.2 | 12.4 | -0.2 | 756.7 | 757 | -0.3 | 0.3700 |
| 400 | -76.0 | -75.6 | -0.4 | -0.011 | 22.2 | 22.0 | +0.2 | 721.3 | 721 | +0.3 | 0.5054 |
| 500 | -136.9 | -137.0 | +0.1 | +0.003 | 35.4 | 35.7 | -0.3 | 686.7 | 686 | +0.7 | 0.6474 |
| 600 | -218.5 | -218.9 | +0.4 | +0.006 | 52.3 | 52.8 | -0.5 | 653.1 | 653 | +0.1 | 0.7968 |
| 700 | -323.2 | -323.9 | +0.7 | +0.010 | 72.9 | 73.4 | -0.5 | 620.5 | 620 | +0.5 | 0.9539 |
| 800 | -453.4 | -452.6 | -0.8 | -0.010 | 97.7 | 97.5 | +0.2 | 588.9 | 588 | +0.9 | 1.1193 |
| 900 | -611.9 | -610.3 | -1.6 | -0.017 | 126.9 | 126.6 | +0.3 | 558.3 | 557 | +1.3 | 1.2937 |
| 1000 | -801.8 | -803.9 | +2.1 | +0.021 | 160.8 | 161.6 | -0.8 | 528.9 | 528 | +0.9 | 1.4778 |

### C12: .338 Lapua Mag 300 gr Berger Hybrid, G7 0.419, to 1500 m (transonic)

Inputs: 300 gr, Ø 0.338 in, G7 BC 0.419, MV 820 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, wind 5 m/s from 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.00 | -0.00 | +0.00 | +0.000 | -1.40 | -1.40 | +0.00 | 783.7 | 783.7 | 0.1247 | 0.1247 |
| 200 | -10.98 | -10.98 | +0.00 | +0.000 | -5.72 | -5.72 | +0.00 | 748.2 | 748.2 | 0.2553 | 0.2553 |
| 300 | -39.50 | -39.50 | +0.00 | +0.000 | -13.17 | -13.17 | +0.00 | 713.7 | 713.7 | 0.3922 | 0.3922 |
| 500 | -156.30 | -156.30 | +0.01 | +0.000 | -38.34 | -38.35 | +0.01 | 647.5 | 647.4 | 0.6864 | 0.6865 |
| 800 | -514.07 | -514.10 | +0.03 | +0.000 | -105.70 | -105.72 | +0.02 | 554.8 | 554.8 | 1.1870 | 1.1871 |
| 1000 | -907.50 | -907.56 | +0.06 | +0.001 | -174.25 | -174.27 | +0.02 | 496.8 | 496.8 | 1.5680 | 1.5680 |
| 1200 | -1460.86 | -1460.93 | +0.07 | +0.001 | -265.89 | -265.90 | +0.01 | 441.5 | 441.5 | 1.9952 | 1.9952 |
| 1500 | -2688.52 | -2688.59 | +0.07 | +0.000 | -457.21 | -457.22 | +0.01 | 364.7 | 364.7 | 2.7437 | 2.7437 |

**Ours (Federal-matched inputs) vs Federal Premium web calculator** — Federal inputs: metric handload, 300 gr, G1 BC 0.818, MV 820 m/s, sight 5.0 cm, zero 100 m, 15.0 °C, altitude 0 m, wind 5 m/s (Federal: full crosswind, rounded to 11 mph). Ours re-run with Federal's effective air (999.9 hPa, RH 78 %, see CMP-01) and wind 4.917 m/s 90°.

| m | drop ours cm | drop Federal cm | Δ cm | Δ mil | drift ours cm | drift Federal cm | Δ cm | v ours m/s | v Federal | Δ m/s | ours ToF s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | -0.0 | 0.0 | -0.0 | -0.000 | 1.4 | 1.4 | -0.0 | 783.6 | 784 | -0.4 | 0.1247 |
| 200 | -11.0 | -11.0 | +0.0 | +0.001 | 5.6 | 5.5 | +0.1 | 748.2 | 748 | +0.2 | 0.2554 |
| 300 | -39.5 | -39.3 | -0.2 | -0.007 | 12.9 | 12.6 | +0.3 | 713.7 | 714 | -0.3 | 0.3922 |
| 400 | -87.3 | -87.1 | -0.2 | -0.005 | 23.6 | 23.5 | +0.1 | 680.1 | 680 | +0.1 | 0.5357 |
| 500 | -156.3 | -156.8 | +0.5 | +0.010 | 37.7 | 38.0 | -0.3 | 647.4 | 647 | +0.4 | 0.6864 |
| 600 | -248.7 | -248.5 | -0.2 | -0.003 | 55.6 | 55.6 | +0.0 | 615.6 | 615 | +0.6 | 0.8448 |
| 700 | -367.0 | -366.3 | -0.7 | -0.011 | 77.6 | 77.1 | +0.5 | 584.8 | 584 | +0.8 | 1.0115 |
| 800 | -514.1 | -513.0 | -1.1 | -0.013 | 104.0 | 103.5 | +0.5 | 555.0 | 554 | +1.0 | 1.1871 |
| 900 | -693.0 | -693.5 | +0.5 | +0.006 | 135.0 | 135.3 | -0.3 | 526.2 | 526 | +0.2 | 1.3722 |
| 1000 | -907.4 | -911.7 | +4.3 | +0.043 | 171.1 | 172.4 | -1.3 | 498.6 | 498 | +0.6 | 1.5675 |
| 1100 | -1161.3 | -1165.1 | +3.8 | +0.034 | 212.5 | 213.4 | -0.9 | 472.2 | 472 | +0.2 | 1.7736 |
| 1200 | -1459.3 | -1461.7 | +2.4 | +0.020 | 259.6 | 259.7 | -0.1 | 447.3 | 447 | +0.3 | 1.9914 |
| 1300 | -1806.4 | -1814.5 | +8.1 | +0.062 | 312.7 | 313.3 | -0.6 | 423.8 | 423 | +0.8 | 2.2212 |
| 1400 | -2208.2 | -2222.3 | +14.1 | +0.101 | 372.0 | 373.0 | -1.0 | 402.1 | 402 | +0.1 | 2.4637 |
| 1500 | -2670.8 | -2687.7 | +16.9 | +0.113 | 437.6 | 437.8 | -0.2 | 382.2 | 382 | +0.2 | 2.7192 |

### C12s: .338 Lapua Mag 300 gr, spin drift (1.75 in, 1:10 RH) and Coriolis (65° N, shooting east)

Inputs: 300 gr, Ø 0.338 in, G7 BC 0.419, MV 820 m/s, sight 50 mm, zero 100 m, 15.0 °C, 1013.25 hPa station, RH 0 %, spin drift on (length 1.75 in, 1:10 in RH), Coriolis 65° N, azimuth 90°. Aerodynamic jump off. ρ and speed of sound: `rho 1.2250 kg/m3, a 340.30 m/s`.

**Ours vs py-ballisticcalc 3.1.1 (same inputs)**

| m | drop ours cm | drop pbc cm | Δ cm | Δ mil | drift ours cm | drift pbc cm | Δ cm | v ours m/s | v pbc | ToF ours s | ToF pbc s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 100 | 0.04 | 0.04 | -0.00 | -0.000 | 0.08 | 0.29 | -0.21 | 783.7 | 783.7 | 0.1247 | 0.1247 |
| 200 | -10.83 | -10.83 | +0.00 | +0.000 | 0.70 | 1.12 | -0.43 | 748.2 | 748.2 | 0.2553 | 0.2553 |
| 300 | -39.14 | -39.15 | +0.00 | +0.000 | 1.85 | 2.49 | -0.64 | 713.7 | 713.7 | 0.3922 | 0.3922 |
| 500 | -155.28 | -155.29 | +0.01 | +0.000 | 5.94 | 7.00 | -1.06 | 647.5 | 647.4 | 0.6864 | 0.6865 |
| 800 | -511.33 | -511.36 | +0.03 | +0.000 | 17.31 | 19.01 | -1.70 | 554.8 | 554.8 | 1.1870 | 1.1870 |
| 1000 | -903.06 | -903.12 | +0.06 | +0.001 | 29.25 | 31.38 | -2.13 | 496.8 | 496.8 | 1.5680 | 1.5680 |
| 1200 | -1454.23 | -1454.30 | +0.07 | +0.001 | 45.64 | 48.20 | -2.56 | 441.5 | 441.5 | 1.9952 | 1.9952 |
| 1500 | -2677.51 | -2677.58 | +0.07 | +0.000 | 81.25 | 84.45 | -3.19 | 364.7 | 364.7 | 2.7437 | 2.7437 |

### Transonic and subsonic: .338 Lapua Mag 300 gr G7 0.419 beyond 1500 m (ours vs py-ballisticcalc)

Same inputs as C12. The bullet is transonic from about 1000 m (Mach 1.4) and crosses Mach 1 at about 1600 m.

| m | Mach (ours) | drop ours cm | drop pbc cm | Δ cm | Δ mil | v ours | v pbc | ToF ours | ToF pbc |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1500 | 1.071 | -2688.5 | -2688.6 | +0.07 | +0.0005 | 364.7 | 364.7 | 2.7437 | 2.7437 |
| 1700 | 0.956 | -3863.8 | -3863.9 | +0.13 | +0.0008 | 325.3 | 325.2 | 3.3287 | 3.3289 |
| 1800 | 0.926 | -4586.4 | -4586.6 | +0.23 | +0.0013 | 315.5 | 315.5 | 3.6420 | 3.6422 |
| 1900 | 0.903 | -5408.1 | -5408.4 | +0.32 | +0.0017 | 307.6 | 307.7 | 3.9642 | 3.9643 |
| 2000 | 0.883 | -6334.1 | -6334.5 | +0.35 | +0.0017 | 300.7 | 300.8 | 4.2945 | 4.2945 |
| 2200 | 0.847 | -8519.9 | -8520.0 | +0.18 | +0.0008 | 288.4 | 288.5 | 4.9780 | 4.9778 |

The G1 and G7 tables in `drag.rs` were also compared node by node with
py-ballisticcalc's `TableG1` (79 points) and `TableG7` (84 points) from Mach 0
to 5. The largest difference is 5e-6, so the drag tables are the same.

## Reproducing

```sh
S=/private/tmp/claude-501/-Users-dro-dev-ballistic-calculator/ee6308e0-d91e-4121-9b9a-2cd6c7a550ac/scratchpad/engine-compare
cd $S/harness && cargo build --release           # our engine, by path
python3 -m venv $S/venv && $S/venv/bin/pip install py-ballisticcalc==3.1.1
$S/venv/bin/python -I $S/scripts/run_pbc.py       # all cases, ours vs py-ballisticcalc
python3 -I $S/scripts/federal_batch.py            # Federal web form (network; ~5 s between requests)
python3 -I $S/scripts/compare_federal.py          # ours with Federal's effective air vs Federal
$S/venv/bin/python -I $S/scripts/transonic.py
python3 -I $S/scripts/tables.py                   # rebuilds the tables in this report
# one scenario by hand:
$S/harness/target/release/engine-compare mass_gr=150 bc=0.338 model=G1 mv_mps=859.5 sight_mm=50 \
  zero_m=100 temp_c=15 press_hpa=1013.25 hum=0 wind_mps=5 wind_deg=90 ranges=100,200,300,500
```

## Appendix: harness source (`harness/src/main.rs`)

`Cargo.toml`: package `engine-compare`, edition 2024, an empty `[workspace]`,
and `ballistics-core = { path = "/Users/dro/dev/ballistic calculator/crates/ballistics-core" }`.

```rust
//! Prints a range table from MX Shooting Calculator's engine for one scenario.
//! Usage: engine-compare key=value ...  (all keys optional, see defaults below)
//! Units: mass_gr, diam_in, len_in (0 = unknown), bc (lb/in²), model G1|G7,
//! mv_mps, sight_mm, zero_m, temp_c, press_hpa (station), hum (0..1),
//! zero_temp_c/zero_press_hpa/zero_hum (default = shot air),
//! wind_mps, wind_deg (from, 0 = head, 90 = from right), incl_deg,
//! lat/az (Coriolis, off if absent), spin 0|1, jump 0|1, twist_in, ranges=100,200,...
//! Output: one JSON object per range.
use ballistics_core::*;
use std::collections::HashMap;

fn main() {
    let args: HashMap<String, String> = std::env::args()
        .skip(1)
        .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    if args.contains_key("cd_dump") {
        let mut m = 0.0;
        while m <= 5.0001 {
            println!("{:.3} {:.5} {:.5}", m, DragModel::G1.drag_coefficient(m), DragModel::G7.drag_coefficient(m));
            m += 0.025;
        }
        return;
    }
    let f = |k: &str, d: f64| args.get(k).map(|v| v.parse::<f64>().unwrap()).unwrap_or(d);
    let temp = f("temp_c", 15.0);
    let press = f("press_hpa", 1013.25);
    let hum = f("hum", 0.0);
    let shot_air = Atmosphere { temperature_c: temp, pressure_hpa: press, humidity: hum };
    let zero_air = Atmosphere {
        temperature_c: f("zero_temp_c", temp),
        pressure_hpa: f("zero_press_hpa", press),
        humidity: f("zero_hum", hum),
    };
    let len_in = f("len_in", 0.0);
    let model = match args.get("model").map(String::as_str).unwrap_or("G1") {
        "G7" => DragModel::G7,
        _ => DragModel::G1,
    };
    let coriolis = args.get("lat").map(|_| Coriolis {
        latitude_degrees: f("lat", 0.0),
        azimuth_degrees: f("az", 0.0),
    });
    let scenario = Scenario {
        bullet: Bullet {
            mass_kg: f("mass_gr", 150.0) / 15_432.358,
            diameter_m: f("diam_in", 0.308) * 0.0254,
            length_m: (len_in > 0.0).then_some(len_in * 0.0254),
            ballistic_coefficient: f("bc", 0.4),
            drag_model: model,
            bc_bands: BcBands::NONE,
            drag_curve: None,
        },
        rifle: Rifle {
            sight_height_m: f("sight_mm", 50.0) / 1000.0,
            zero_range_m: f("zero_m", 100.0),
            zero_height_m: 0.0,
            twist_m: f("twist_in", 10.0) * 0.0254,
            right_hand_twist: f("rh", 1.0) != 0.0,
        },
        load: Load {
            muzzle_velocity: f("mv_mps", 800.0),
            reference_temperature_c: 15.0,
            temperature_sensitivity: 0.0,
        },
        zero_atmosphere: zero_air,
        zero_powder_temperature_c: 15.0,
        impact_offset: [0.0, 0.0],
        shot: Conditions {
            atmosphere: shot_air,
            powder_temperature_c: 15.0,
            wind: Wind { speed: f("wind_mps", 0.0), from_degrees: f("wind_deg", 90.0) },
            inclination_degrees: f("incl_deg", 0.0),
            coriolis,
            cant_degrees: 0.0,
        },
        spin_drift: f("spin", 0.0) != 0.0,
        aerodynamic_jump: f("jump", 0.0) != 0.0,
    };
    let ranges: Vec<f64> = args
        .get("ranges")
        .map(|s| s.split(',').map(|r| r.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![100.0, 200.0, 300.0, 500.0]);
    let max = ranges.iter().cloned().fold(0.0, f64::max) + 5.0;
    let sol = solver::solve(&scenario, max).expect("solve");
    eprintln!(
        "# muzzle {:.1} m/s, zero elevation {:.4} mrad, Sg {:?}, rho {:.4} kg/m3, a {:.2} m/s",
        sol.muzzle_velocity,
        sol.zero_elevation * 1000.0,
        sol.stability,
        shot_air.density(),
        shot_air.speed_of_sound()
    );
    for r in ranges {
        let row = sol.at(r).expect("range");
        println!(
            "{{\"range\":{},\"drop_cm\":{:.3},\"wind_cm\":{:.3},\"elev_mil\":{:.4},\"wind_mil\":{:.4},\"elev_moa\":{:.4},\"wind_moa\":{:.4},\"v_mps\":{:.2},\"e_j\":{:.1},\"tof_s\":{:.5},\"mach\":{:.4}}}",
            r,
            row.drop * 100.0,
            row.windage * 100.0,
            row.elevation_correction * 1000.0,
            row.windage_correction * 1000.0,
            row.elevation_correction * 10800.0 / std::f64::consts::PI,
            row.windage_correction * 10800.0 / std::f64::consts::PI,
            row.velocity,
            row.energy_j,
            row.time,
            row.mach
        );
    }
}
```
