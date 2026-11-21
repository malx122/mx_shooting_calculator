# Group statistics

How MX Shooting Calculator measures a group of shots on a photographed
target: what each number means, the formula behind it, and how far to
trust it. Everything here is worked out by the `groups-core` crate, which
has its own tests against published tables; the sections name the
functions and types so the code is easy to find.

## 1. Conventions

### On the target

Shots are points **in millimetres on the target**, after the photo has been
scaled and, if needed, straightened:

- x grows to the **right**, y grows **up**, as on a graph.
- The **centre** of a group is the mean of its shots.
- The **impact** is where the centre is from the aim point:
  **impact = centre − aim**. A group above and right of the aim has a
  positive x and a positive y.
- The **correction** is the opposite, **aim − centre**: how far the group
  has to move to sit on the aim, up and right positive. A group 20 mm low
  needs a correction of +20 mm, up.

### In the photo

Photo pixels have x to the right and y **down**, as images are stored.
Only the calibration takes or gives pixels; it turns the y axis over.

### Angles

Sizes become angles with the distance from the muzzle to the target
(`angles`):

| Unit | Definition |
|---|---|
| mrad (mil) | 1 mm at 1 m, as reticles and turrets are made (the true angle differs by less than one part in a million at these sizes) |
| MOA | a true 1/60 degree: 1 mrad = 10.8/π = 3.437746771 MOA |
| cm per 100 m | the angle as a length at 100 m: 1 mrad = 10 cm per 100 m |

So 25 mm at 250 m is 0.1 mrad, 0.344 MOA or 1 cm per 100 m.

## 2. Calibration: from pixels to millimetres

`Calibration` is a projective map (a *homography*) between the photo and
the target, and its inverse. Every shot is stored in photo pixels, so a
better scale later never loses a shot.

### Two points

Two taps a known distance apart give one scale, mm per px = distance ÷
the taps' distance in pixels, the same everywhere. The photo is taken as
square-on and upright. The origin is the first tap.

### Four corners

Four taps on the corners of a rectangle of known width and height, in any
order, also straighten a photo taken at an angle:

1. **Sorting.** The taps are sorted by their angle round their mean, which
   puts them in order round the quad (clockwise on the screen), then
   turned so the first is the one nearest the photo's top-left. They come
   out as top-left, top-right, bottom-right, bottom-left, as the photo
   shows them. This tells the corners apart for a target turned up to 45°
   either way in the photo.
2. **Which side is which.** The width is the side along the top and
   bottom edges *as the photo is shown*, the height the side along its
   left and right edges, exactly as typed. Nothing is guessed from the
   quad's proportions: a near-square target photographed from the side
   can look taller than wide, and a guess would then swap the sides and
   put the scale several percent out on both axes. For a target shown a
   quarter turn round, the app will offer to turn the photo upright or to
   swap the two sizes.
3. **The map.** The rectangle's bottom-left corner (as the photo shows it)
   is the origin, x along its bottom edge, y up its left edge. The
   homography taking the four corners to the rectangle is found by the
   direct linear transform (Hartley and Zisserman 2004): eight equations
   in its eight unknowns, solved by Gaussian elimination with partial
   pivoting, after moving both sets of points to have their mean at the
   origin and a mean distance of √2 from it (Hartley 1997), which keeps
   the arithmetic well conditioned with pixel coordinates in the
   thousands.

### Both

- `to_mm` and `to_px` map a point either way. A pixel on or beyond the
  photo's horizon (the vanishing line of the target's plane) has no point
  on the target and gives none. `to_px` will let the app draw a straightened
  photo as a mesh: each corner of a grid on the target is looked up in the
  photo.
- `mm_per_px_at` gives the local scale at a pixel: the square root of how
  much the map scales an area there (the geometric mean of the two
  directions). A bullet-sized ring there is calibre ÷ mm per px pixels
  across.
