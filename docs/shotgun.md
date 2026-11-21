# Shotguns in the engine

`ballistics_core::pellets` flies shot, spreads patterns, counts hits on the
vitals and finds how far shot can carry. The app doesn't use it yet; the
shooting simulator calls it through its FFI. The research behind every
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
  - the shot string: pellets behind the front ones start slower and draft.
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
