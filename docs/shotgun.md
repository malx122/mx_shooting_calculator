# Shotguns in the engine

`ballistics_core::pellets` flies shot, spreads patterns, counts hits on the
vitals and finds how far shot can carry. The app's Guns page uses it for
shotguns (see [In the app](#in-the-app)); the shooting simulator calls it
through its FFI. The research behind every
constant is in [shotgun-research/](shotgun-research/):
[flight](shotgun-research/flight.md), [patterns](shotgun-research/patterns.md)
and [lethality and the Nordic rules](shotgun-research/lethality.md) (all
2026-11-21). Each number there has its source.

## One pellet's flight

- **Drag:** a smooth sphere, the correlation of Loth et al. (AIAA Journal 59(8),
  2021) at a Reynolds number of 5·10⁴. Pellets fly at about 2·10⁴–1.2·10⁵,
  below the drag crisis, where the Reynolds number moves the velocity at
  35–45 m by under 1 %, so it isn't modelled.
- **Checked against:**
  - Lowry's Winchester table (steel BB–4, lead 2–6), within 5 % to 40 yards and
    6 % at 50;
  - Eley's striking velocities (English BB–7), within 2.5 % at 40 m and 4 % at
    35 m;
  - Gough Thomas's pellet energies from an observed velocity, within 8 %;
  - Lowry's wind drift for No. 8, within 15 %;
  - the drift is the wind times the lag, as Didion's rule says.
- **Not modelled:**
  - fired lead is dented and drags up to about 5 % more than a sphere;
  - published tables for shot under 2.3 mm disagree by ±10 %;
  - one pellet's flight is the front pellets'; the swarm below gives the
    others theirs.
- **Where a path ends:** at the range asked for, or where the pellet falls
  more steeply than 60° across the line of sight, or after 15 s. A shot's path
  goes on however slowly the pellet moves; a bullet's ends below 20 m/s.
- **Published velocities:** `muzzle_velocity` turns a C.I.P. V2.5, an American
  3-foot velocity or a British observed velocity (the average over 20 yards)
  into a muzzle velocity.
  - V2.5 is 25–35 m/s below the muzzle velocity for birdshot.
  - An observed velocity is about 80 m/s below it.
- **Materials and sizes:**
  - `Material` gives the densities of hardened lead, steel, bismuth, tungsten-iron,
    Tungsten-Matrix, TSS and zinc.
  - `US_SIZES` and `ENGLISH_SIZES` hold the diameters per number.
  - `nordic_diameter` gives the millimetres continental makers print for US
    numbers on Nordic boxes ("US 5 / 3,0 mm").
- **Danger range:** `max_range` finds the farthest a pellet carries over level
  ground and the elevation that does it (about 23–28° in still air, steeper
  in a tailwind). It flies 2 ms steps for up to 300 s.
  - It agrees with Journée's rule (79 m per millimetre of lead) within 7 % for
    2–4 mm.
  - Steel carries about three quarters as far as lead of its size, buckshot
    about 550 m.

## Patterns

- **Spread:** a pattern is a round normal spread about its centre. Its σ grows
  as σ₄₀ · x · (1 + q(x − 1)):
  - x is the range over 40 yards;
  - σ₄₀ puts the choke's percentage in the 30-inch circle at 40 yards;
  - q is 0.011 per point that percentage is above a cylinder's 40.
- **What the choke does:** it mostly delays the start of the spread. In Gough
  Thomas's table improved cylinder is true cylinder moved 5 yards out, half
  choke is improved cylinder moved 5 more, and full is half moved 5 more.
- **Fit:** against that table (its rounded-up 100s read as at least 92) the
  model errs by 1.1 points RMS and 2.6 at worst.
  A constant angle, as `pattern` still gives for the simulator's replays, errs
  by up to 8 points for full choke.
- **Range:** below about 25 m the model is a little tight for open borings. It
  is extrapolated beyond 55 m, no choke spreads wider than a cylinder, and it
  stops at 150 m.
- **Choke names:** `Choke` maps names to nominal percentages.
  - `percent_from_constriction` maps a 12-bore constriction to one; names
    are more reliable than constrictions.
  - `steel_percent` adds the 10 points steel patterns tighter, up to 72.
  - `Choke::suits_steel` follows the advice of no tighter than half choke
    with steel.
  - `percent_from_nordic` converts the Nordic 75 cm / 35 m test.

## The swarm

A shot's pellets don't arrive together. `Swarm` flies each pellet on its
own (research and sources:
[shotgun-research/swarm.md](shotgun-research/swarm.md)).

- **Per pellet:** a start speed `V·(1 − a|z₁|)` and a drag `Cd·(1 + b|z₂|)`,
  both half-normal, so nothing outruns the front pellets and the string has
  a hard front and a long tail. The string isn't drawn; it comes from the
  flights.
  - Soft lead a = 3.5 %, b = 0.12; hardened game lead a = 2 %, b = 0.10;
    steel a = 5 %, b = 0.04. Bismuth, tungsten and zinc are taken as hard
    lead, for want of data.
  - More drag is flown as less density at the same diameter. A 7 × 7 grid
    of flights is read between for each pellet, within 0.1 ms of flying it
    alone.
- **Direction:** a round normal of the choke's spread, kept for the whole
  flight. The mean spread is the choke law's; within it a tight choke's
  pellets spread in proportion to their own flight time (Compton: σ ∝ t),
  so slow, dented pellets land later and wider. A cylinder's spread at a
  fixed angle.
- **From shot to shot:** the front velocity varies by 1.1 % for lead and
  1.7 % for steel, the pattern's size by 4 % for small shot and up to 17.5 %
  for BB and larger.
- **Checked against:**
  - Lowry's string lengths at 40 yards (79, 56 and 55 inches holding 80 %),
    within 20 %, and their growth to 65 yards (1.6–2.2 times; Lowry 1.96);
  - Lowry's table A, within 15 %: a closer fit overshoots Compton;
  - Compton's lead and steel cloud lengths at 20–50 m, within 20 and 25 %;
  - Compton's trailing edge at 20 m, 11–15 % slower than the front for lead
    and 7 % for steel: the pellet trailing 95 in 100 is 9–15 % and 5–10 %
    slower;
  - Compton's arrival-time peaks, 2–7 ms behind the front at 20–50 m, within
    1.5 ms;
  - Lowry's and Brister's leads at 40 yards, within 10 % (a check of the
    flight time more than of the string);
  - Compton's spread of 30-inch counts from shot to shot (SD 7.5–12.5).
- **Not modelled:** clumping (Compton's counts are Poisson), a separate
  group of fliers, and how far a pattern's centre wanders from shot to shot:
  no data.
- **A crossing target:** each pellet lands where the target is when that
  pellet arrives, so the string smears the pattern along the target's path.

## Hits and effective range

- **Hits:** pellets land on a vital circle independently, so hits are
  binomial.
  - `chance_of_hits` averages the chance of at least *n* hits over the hunter's
    aim error.
  - When the expected hits equal the hits needed, the chance is only 55–60 %.
- **Effective range:** `effective_range` is the farthest whole metre, out to
  100 m, where every pellet still has the quarry's energy and the chance of
  its hits is high enough.
- **Example:** 36 g of US 3 lead from half choke on a roe deer is enough to
  25 m. That matches the Finnish advice (25 m) and the Danish (20 m).
  - The quarry: an 18 cm circle (the Swedish hunting test's), 5 hits of 6 J,
    nine times in ten, 5 mrad aim error.
  - The 18 cm circle is sourced; the 5 hits and 6 J are assumptions, near the
    German Wannsee norm for hare. The research lists what is known per
    species.
- **Choke and aim:** a tighter choke reaches farther only for a steady aim. With
  a hunter's aim error its denser middle misses more often, as BASC found.

## In the app

Shotguns are kept and calculated as rifles are.

- **Calibers:** `catalog/shotgun-calibers.json` holds 20 shotgun calibers
  (gauge and chamber length, 10/89 to .410/76) with C.I.P.'s least bore and
  chamber, pressures and steel-shot limits (decision XXXII-45, 2014);
  sources and gaps in [shotgun-research/calibers.md](shotgun-research/calibers.md).
  How common each is in Nordic hunting is a judgement. A shell fits a gun of
  its gauge whose caliber is as long or longer (`ShotgunCaliber::fits`).
  Each caliber has a 3D shell with its C.I.P. measurements, drawn and
  printed like a rifle cartridge ([cartridges.md](cartridges.md#shotgun-shells)).
- **Catalog shells:** `catalog/shells/*.json`, one entry per pellet size of a
  load, from the makers' own pages: Gamebore
  ([notes](shotgun-research/gamebore.md)) and Norma, which took over
  Gyttorp's shells in 2019–20 ([notes](shotgun-research/gyttorp-norma.md)).
  A shell whose length has no caliber of its own is listed under the
  shortest it fits (Gamebore's 12/86 under 12/89). Gamebore's "V1" is taken
  as the velocity 1 m from the muzzle. Gamebore's own velocity-by-range
  chart runs 8–23 % faster than the engine at 20–80 m, while the engine
  matches Lowry's, Eley's and Gough Thomas's measured tables within 2–5 %;
  the chart looks modelled, so it isn't used.

- **Records:** a shotgun (`ballistics_store::Shotgun`) holds its barrels'
  chokes, as a rifle holds its sights. A shell (`Shell`) is ammunition for a
  gauge, as a bullet is for a caliber: shot or a slug, with the velocity as
  the box gives it and where it is measured. A `ShotgunLoad` is a shell in
  a gun, as a `RifleLoad` is a bullet in a rifle, and holds the pattern
  boards counted with it. All travel in backups and sync (export format 8).
- **Pages:** shotguns are in the Guns list with the rifles; shells are on
  the Ammunition page with the bullets, by gauge; Calculate takes a shotgun
  as it takes a rifle, with its shell and barrel, and the same range, wind,
  target and weather; the angle applies to slugs, and shot is worked out
  level (`crates/app/src/calculator/shotgun.rs`).
- **Working out:** `ballistics_shot::shotgun` turns a shell into the
  engine's inputs.
  - The muzzle velocity is back-solved from the box's.
  - A barrel's pattern comes from the hunter's boards for the shell in that
    gun (averaged), or else from its choke, 10 points tighter for hard
    shot. A constriction is read on the 12-bore scale, scaled to the bore.
  - Shot is flown level; a slug is aimed at the shot's angle, as a rifle is.
  - The shot's pellets are flown one by one (`Swarm`) out to 150 m: the
    table's string and back-of-the-string energy, and each shot on the
    animal, which may be crossing. The lead is to where the animal is when
    the middle of the swarm arrives.
  - Slugs fly in the rifle solver with their G1 BC, zeroed at the gun's slug
    zero (50 m when not set) over its sights (a 1.5 cm bead when not set).
- **The law and the advice:** `ballistics_store::rules::shotgun` keeps the
  Swedish (NFS 2023:8, 3 kap.) and Finnish (metsästysasetus 17–18 §,
  metsästyslaki 33 §) rules by the catalog's target ids, and the advice
  the app quotes, each with its source: Svenska Jägareförbundet's shot
  sizes per species, Riistakeskus' 25 m and 3.5–4.0 mm for roe deer,
  Metsästäjäliitto's 35 m, and the Swedish hunting test's circles. An
  animal the hunter made, not copied from the catalog, gets no verdict.
- **No verdict on the kill:** the app counts the pellets on an animal's hit
  area and gives the average, but doesn't say how many are enough: the
  hits and energy per species in the literature are too uncertain (old
  British rules, assumptions). `effective_range` and `Quarry` stay in the
  engine for callers who bring their own numbers.