- `resolution` is the sanity check:
  - the scale in the middle of the reference points;
  - the **coarsest** scale: the most millimetres one pixel spans in any
    direction (the larger singular value of the map's Jacobian), at the
    coarsest of the reference points. A photo taken at a steep angle can
    be fine across and coarse up and down; the geometric mean would hide
    that. More than **0.5 mm per px** counts as too coarse: a tap is good
    to about a pixel, so each shot would be half a millimetre out, and a
    .22 hole would be barely a dozen pixels across;
  - the scale's **uncertainty** from taps one pixel out: √2 px ÷ the
    distance between the two points, or ÷ the rectangle's shortest side
    in pixels. Above **1%** it is worth a warning (two points 141 px
    apart are at 1%).

### What is refused

A calibration is refused when a point or size is not a number, a size is
zero or negative, two points are closer than **30 px** (where one pixel of
tap error would put the scale about 5% out), three corners are on (or
within **3°** of) one line, or one corner is inside the triangle of the
other three.

## 3. The model

The ranges and the derived radii rest on the usual model for rifle groups:
**shots scatter round a centre as a normal distribution, with the same
standard deviation σ across and up, independently** (Grubbs 1964). σ is
the rifle-and-shooter's precision in one number; every size below is a
multiple of it under the model. The model is a good description of
mechanical precision; it does not know about a called flier, a change of
light, a fouled bore or wind that changed between shots.

## 4. What is measured

For n shots pᵢ (mm) with centre c:

| Statistic | Definition | Needs |
|---|---|---|
| Centre | c = mean(pᵢ) | 1 shot |
| Impact, correction | c − aim, aim − c | an aim point |
| Extreme spread (ES), centre to centre | the largest distance between two shots, with which two | 2 shots |
| ES, edge to edge | ES + calibre: what a ruler across the outside of the two holes reads | a calibre |
| Mean radius (MR) | mean of \|pᵢ − c\| | 1 shot |
| Median radius | median of \|pᵢ − c\| | 1 shot |
| Radial σ̂ | σ̂² = Σ\|pᵢ − c\|² ÷ (2(n − 1)) | 2 shots |
| SD-H, SD-V | the sample standard deviations of x and of y (dividing by n − 1) | 2 shots |
| Width, height | max − min of x and of y, centre to centre | 1 shot |
| Correlation | Σ(x − cx)(y − cy) ÷ √(Σ(x − cx)² Σ(y − cy)²), −1 to 1; positive when the group leans from bottom-left to top-right | 2 shots |
| Shape | the SDs along the group's longest and shortest axes (its *principal axes*) and their ratio, with the longest axis's angle from the right, anticlockwise | 2 shots |

Notes:

- **σ̂ is the best single measure of precision.** It uses every shot's
  distance, twice over (across and up), so it has 2(n − 1) degrees of
  freedom where SD-H has n − 1. Under the model, 2(n − 1) σ̂² / σ² follows
  χ² with 2(n − 1) degrees of freedom exactly. σ̂² is also the mean of
  SD-H² and SD-V², so turning the target does not change it.
- **ES is kept because everybody uses it,** and it is the noisiest
  measure: it rests on two shots and ignores the rest, and it grows with
  the number of shots, so a 3-shot ES and a 10-shot ES are not
  comparable.
- **MR, measured from the group's own centre, reads a little small**
  for few shots, because the centre is fitted to the shots: on average
  by a factor √((n − 1)/n) (0.82 for 3 shots, 0.89 for 5). Its range
  (below) is for the true value.
- The median radius will be shown beside CEP: for many shots they agree.
- **The principal axes** come from the scatter sums Sxx = Σ(x − cx)²,
  Syy = Σ(y − cy)² and Sxy = Σ(x − cx)(y − cy): the two variances along
  them are the eigenvalues of [[Sxx, Sxy], [Sxy, Syy]] ÷ (n − 1), and the
  longest axis lies at ½ atan2(2Sxy, Sxx − Syy). Unlike SD-H and SD-V they
  don't change when the target is turned, so a group strung along a
  diagonal shows as strung.

## 5. How sure the numbers are

σ̂, the mean radius, CEP, R90, R95, the extreme spread and the centre
(and so the impact) come with a **95% range**: under the model, ranges
made this way contain the true value (the one many more shots would
settle on) 95 times in 100. The ranges are exact under the model for any
n ≥ 2, with no resampling noise. The median radius, width, height, SD-H
and SD-V have none.

### σ

From the χ² distribution with ν = 2(n − 1) degrees of freedom:

  σ from σ̂ √(ν / χ²₀.₉₇₅(ν)) to σ̂ √(ν / χ²₀.₀₂₅(ν)).

How wide that is, as multiples of the measured σ̂:

| Shots | 95% range of σ |
|---|---|
| 2 | 0.52 to 6.29 × σ̂ |
| 3 | 0.60 to 2.87 × σ̂ |
| 5 | 0.68 to 1.92 × σ̂ |
| 10 | 0.76 to 1.48 × σ̂ |
| 20 | 0.82 to 1.29 × σ̂ |
| 30 | 0.85 to 1.22 × σ̂ |

So a 5-shot group's σ could be a third smaller or nearly twice as large as
it measured. That is not a flaw of the method; it is how much five shots
can say. A test checks by simulation (a fixed-seed Monte Carlo of 10,000
groups each of 2, 3, 5 and 10 shots) that the range does cover the true σ
95% of the time.

### Mean radius and CEP

The true mean radius is √(π/2) σ = 1.2533 σ and the true CEP 1.1774 σ, so
their ranges are σ's range times those factors. When CEP comes from
Grubbs's approximation (below), its range is σ's range times CEP ÷ σ̂.

### Extreme spread

The ES to expect from groups of n shots is a fixed multiple of σ. There is
no tidy formula for it, so the app carries a table for n = 2 to 50 from a
fixed-seed Monte Carlo of ten million strings of shots (good to about
±0.001; a test makes the table again and checks it):

| Shots | 2 | 3 | 4 | 5 | 7 | 10 | 15 | 20 | 30 | 50 |
|---|---|---|---|---|---|---|---|---|---|---|
| Expected ES in σ | 1.772 | 2.409 | 2.794 | 3.066 | 3.444 | 3.811 | 4.196 | 4.451 | 4.790 | 5.188 |

(For two shots it is exactly √π σ = 1.7725 σ, which the test checks.)
The ES range is σ's range times the expected ES for that many shots: it is
the range of the *typical* ES of such groups, not of this one group's
ES, which varies even more. Beyond 50 shots no ES range is given.

### Centre and impact

The centre's standard error on each axis is σ̂ / √n, and (true centre −
measured centre) ÷ that follows Student's t with 2(n − 1) degrees of
freedom, which gives each axis a range: centre ± t₀.₉₇₅ σ̂ / √n. For a
clearly elliptical group each axis uses its own SD: SD-H / √n and
SD-V / √n, with n − 1 degrees of freedom. The impact (centre − aim) has
the same ranges moved by the aim point. With 5 shots t₀.₉₇₅(8) = 2.31, so
the centre is known to about ± σ̂ either way; this is what a zero check
will lean on.

## 6. CEP, R90 and R95

The radius round the centre that holds a share p of the shots:

- **Round group:** R_p = σ̂ √(−2 ln(1 − p)): CEP (p = 0.5) = 1.1774 σ̂,
  R90 = 2.1460 σ̂, R95 = 2.4477 σ̂.
