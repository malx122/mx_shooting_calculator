# Core software reliability review — 2026-10-25

Initially reviewed revision `6ed03b4`. This review covers malformed-input handling
and automated-test results. It does **not** establish ballistic accuracy or include
a completed comparison against other calculators. The findings below describe the
original behavior; all four were subsequently fixed on the user's instruction.

## Findings

### CORE-01: Non-finite environmental inputs can return success

Locations: `crates/ballistics-core/src/solver.rs:491` and `:574`.

The finite-number check omits both atmospheres' temperature, pressure and humidity,
and optional Coriolis coordinates. The later atmospheric check uses comparisons
that do not reject NaN. Direct Rust API reproductions, using the existing valid
test fixture with just one field changed, found:

| Changed input | Observed result from `solve` |
| --- | --- |
| Shot temperature = NaN | `Ok`, one sample containing non-finite fields |
| Shot pressure = NaN | `Ok`, one sample containing non-finite fields |
| Shot humidity = NaN | `Ok`, finite sample fields; invalid input silently accepted |
| Coriolis latitude = NaN | `Ok`, only the initial sample |

This weakens the distinction between a valid result and invalid input. These are
confirmed API-level findings; no normal UI route supplying NaN was established.
UI weather controls have numeric ranges, and JSON does not directly represent
NaN. Those constraints do not cover arbitrary callers of the public Rust API.

### CORE-02: Lookup table accepts infinity and can panic on NaN

Location: `crates/ballistics-core/src/solver.rs:88` and `:115`.

`BcBands::new` checks positivity but accepts positive infinity for either member
of a point. Independently, `BcBands::at(f64::NAN)` panics on a nonempty table:
both endpoint comparisons fail, `partition_point` returns zero, and the code
indexes `upper - 1`.

Confirmed with a two-point finite table and `catch_unwind`. Separately confirmed
that constructing a one-point table with either an infinite key or infinite
value returns `Ok`.

The store's `Bullet::validate` uses bounded ranges for these stored points and
rejects infinities, so the constructor issue is narrower than a demonstrated
normal catalog or editor failure. Direct core callers can bypass the store.

### CORE-03: Additional public API arguments bypass validation

Location: `crates/ballistics-core/src/solver.rs:372`.

`solve_with_zero` validates the scenario but does not validate its separate
`zero` tuple. Passing `(NaN, 0.0)` with an otherwise valid fixture returns `Ok`
with one sample containing non-finite fields.

This is a public API contract issue. The review did not establish an ordinary
application path generating this argument.

### CORE-04: Invalid maximum range is accepted

Location: `crates/ballistics-core/src/solver.rs:362` and `:372`.

Neither entry point checks its separate `max_range` argument. Calling `solve`
with NaN or a negative value returns `Ok` with two samples in the reproduction,
rather than an invalid-input error. This also means a successful return alone
does not establish that the request was meaningful.

## Verification and limits

The fixes reject non-finite environmental inputs and explicit zero angles with
`SolveError::InvalidInput`. Both solver entry points reject non-finite or negative
maximum ranges; zero remains valid. `BcBands::new` rejects non-finite points, and
`BcBands::at` returns `None` for non-finite queries, avoiding the NaN panic.

Five regression tests were added and first run against the original code: all
five failed, including the lookup panic. After the fixes,
`cargo test -p ballistics-core -p ballistics-shot` passed: 43 core tests and
6 adapter tests. The new tests cover NaN, positive and negative infinity,
both solver entry points, empty/single/multiple-point lookup tables, and the
zero-range boundary. The pre-existing independent-reference test also passed;
it was not extended or independently regenerated in this review.

The full workspace also passed: 205 tests successful and four ignored.
`cargo fmt --all`, desktop Clippy (`--workspace --all-targets -- -D warnings`)
and browser lint (`scripts/web.sh lint`) completed successfully.

The malformed-input probes were compiled against the unmodified core with
`rustc`, using a temporary harness and the existing `scenario()` test fixture.
They inspected return status, sample count and whether fields were finite;
they did not benchmark firing corrections. The NaN lookup probe used
`catch_unwind` to confirm the panic without terminating the remaining probes.

This is a focused reliability review, not a full audit of every public helper,
import path or UI control. These four findings are resolved by the follow-up fixes.
