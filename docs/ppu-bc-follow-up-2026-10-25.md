# PPU BC follow-up — 2026-10-25

The user approved the recommendation: A-689 is now G1 0.535 and A-718 is
now G1 0.400. Their notes preserve the original ammunition-table values and
the reason for the correction. A subsequent decision also approved A-594 G1 0.485; its note records the
moderate-confidence reasoning. A-721 retains 0.519.
The table below preserves the pre-change snapshot.
The separately authorized
Fiocchi #709959 correction has been made: G7 0.310 → 0.243, supported by its
[official product page](https://fiocchi.com/en/709959.html).

## Evidence and conclusion

The [PPU 2023 catalog](https://www.prvipartizan.com/download/PPU_2023.pdf), printed
pp. 40–41 (PDF page index 20), identifies the projectiles and supplies these BCs
and velocities. The [official download page](https://www.prvipartizan.com/download.php)
still links that catalog. Current website selectors did not expose an alternative
numerical table in the browser extraction.

| Load | Identified bullet | Original stored G1 | Component BC | G1 fitted to published velocities |
| --- | --- | ---: | ---: | ---: |
| A-689, Creedmoor 140 gr | B-689 | 0.400 | 0.535 | 0.529 |
| A-718, Creedmoor 120 gr | B-540 | 0.519 | 0.400 | 0.406 |
| A-540, Swedish 120 gr | B-540 | 0.400 | 0.400 | 0.403 |
| A-721, Swedish 140 gr | B-689 | 0.519 | 0.535 | 0.527 |
| A-594, .308 156 gr | B-594 | 0.450 | 0.485 | 0.493 |

Inference: the two Creedmoor BC cells likely contain a publication error.
Recommend A-689 **0.535** and A-718 **0.400**, using the identified component
values rather than introducing fitted numbers. A simple swap is insufficient:
0.519 would still differ from the 140 gr component's 0.535. The smaller A-721
and A-594 discrepancies were decided separately: keep A-721 at 0.519 and
use component G1 0.485 for A-594.

## Calculation and limits

Diagnostic only: constant G1 BC, still air, density 1.225 kg/m³ and sound speed
340.294 m/s, horizontal one-dimensional deceleration. Use the existing core's
G1 Cd table with linear interpolation and BC conversion 703.06958 kg/m² per
lb/in². Integrate
`dv/dx = -rho*pi*Cd(v/a)*v/(8*BC*703.06958)` with RK4, step 0.5 m.
Minimize squared velocity residuals at 100, 200 and 300 m; muzzle velocity is
fixed to the published value. A 0.25 m repeat changed predicted velocities by
less than 0.000001 m/s. Fits above are rounded, not uncertainty estimates.

For reproducibility, the input vectors (m/s, muzzle through 300 m) were:
`A-689 [820,763,709,656]`, `A-718 [885,808,735,666]`,
`A-540 [830,755,684,618]`, `A-721 [775,720,667,616]`,
`A-594 [835,773,715,658]`.

At 300 m, stored BCs predict 608 versus the table's 656 m/s for A-689,
and 710 versus 666 m/s for A-718. Component BCs predict approximately
658 and 663 m/s respectively. These discrepancies are much larger than the
smaller comparison-load differences.

Atmospheric conditions underlying PPU's table were not established. Rounded
tabular velocities may be calculated from BCs, so this establishes internal
consistency only; it is not independent evidence of real projectile drag.
No solver code was modified. The three approved PPU BCs and their source
notes were changed; existing catalog assertions were updated accordingly.