- **Clearly elliptical group:** Grubbs's approximation (Grubbs 1964,
  after Patnaik 1949). The squared distance from the centre is taken as a
  scaled χ² with the same mean and variance as the ellipse gives: with
  the sample variances sx², sy² and covariance sxy,
  m = sx² + sy², v = 2(sx⁴ + sy⁴ + 2 sxy²), and
  R_p = √((v / 2m) · χ²_p(2m² / v)), with χ² of a fractional number of
  degrees of freedom. It is exact for a round group and for shots on a
  line, and within a few percent between. It depends only on the
  ellipse's axes, not on which way it points. The card will say when it
  is used.

### When is a group "clearly elliptical"?

The test is on the principal axes, so it catches a group strung along a
diagonal as surely as one strung up and down. With ν = n − 1 and λ₁ ≥ λ₂
the variances along the axes, the statistic

  r² = ((λ₁ − λ₂) / (λ₁ + λ₂))² = ((Sxx − Syy)² + 4Sxy²) / (Sxx + Syy)²

of a round group follows Beta(1, (ν − 1)/2), so it exceeds
1 − 0.05^(2/(ν − 1)) only 5% of the time. A group counts as elliptical
when it has at least **5 shots**, r² is beyond that, and the ratio of the
SDs along the axes, √(λ₁/λ₂), is at least **1.5**, so a small, harmless
difference never switches the method. As a limit on the SD ratio:

| Shots | 5 | 10 | 20 | 30 | 40 or more |
|---|---|---|---|---|---|
| SD ratio limit | 5.24 | 2.51 | 1.81 | 1.60 | 1.50 |

A test checks by simulation that 5.0% of round groups of 5, 10 and 20
shots are called elliptical. Five shots can only tell a long thin string;
a 3 : 1 group usually needs 10 shots or more to show.

At a ratio of 1.5 the two methods differ by about 3% (CEP 1.46 against 1.50
for SDs of 1.5 and 1 along the axes); at 3 by about 12%.

## 7. Hit chance

The chance that a shot lands in a chosen circle or rectangle, under the
spread the group shows: round with σ̂, or the group's own ellipse (SD-H,
SD-V and correlation) when it is clearly elliptical (`ShotModel`).

- **A circle round the group's centre,** round spread:
  P = 1 − exp(−r² / 2σ̂²), exactly.
- **A circle anywhere else,** such as one on the aim point, which counts
  the group's offset in, and **any circle for an elliptical spread:** the
  bivariate normal integrated numerically. Across the axis with the larger
  spread it is adaptive Simpson's rule over ±9 standard deviations (with
  x = c + r sin θ, so the circle's edge needs no special care); along the
  other axis the conditional normal is integrated in closed form. Good to
  about 10⁻⁹.
- **An axis-aligned rectangle:** a product of two normal CDFs when across
  and up are uncorrelated, otherwise the same numerical integral.

Caveat: the chance is for the spread *measured*; with few shots the true
spread may be quite different (section 5).

## 8. Flier check

A **suggestion** only, for groups of **5 shots or more**. Nothing is ever
left out unless the user says so; the app will label the view without the
shot.

For each shot, take the centre and spread of the *other* shots (leave one
out) and how far out the shot lies from them. The shot that lies furthest
out is suggested if **both**

1. it lies beyond the **R99 of the rest**: where a further shot from
   their spread would land inside 99% of the time; and
2. leaving it out **cuts σ̂ by at least 25%**.

The R99 of the rest allows for the rest being only n − 1 shots: their
centre and spread are estimates too.

**In a round group** (the usual case) the shot's distance d from the
rest's centre is measured in their σ̂. Under the model,
d² / (2 (1 + 1/(n − 1)) σ̂²_rest) follows F(2, 2n − 4) exactly, so

  R99 = σ̂_rest √(2 (1 + 1/(n − 1)) F₀.₉₉(2, 2n − 4)).

