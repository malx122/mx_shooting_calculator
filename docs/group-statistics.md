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

## 11. For comparing and pooling groups later

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

## 12. How the distributions are worked out

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

- Grubbs, F. E. (1964). Approximate circular and noncircular offset
  probabilities of hitting. *Operations Research* 12(1), 51–62.
- Grubbs, F. E. (1964). *Statistical Measures of Accuracy for Riflemen and
  Missile Engineers.* Ann Arbor: Edwards Brothers.
- Patnaik, P. B. (1949). The non-central χ²- and F-distributions and their
  applications. *Biometrika* 36, 202–232.
- Hartley, R. I. (1997). In defense of the eight-point algorithm. *IEEE
  Transactions on Pattern Analysis and Machine Intelligence* 19(6),
  580–593.
- Hartley, R. and Zisserman, A. (2004). *Multiple View Geometry in Computer
  Vision*, 2nd ed., chapter 4. Cambridge University Press.
- Lanczos, C. (1964). A precision approximation of the gamma function.
  *SIAM Journal on Numerical Analysis, Series B* 1, 86–96.
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