For a 5-shot group that is 5.2 σ̂ of the other four, for 10 shots 3.7 σ̂,
for 20 shots 3.3 σ̂, tending to 3.03 σ̂ (the round spread's own R99) for
many shots. Using 3.03 σ̂ for small groups, as if the rest's σ̂ were
exact, would suggest a flier in about 4 of every 10 perfectly honest
5-shot groups.

**In a clearly elliptical group** (section 6) a shot along the string is
expected further out than one across it, so the shot is measured against
the rest's own ellipse instead: its Mahalanobis distance d S⁻¹ d, with S
the rest's covariance. For m = n − 1 other shots, Hotelling's
T² = (m / (m + 1)) d S⁻¹ d, and T² (m − 2) / (2(m − 1)) follows
F(2, m − 2) exactly for any normal spread, round or not. R99 is then the
99% ellipse of the rest, and `rest_radius_mm` its reach in the shot's
direction. A shot whose others lie exactly on one line has no ellipse to
be judged by, and is not.

The second rule is also a limit on the round test's statistic
(F ≥ 0.778 n + 0.222). Each rule's chance of passing for one honest shot
is exact; for the group it is at most n times that, since any of the n
shots could be the one. In groups that truly come from one round spread, a
flier is suggested at most

| Shots | 5 | 6 | 7 | 8 | 9 | 10 | 15 | 20 |
|---|---|---|---|---|---|---|---|---|
| Rule that decides | R99 | R99 | R99 | R99 | 25% | 25% | 25% | 25% |
| Honest groups flagged, at most | 5% | 6% | 7% | 8% | 6% | 4% | 0.3% | 0.02% |

Honest **elongated** groups are flagged a little more often, because five
to ten shots often can't show that they are elongated, and the round test
then reads the string's ends as fliers. Simulated with 20,000 groups each:

| Shots | round | 2 : 1 | 3 : 1 | 10 : 1 | on a line |
|---|---|---|---|---|---|
| 5 | 4.3% | 6.1% | 7.7% | 3.6% | 2.2% |
| 8 | 7.1% | 9.7% | 7.8% | 3.5% | 3.5% |
| 10 | 3.3% | 5.3% | 4.8% | 3.9% | 4.0% |

With the round test alone, 3 : 1 strings of 5 shots were flagged 13% of
the time and lines 23%.

Caveats:

- In a group of 5 to 8 shots, one far shot can itself make the group look
  elliptical, and the elliptical test needs a shot much further out: a
  shot 10 σ out of a round 5-shot group is suggested about 6 times in 10
  (the round test alone: always). From 10 shots on, it costs little.
- With many shots a single shot has to be very far out to cut σ̂ by a
  quarter, so the check mostly speaks up in small groups.
- It can only ever point at one shot. A shot the shooter called as a
  pulled shot is better marked as such.

## 9. Velocity

For chronograph readings vᵢ (m/s): the average, the ES (fastest −
slowest) and the sample SD s (dividing by n − 1). Its 95% range is from
χ² with n − 1 degrees of freedom:

  SD from s √((n − 1) / χ²₀.₉₇₅(n − 1)) to s √((n − 1) / χ²₀.₀₂₅(n − 1)).

For 5 readings that is 0.60 to 2.87 × s; for 10, 0.69 to 1.83 × s. A
velocity SD from five shots says little.

Readings outside **10 to 2,500 m/s** are refused, the same bounds as the
chronograph import: no chronograph reads them, so they are misreads or
typing slips ("2850" for 285.0). They are refused, never guessed at.

Readings inside those bounds are accepted, but some are worth a second
look, and each one is listed with a warning:

- outside **100 to 1,500 m/s**: possible (an air rifle, a fast varmint
  load), but unusual;
- **10% or more from the median** of three readings or more: feet per
  second typed as metres per second ("2450" for a 747 m/s load), or a
  slipped decimal point ("79.0" for 790). A real string of shots varies by
  a few percent at most.

## 10. Velocity against vertical

Does a faster shot land higher? A straight line is fitted by least squares
to each shot's height h (mm from the group's centre, up positive) against
its velocity v, over the shots that have both:

  h = a + b v, b = Σ(v − v̄)(h − h̄) / Σ(v − v̄)², a = h̄ − b v̄.

- **Slope b,** mm higher per m/s faster, with a 95% range
  b ± t₀.₉₇₅(n − 2) · SE, where SE² = (residual sum of squares / (n − 2))
  / Σ(v − v̄)² (Student's t).
- **Intercept a** is the line's height at 0 m/s; the line is better read
  through (v̄, h̄).
- **R²** is the share of the shots' vertical spread that velocity
  explains, 0 to 1.
- It needs **5 shots** with both a place and a velocity, and the
  velocities must not all be the same.

The app will show b beside the slope the ballistic solver expects at that
distance and muzzle velocity: a b well below the expected one suggests the
barrel's timing compensates (positive compensation). With 5 to 10 shots
the slope's range is wide; a large R² from 5 shots can be chance, so read
the range before the R².

## 11. Pooling groups: scatters

Every spread estimate is made from a `Scatter`: the sums Σ(x − cx)²,
Σ(y − cy)², Σ(x − cx)(y − cy) of the shots about their centres, with the
number of shots and of centres fitted from them. Scatters add, so groups
pool by adding theirs, each about its own centre: σ̂² is then the pooled
sum ÷ 2(N − k) for N shots in k groups, on 2(N − k) degrees of freedom.
A `Sigma` carries σ̂ with its degrees of freedom, which is what an F test
between two groups needs: σ̂²_A / σ̂²_B follows F(ν_A, ν_B) when both
come from the same spread.

Groups shot at different distances should pool as angles, not as
millimetres: `Scatter::scaled(1 / distance_m)` turns a group's sums into
milliradians first. `GroupStats::with_scatter` measures shots with a
pooled scatter; it refuses a scatter that doesn't count the same shots,
but keeping the units and centres consistent is up to the caller, and its
ES range and centre ranges still assume one group of that many shots.

## 12. Comparing two groups

Is load A really tighter than load B, or is it the luck of a few shots?
`compare` takes the two groups' σ̂ with their degrees of freedom ν (a
`Sigma`: 2(n − 1) for a group of n shots, more for a composite);
`compare_groups` takes two measured groups, as angles when both have a
distance (so groups shot at different distances compare fairly) and in
millimetres when neither has.

- **The ratio** R = σ̂_A / σ̂_B. Under the model, R² ÷ (σ_A / σ_B)²
  follows F(ν_A, ν_B) exactly (Snedecor and Cochran 1989), so the true
  ratio lies, with 95% confidence, between

  R / √F₀.₉₇₅(ν_A, ν_B) and R / √F₀.₀₂₅(ν_A, ν_B).

  Two 5-shot groups (ν = 8 each): F₀.₉₇₅(8, 8) = 4.43, so a measured
  ratio of 2 could be anything from 0.95 to 4.2.
- **The verdict:**
  - **Same** when the whole range lies within 1/1.1 to 1.1 (0.91 to
    1.1, so swapping the groups can't change the verdict): any difference
    is under 10%. Even two groups that measure exactly alike need 214
    shots each for it, so expect to see it seldom. It wins over
    "Different": a difference known to be under 10% is the same for a
    shooter.
  - **Different** when the range leaves out 1. When both groups truly
    come from one spread this happens 5% of the time, exactly (a
    simulation of 40,000 pairs each: 3 v 3 shots 5.01%, 5 v 5 5.10%,
    10 v 10 4.82%, 3 v 10 4.83%, 5 v 20 4.88%).
  - **Can't tell** otherwise: the usual answer with 5-shot groups.
- **Shots needed:** how many shots each group would need, if the measured
  ratio were the true one, to show as different 80% of the time (at 5%,
  two-sided), both groups the same size. The chance is exact under the
  model: with ν = 2(n − 1), the test says different when R² is beyond
  F₀.₀₂₅(ν, ν) or F₀.₉₇₅(ν, ν), and R² is ρ² times an F(ν, ν), so

  power = 1 − F_cdf(F₀.₉₇₅ / ρ²) + F_cdf(F₀.₀₂₅ / ρ²),

  and the smallest n with power ≥ 0.8 is searched for. None is given for
  a ratio within 1/1.1 to 1.1: too small a difference to chase.

| True ratio of σ | 1.15 | 1.2 | 1.3 | 1.5 | 2 | 3 |
|---|---|---|---|---|---|---|
| Shots per group | 203 | 120 | 59 | 26 | 10 | 5 |
| Power at that size, formula | 0.801 | 0.801 | 0.803 | 0.810 | 0.816 | 0.832 |
| Power simulated | 0.806 | 0.798 | 0.801 | 0.808 | 0.822 | 0.831 |

(40,000 simulated pairs each, 4,000 for 203 and 120 shots.) So "5 shots
can't tell these apart; about 26 each could" is what a ratio of 1.5
says.

### Ranking several groups

`rank` puts groups in order by **mean radius** (the default: it uses
every shot) or by **extreme spread**, as angles (mm ÷ distance), smallest
first, with each group's size and how much larger than the best it is, in
percent. A group that can't be measured (no shots, a single shot, a value
that is not a number, a distance that is not positive, or every shot in
one hole) is listed apart with the reason, not ranked.

`rank` and the composite (section 13) take groups as shots with a
distance, which they always have, because they set sizes from different
groups side by side; `compare_groups` takes measured groups, whose distance
is optional, because a ratio only needs both sizes in one unit: angles
when both have a distance, millimetres when neither has, and a refusal
when only one has.

The ranking is of the sizes as measured; it says nothing about whether the
differences are real (compare two groups for that). Groups of different
sizes flatter the small ones: a 3-shot mean radius reads 0.82 of the true
one and a 10-shot one 0.95 (section 4), and the extreme spread grows much
faster with the shots (section 5). The ranking says when the shots differ
(`unequal_shots`).

## 13. Composites: several groups pooled

A composite pools several groups (`Composite`), each given as its shots
in millimetres, its aim point if any, and its distance. Every shot is
first turned into an angle, **mrad = mm ÷ distance in m**, so groups shot
at different distances never mix millimetres; the figures are in mrad,
and `mm_at` gives them in millimetres at any chosen distance. Every
figure is worked out from the pooled shots, never by averaging the
groups' own figures, and each point keeps the index of its group (for
colouring it on screen).

Each group is first moved onto a common origin:

| Mode | When | Each shot becomes | Centre of the figures | σ̂'s degrees of freedom |
|---|---|---|---|---|
| About aims | every group has an aim point | shot − its aim: its impact | the composite's own centre, the mean impact | 2(N − 1) for N shots |
| About own centres | any group lacks an aim point (or chosen) | (shot − its group's centre) × √(nᵢ / (nᵢ − 1)) | each shot's own group centre | 2Σ(nᵢ − 1) |
| Cold bore | every group is one shot with an aim point | shot − its aim | the k impacts' centre | 2(k − 1) |

- **About aims** answers "where do my shots go, all told": the shifts of
  impact between groups (from day to day, after remounting) are part of
  the spread. σ̂ and its range treat the N impacts as one group of N
  shots, which they are only if the groups shared one impact. When they
  didn't, σ̂ is still a fair measure of the total spread, but its range
  is too narrow, because the shots of one group move together:
  simulated with a group-to-group SD τ equal to the shots' σ, the range
  covered the true total σ 82% of the time for 3 groups × 5 shots, 89%
  for 5 × 3 and 67% for 4 × 10 (95% with τ = 0). The composite also gives
  σ̂ within the groups for comparison; if σ̂ is well above it, the impact
  moved between groups, and the zero check (section 14) tells by how
  much.
- **About own centres** answers "how precise are the rifle and load":
  impact is gone, and σ̂ pools the scatter within the groups exactly as
  the sums of squares add (section 11). Its range is exact under the
  model whatever the impacts did (simulated: 94.7% to 95.1% coverage for
  3 × 5, 5 × 3 and 4 × 10 with τ = 0, σ and 2σ). A shot's distance from
  its own group's centre is shorter than from the true centre, by a
  factor √(1 − 1/n) on average (0.82 for 3 shots): the centre was fitted
  to it. So each residual is scaled up by √(n/(n − 1)) before the sizes
  are measured: the mean and median radius, extreme spread, width and
  height then read as for shots about a known centre (simulated: the mean
  radius of 20 groups of 3 averages the true one to within 2%; unscaled it was
  0.82 and below its own range every time). σ̂ and its sums are of the
  shots as shot, unscaled. **Single-shot groups are left out** and listed:
  a single shot is its own centre and shows no spread.
- **Cold bore**: the first shot from a cold, clean barrel, one per
  session. Each is its own group, so the k impacts are independent shots
  from the cold-bore spread (the day-to-day shifts are part of it), with
  k − 1 degrees of freedom per axis.

Mean radius, median radius, CEP, R90 and R95 are about the centre in the
table. **The extreme spread** is the two points furthest apart as the
composite is drawn: an honest figure for the pooled points. Its range
(section 5) is given only for a cold-bore series (up to 50 shots), the
one composite whose points are independent shots from one spread. About
own centres the scaled points of a group are tied to each other (their
residuals sum to zero) and spread a little more than as many shots from
one spread (simulated means: 4.35σ for 3 × 5 and 4.36σ for 5 × 3 against
4.20σ for one group of 15; 5.08σ against 5.02σ for 4 × 10); about aims,
shifts between groups cluster the points and they spread less (with
τ = σ, 3.79σ and 4.00σ in total σ). Neither matches the one-group table,
so neither gets a range.

## 14. Zero check

Is the zero holding, or has it moved? `ZeroCheck` takes groups shot since
the last zero, each with its impact (centre − aim, mrad), number of
shots, σ̂ (mrad) and whatever was dialled away from the zero for it,
which is subtracted first (`ZeroGroup::from_stats` makes one from a
measured group). It gives back each group's net impact, for plotting. On
each axis separately:

- **The mean impact** is per shot: ȳ = Σ nᵢ yᵢ / N, so a 10-shot group
  counts twice a 5-shot one.
- **Its standard error counts the shifts from group to group** (one-way
  random-effects model: group i's true impact is the zero's plus a shift
  of SD τ). The method of moments (Searle, Casella and McCulloch 1992,
  chapter 3) gives

  τ̂² = max(0, (SSB − (k − 1) σ̂²) ÷ (N − Σnᵢ²/N)), SSB = Σ nᵢ (yᵢ − ȳ)²,

  with σ̂² pooled within the groups, and Var(ȳ) = τ̂² Σwᵢ² + σ̂²/N with
  wᵢ = nᵢ/N. For groups of the same size this is exactly s²/k, the
  scatter of the group impacts, whenever τ̂² > 0.
- **The 95% range** is ȳ ± t₀.₉₇₅ √Var(ȳ). When τ̂² > 0 (the group
  impacts scatter more than their shots explain), Var(ȳ) is a MSB + b σ̂²
  with MSB = SSB/(k − 1), a = Σwᵢ²(k − 1)/(N − Σnᵢ²/N) and b = 1/N − a,
  and t has Satterthwaite's (1946) degrees of freedom,
  Var(ȳ)² ÷ ((a MSB)²/(k − 1) + (b σ̂²)²/ν_σ), kept between 1 and k − 1:
  k − 1 for groups of the same size, fewer when one group holds most of
  the shots. Otherwise t has σ̂'s 2Σ(nᵢ − 1). Single-shot groups
  (cold-bore shots) have no σ̂ and use the impacts' own scatter, s²/k on
  k − 1.
- **With one or two groups** the shifts can't be measured: the range
  comes from the shots alone, σ̂/√N, and says so
  (`counts_group_to_group` is false). It is too narrow if the impact does
  move between groups: simulated with 2 groups × 5 shots and τ = σ, it
  covered the true zero only 60% of the time. So **one or two groups give
  no verdict**, only "can't tell" and how many more groups would settle it.
- **No spread** (every shot in one hole, or identical cold-bore impacts)
  gives no range, as in the comparison of two groups.
- **The correction** to dial is minus the mean impact.

The verdict, from three groups or more, with a tolerance of **0.1 mrad**
either way unless another is chosen (0.1 mrad is 1 cm at 100 m, 0.34 MOA):

- **Holding:** the range lies within ± the tolerance on both axes.
- **Drifting:** on an axis the range leaves out 0 and the mean impact is
  beyond the tolerance.
- **Can't tell:** neither, with how many more groups like these (the same
  shots per group, scatter and mean impact) would settle it, as a rough
  guide.

How well this works, simulated with 20,000 checks each (σ = 1 per shot;
groups × shots; coverage per axis; "drifting" is how often a zero that
holds on average is called drifting, on either axis; the effective number
of groups is N²/Σnᵢ²):

| Groups | Effective | τ (in σ) | Range covers | Drifting, tolerance σ | Tolerance σ/2 |
|---|---|---|---|---|---|
| 2 × 5 | 2.0 | 1 | 60.4% | none (no verdict) | none |
| 3 × 5 | 3.0 | 0 | 97.3% | 0.0% | 4.6% |
| 3 × 5 | 3.0 | 0.5 | 93.9% | 0.7% | 11.0% |
| 3 × 5 | 3.0 | 1 | 93.4% | 4.8% | 12.2% |
| 3 × 5 | 3.0 | 2 | 94.4% | 8.5% | 10.7% |
| 6 × 3 | 6.0 | 0 | 97.5% | 0.0% | 3.5% |
| 6 × 3 | 6.0 | 1 | 95.0% | 3.4% | 9.3% |
| 10 × 5 | 10.0 | 0.5 | 95.1% | 0.0% | 2.9% |
| 10, 3, 5, 3 shots | 3.1 | 0 | 97.3% | 0.0% | 2.5% |
| 10, 3, 5, 3 shots | 3.1 | 1 | 95.7% | 2.5% | 7.3% |
| 10, 5, 3, 3, 3 shots | 3.8 | 0.5 | 95.2% | 0.2% | 6.2% |
| 10, 5, 3, 3, 3 shots | 3.8 | 1 | 96.2% | 3.1% | 6.7% |
| 20, 3, 3 shots | 1.6 | 0 | 97.2% | 0.0% | 1.2% |
| 20, 3, 3 shots | 1.6 | 0.5 | 88.7% | 0.9% | 14.4% |
| 20, 3, 3 shots | 1.6 | 1 | 91.0% | 4.1% | 13.5% |
| 20, 3, 3 shots | 1.6 | 2 | 96.2% | 4.1% | 7.1% |
| 5 × 1 (cold bore) | 5.0 | 1 | 94.9% | 6.5% | 9.5% |

A tolerance of σ is a 1-MOA rifle (σ ≈ 0.1 mrad) at the default.

- **The range** covers close to 95% with three groups or more, a little
  more when the impact doesn't move between groups (the range then rests
  on the shots, whose estimate is good), **except when one group holds
  most of the shots** (20, 3 and 3: effectively 1.6 groups). Then the
  mean is mostly that group's impact, and only the two small groups can
  show how far impacts move; when they move by about half the shots' σ
  the small groups usually can't show it, and the range covers 89–91%.
  `effective_groups` says when this applies: well below the number of
  groups.
- **False "drifting"** at a realistic tolerance is rare: 0–0.9% when the
  impact holds still from group to group or moves by σ/2. When it moves
  by σ to 2σ from group to group, 2.5–8.5% of checks call it drifting:
  the zero then really does wander, even if it is right on average. With
  a tolerance below the shots' own scatter (σ/2 here) the range alone
  decides, and since either axis can call it, false "drifting" runs to
  about twice what each axis leaves out (up to 14% in the cases above).
  Choose a tolerance no tighter than the rifle can show.

## 15. Load ladders

A ladder (`Ladder`) shoots groups at a series of charges (g) or seating
depths (mm) and looks for a stretch where the impact holds still. Each
group gives its rung value, impact (mrad), shots, σ̂ and, if measured, its
chronograph readings (`LadderGroup::from_stats` makes one from a measured
group).

- **Rungs:** groups at the same value (within **0.001** g or mm of the
  rung's first value, below any scale or calliper step; configurable, 0
  for equal values only) pool into one rung: its per-shot mean impact up
  and across, its σ̂ pooled within its groups, and its velocity mean and
  SD (all readings about their mean).
- **Standard errors:** each rung's is σ̂/√N on each axis, where σ̂ is
  pooled within **every group of the ladder**, not the rung's own. A
  rung's 3 to 5 shots give a σ̂ that may be half or twice the truth;
  with each rung's own, runs of loose rungs would score flatter than
  runs of tight ones. A σ̂ known from other groups (`known_sigma`, with
  its degrees of freedom) is pooled with the ladder's own by degrees of
  freedom, σ̂² = (ν₁σ̂₁² + ν₂σ̂₂²)/(ν₁ + ν₂); a ladder of single shots can't
  measure its own noise and needs one.
- **Runs:** every run of **3 or more** neighbouring rungs is scored by
  (highest − lowest vertical impact) ÷ noise, the noise being the RMS of
  its rungs' vertical standard errors; likewise across.
- **Flat:** a run whose score is within the **2/3 quantile of the range
  of as many independent standard normals**: P(W ≤ w) = m ∫ φ(x)
  (Φ(x + w) − Φ(x))^(m − 1) dx for m rungs, worked out by Simpson's rule
  and Newton's method (2.00 for 3 rungs, 2.38 for 4, 2.65 for 5, 2.85 for
  6; a simulation of 400,000 sets each agrees to 0.003). So a run whose
  impact truly holds still passes two times in three whatever its length.
  The same limit judges flat across.
- **Velocity:** each run's change in m/s per unit of the rungs' value
  (per g of charge, or per mm of seating depth), by least squares over
  every reading in the run, with its 95% range from t with readings − 2
  degrees of freedom, from three readings on.
- **Ranking:** by default vertical, then horizontal, then σ̂, each
  deciding only between runs the one before can't tell apart: flat runs
  first, and among those the runs also flat across first, ordered by σ̂;
  then flat runs that aren't flat across, by horizontal score; then the
  rest by vertical score. A key can rank by vertical score, horizontal
  score or σ̂ alone. The best run of each length is given.

**What "flat" catches.** Simulated with 10,000 runs each, with σ̂
estimated from the shots (which makes the noise itself a little
uncertain), the share of runs called flat:

| Rungs (3 shots each) | Limit | Truly flat | 0.5 noise per step | 1 noise per step | 2 noise per step |
|---|---|---|---|---|---|
| 3 | 2.00 | 63% | 55% | 37% | 6% |
| 4 | 2.38 | 63% | 48% | 19% | 0% |
| 5 | 2.65 | 64% | 39% | 7% | 0% |
| 6 | 2.85 | 63% | 29% | 2% | 0% |

With 5-shot rungs truly flat runs pass 65% at every length and the climbs
are caught as often to within 1%. A truly flat run passes about two times
in three at any length (a little less than 2/3 because σ̂ is estimated),
and a climb of 2 noise per step is almost never called flat; but 3 rungs
can't tell "flat" from "climbing by its own noise per step" (37%), and
only longer runs can.

**Most flat spots are noise.** In a simulated 8-rung ladder with no node
at all, the impact climbing steadily, a run of 3 rungs was called flat
somewhere in the ladder 97% of the time with a climb of 0.5 noise per
step, 94% with 1, 72% with 1.5 and 31% with 2, whether the rungs were 3
or 5 shots. With 3-shot rungs of a 1-MOA rifle (σ ≈ 0.1 mrad) the noise
is 0.06 mrad, so a ladder climbing 0.06 mrad per rung shows a "flat
spot" almost every time. Read the error bars; shoot the promising run
again, with more shots, before believing it.

## 16. Powder temperature sensitivity

`TemperatureSensitivity` fits how the mean velocity of a load changes with
the powder's temperature, from sessions each given as (temperature °C,
mean velocity, shots, SD). It needs **at least 3 sessions spanning at
least 10 °C**, and says which is missing otherwise.

- **The fit** is weighted least squares of the sessions' mean velocities
  on temperature, each weighted by its shots (as if every shot had been
  fitted, with one SD for all; a session's own SD from a few shots is too
  uncertain to weight by): b = Σnᵢ(Tᵢ − T̄)(vᵢ − v̄) / Σnᵢ(Tᵢ − T̄)², with
  T̄ and v̄ the shot-weighted means (Draper and Smith 1998).
- **The slope b is in m/s per °C,** the unit MXSC stores a bullet's and a
  load's sensitivity in (`velocity_per_degree`, which the calculator
  accepts within ±5 m/s per °C). It is also given as a percentage of the
  line's velocity at 15 °C per °C; ×15 gives the percent per 15 °C some
  apps (Strelok) use.
- **The 95% range** is b ± t₀.₉₇₅ · SE from the larger of two standard
  errors, with that one's degrees of freedom, as in the zero check: from
  the sessions' scatter about the line, √(Σnᵢrᵢ² / (k − 2) / Σnᵢ(Tᵢ − T̄)²)
  on k − 2, which counts whatever else changed between sessions (another
  lot, the chronograph's placement, a fouled barrel); or from the shots
  alone, s_p / √Σnᵢ(Tᵢ − T̄)² with the pooled SD s_p, on Σ(nᵢ − 1).

Simulated with 20,000 fits each, shot SD 4 m/s, the range covered the
true slope:

| Sessions × shots | Between-session SD 0 | 1 m/s | 2 m/s | 4 m/s |
|---|---|---|---|---|
| 3 × 5 (0, 10, 20 °C) | 97.1% | 95.8% | 92.3% | 88.4% |
| 3 × 10 | 96.7% | 93.5% | 88.7% | 87.6% |
| 5 × 10 (−5 to 35 °C) | 97.4% | 95.5% | 94.5% | 94.6% |

With 3 sessions the scatter about the line rests on 1 degree of freedom
and often hides real differences between sessions, so the range can be
too narrow (88% instead of 95%) when the sessions differ by more than
their shots; 5 sessions do not have that problem. Spread the sessions
over as wide a span as you can: the slope's error falls with the span.

## 17. Box plots for trends

`BoxPlot` gives the five numbers of a box plot of any values (σ̂ of each
session, say): min, first quartile, median, third quartile, max. The
quartiles are Hyndman and Fan's **type 7**, the default of R, NumPy and
Excel's QUARTILE.INC: the value at position p(n − 1) of the sorted
values, counted from 0, interpolated between neighbours. For 1, 2, 4, 7,
11 and 16 that is 2.5, 5.5 and 10.

## 18. How the distributions are worked out

The app has no outside libraries for these; they are written in the crate
(`distributions`) and tested against closed forms (to 10 significant
digits or better) and published tables (the NIST/SEMATECH e-Handbook of
Statistical Methods, section 1.3.6.7, to every digit they print). Shapes
beyond about 10¹¹, which no group comes near, give no answer (NaN) rather
than a wrong one.

| Function | Method |
|---|---|
| ln Γ(x) | Lanczos's approximation, g = 7, nine terms (Lanczos 1964), with the reflection formula below ½ |
| Regularised incomplete gamma P(a, x), Q(a, x) | power series below x = a + 1, continued fraction (modified Lentz) above (Press et al. 2007) |
| Regularised incomplete beta Iₓ(a, b) | continued fraction (modified Lentz) on the side where it converges fast (Press et al. 2007) |
| χ² CDF | P(k/2, x/2) |
| F CDF | I_{d₁x/(d₁x + d₂)}(d₁/2, d₂/2) |
| Student's t CDF | through Iₓ(ν/2, ½) in the tails and I(½, ν/2) near 0 |
| Normal CDF | through Q(½, z²/2), so the far tails keep their precision |
| Normal quantile | Acklam's rational approximation, refined by Halley's method |
| χ², F, t quantiles | Newton's method kept inside a shrinking bracket, started from Wilson and Hilferty's approximation (χ²) or the normal quantile (t); above the median the upper tail is matched instead, so quantiles near 1 keep their precision |

## References

- Draper, N. R. and Smith, H. (1998). *Applied Regression Analysis*, 3rd
  ed. (weighted least squares). New York: Wiley.
- Grubbs, F. E. (1964). Approximate circular and noncircular offset
  probabilities of hitting. *Operations Research* 12(1), 51–62.
- Grubbs, F. E. (1964). *Statistical Measures of Accuracy for Riflemen and
  Missile Engineers.* Ann Arbor: Edwards Brothers.
- Patnaik, P. B. (1949). The non-central χ²- and F-distributions and their
  applications. *Biometrika* 36, 202–232.
- Hyndman, R. J. and Fan, Y. (1996). Sample quantiles in statistical
  packages. *The American Statistician* 50(4), 361–365.
- Hartley, R. I. (1997). In defense of the eight-point algorithm. *IEEE
  Transactions on Pattern Analysis and Machine Intelligence* 19(6),
  580–593.
- Hartley, R. and Zisserman, A. (2004). *Multiple View Geometry in Computer
  Vision*, 2nd ed., chapter 4. Cambridge University Press.
- Lanczos, C. (1964). A precision approximation of the gamma function.
  *SIAM Journal on Numerical Analysis, Series B* 1, 86–96.
- Satterthwaite, F. E. (1946). An approximate distribution of estimates
  of variance components. *Biometrics Bulletin* 2(6), 110–114.
- Searle, S. R., Casella, G. and McCulloch, C. E. (1992). *Variance
  Components*, chapter 3 (the one-way classification). New York: Wiley.
- Snedecor, G. W. and Cochran, W. G. (1989). *Statistical Methods*, 8th
  ed. (the F test of two variances). Ames: Iowa State University Press.
- Press, W. H., Teukolsky, S. A., Vetterling, W. T. and Flannery, B. P.
  (2007). *Numerical Recipes*, 3rd ed., sections 6.1–6.4. Cambridge
  University Press.
- Acklam, P. J. (2003). An algorithm for computing the inverse normal
  cumulative distribution function.
- Wilson, E. B. and Hilferty, M. M. (1931). The distribution of
  chi-square. *Proceedings of the National Academy of Sciences* 17,
  684–688.
- NIST/SEMATECH e-Handbook of Statistical Methods, section 1.3.6.7,
  tables of critical values.
