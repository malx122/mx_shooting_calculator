# ChamberSim projectile catalog comparison — 2026-10-26

This is the baseline before the subsequent licensed import. See the
[imported collection](../crates/ballistics-store/catalog/third-party/chambersim/README.md)
for the later additions and current selection decisions.

Compared the local `../ChamberSim-Reloaded/data/projectiles` files against
`crates/ballistics-store/catalog/bullets`. Our revision was `63c11ce`; ChamberSim's
was `8e544eb`. The comparison used the actual working-tree JSON files, whose
SHA-256 hashes are recorded in the [complete ledger](chambersim-projectile-comparison-2026-10-26.json).
Neither catalog was changed.

## Scope and result

| Dataset | Records | Files | Manufacturers |
| --- | ---: | ---: | ---: |
| Our entire catalog | 1,529 | 26 | 26 |
| Our component bullets | 499 | — | 16 |
| Our loaded ammunition | 1,030 | — | — |
| ChamberSim projectiles | 3,120 | 51 | 51 |

ChamberSim's records describe projectiles, so direct comparisons are against
our **component bullets**. Loaded ammunition is not the same catalog entity as
a component bullet, even when manufacturer, nominal weight and product family
are similar. For example, our 275 Federal records are ammunition, whereas
ChamberSim's 30 Federal records are components. They were not declared conflicting
copies of the same SKU.

Of our 499 component records:

- **350** have one ChamberSim record sharing an explicit manufacturer item number.
- **16** have a unique name-and-dimensions candidate without a shared item number.
- **39** have candidates based only on manufacturer, diameter and nominal mass;
  some have several possible matches. These are not established identities.
- **94** have no candidate under these rules. This does not prove absence under
  every possible alias, historical product name or dimensional discrepancy.

The 350 item-number matches have **195 G1 and 107 G7 values present in both
datasets**. All **302 shared BC fields are exactly equal**, without using the
comparison's rounding allowance. ChamberSim lacks G1 for 155 of these matches
and G7 for 68 matches where our catalog has it. It supplies no additional BC
field for these 350 item-number matches.

This is agreement between two catalogs, not independent physical validation.
Both catalogs can draw on the same manufacturer publications. Candidate matches
based only on dimensions can have different BCs because they are different
products; their differences are not counted as confirmed conflicts.

## Differences on item-number matches

The following 11 component records have at least one difference exceeding the
documented comparison allowances. A difference is a follow-up item, not a finding
that either side must be changed.

| Item | Field | Ours | ChamberSim | Context |
| --- | --- | ---: | ---: | --- |
| Barnes LRX #30862, 95 gr | Length, mm | 32.30 | 30.9 | ChamberSim explicitly estimates the length. |
| Barnes LRX #30228, 127 gr | Length, mm | 35.60 | 35.0 | ChamberSim explicitly estimates the length. |
| Lapua TRX N583 | Length, mm | 35.01 | 34.7 | ChamberSim measures a product rendering. |
| Lapua TRX N585 | Length, mm | 33.62 | 32.9 | ChamberSim measures a product rendering. |
| Lapua TRX N584 | Length, mm | 37.61 | 36.6 | ChamberSim measures a product rendering. |
| PPU B-541 | Diameter, mm | 8.2042 | 8.18 | Our note records conversion from published .323 in. |
| PPU B-498 | Mass | 16.3 g | 250 gr (16.1997 g) | Already explained in our note: the PPU table prints 16.3 g / 250 gr. We deliberately leave `mass_gr` null. |
| RWS #2145960 | Nominal mass, gr | 56 | 55 | Our stored metric mass is 3.6 g. Nominal grain rounding may explain the discrepancy. |
| Sierra #1100 | Diameter, mm | 5.664 | 5.69 | Our note records a .223 in manufacturer spec; source record says .224. |
| Sierra #1110 | Diameter, mm | 5.664 | 5.69 | Same .223 versus .224 distinction. |
| Sierra #2755 | Diameter, mm | 9.017 | 9.09 | Our note records .355 in; ChamberSim record says .358. |

The source URLs, both values, source record file/index, and ChamberSim's
descriptions are retained in the ledger. The manufacturer pages were **not**
re-fetched for this comparison. The diameter differences merit source checks
before deciding whether either catalog is wrong. The five length differences
have explicit estimate/image provenance on the ChamberSim side; this comparison
does not justify replacing the values already recorded here from published specs.

## Length coverage and provenance

Our component catalog contains 186 lengths and leaves 313 blank. Among the
350 direct item-number matches, ChamberSim has **193 lengths we leave blank**:

| ChamberSim evidence in the record | Count |
| --- | ---: |
| Explicit photo/rendering measurement | 69 |
| Explicit estimated length | 31 |
| `gValid: true`, without an explicit length method in `descr` | 65 |
| Method unspecified in the record | 28 |

The flag is a general validation marker, not evidence that a BC was physically
tested, nor a per-field manufacturer citation. The ledger labels this group
`gValid_flag_only`; it does not call those lengths measured or manufacturer-verified.
ChamberSim's [source documentation](../../ChamberSim-Reloaded/docs/DATA_SOURCES.md)
also describes estimation and image measurement methods. Of all 3,120 source
records, the record-level classification is 942 explicit estimates, 601 explicit
image measurements, 1,026 with only the validation flag, and 551 unspecified.

The missing lengths are potential enrichment data with different evidence
levels. They should retain that provenance if a later decision is made to use
them. There was no automatic filling of blank fields.

## Coverage by manufacturers represented here

An item-number match count of zero does not necessarily mean no underlying
product overlap. CopperBear, for example, has dimension-based candidates for
all 24 of our components but no shared item codes in the source names.

| Manufacturer | Our components | Our ammunition | ChamberSim components | Item-number matches |
| --- | ---: | ---: | ---: | ---: |
| Aguila | 0 | 19 | 0 | 0 |
| Barnes | 17 | 25 | 173 | 5 |
| Berger | 119 | 43 | 122 | 114 |
| CCI | 0 | 48 | 0 | 0 |
| CopperBear | 24 | 26 | 28 | 0 |
| Federal | 0 | 275 | 30 | 0 |
| Fiocchi | 0 | 46 | 3 | 0 |
| Geco | 0 | 11 | 33 | 0 |
| GGG | 3 | 12 | 0 | 0 |
| Hornady | 20 | 23 | 362 | 18 |
| Infitech | 5 | 0 | 0 | 0 |
| Lapua | 5 | 19 | 150 | 5 |
| Norma | 5 | 193 | 59 | 5 |
| Nosler | 26 | 0 | 214 | 26 |
| Prvi Partizan / PPU | 18 | 37 | 171 | 15 |
| Remington | 0 | 3 | 11 | 0 |
| RWS | 17 | 76 | 59 | 17 |
| Sako | 0 | 11 | 53 | 0 |
| Sellier & Bellot | 0 | 152 | 105 | 0 |
| Sierra | 123 | 0 | 244 | 118 |
| SK | 0 | 9 | 0 | 0 |
| Speer | 33 | 0 | 159 | 27 |
| Swift | 42 | 0 | 2 | 0 |
| Tarvas | 11 | 1 | 0 | 0 |
| Winchester | 0 | 1 | 34 | 0 |
| Woodleigh | 31 | 0 | 1 | 0 |

ChamberSim also contains **31 manufacturers absent from our catalog**, comprising
1,107 records. Examples with BC fields include Styria Arms (230 records with G1),
Hasler (52 records, 47 with G1 and three with G7), Fox (54, 33 with G1), Brenneke
(32, 31 with G1), Balistix (33, 30 with G1 and three with G7), and
Steinbockprecision (11, all with G1). The ledger inventories every manufacturer
and source record, including those without BCs.

Across ChamberSim, 1,469 records have G1, 213 have G7, and 1,645 have neither.
There are 2,770 source records without a direct item-number match here, including
1,280 with at least one BC. These are an inventory for potential future catalog
work, **not 2,770 proven missing products**: unresolved candidates, differences
in product scope, and loaded-ammunition/component distinctions remain.

Our catalog retains metadata absent as dedicated fields from these source
records, including per-record source URL/date, construction, lead-free status,
and ammunition-specific velocity/barrel information. ChamberSim has additional
base geometry and application-specific model fields that have no direct mapping
to our current catalog schema. No such fields were imported or inferred.

## Method and audit checks

Manufacturer names are compared without punctuation or case; Prvi Partizan is
aliased to PPU. Item identifiers are extracted from explicit item/part/SKU/article/
material references in our notes, product codes, and Norma article numbers in
URLs, then matched against tokens in ChamberSim product names. Package-prefix
`19-` is stripped when recognizing Speer item numbers. Suffixes such as Sierra's
`T` are not silently removed to force an item match.

For candidates without an item match, nominal mass must be within 0.51 gr and
diameter within 0.015 mm. A unique normalized product-name containment match
is marked as a name-and-dimensions candidate; otherwise candidates remain
dimensions-only. The candidate list can contain other products of the same size.

Field comparison allowances are 0.055 g, 0.51 gr, 0.015 mm diameter, 0.15 mm
length, and 0.0005 for each BC. These suppress small representation/rounding
differences; they are **not accuracy specifications**. Grains use
`1 gr = 0.06479891 g`. Where our nominal `mass_gr` is null, the ledger derives
an equivalent grain mass from `mass_g` for comparison only. Missing or nonpositive
source numerical values are treated as unavailable, consistent with ChamberSim's
[format documentation](../../ChamberSim-Reloaded/data/README.md).

Checked that the ledger covers all 499 unique local component IDs and all
3,120 unique source file/index keys, that manufacturer totals reconcile, and
that the 302 shared BC fields exactly reproduce the two source files' values.
All 77 input file hashes were checked again before writing the ledger. No
duplicate exact manufacturer/product-name pairs were found in ChamberSim.
The comparison includes every local component, but the conservative matching
rules do not establish an identity for every plausible overlap.

Attribution: **ChamberSim Reloaded project database**. Its
[data license file](../../ChamberSim-Reloaded/data/LICENSE.md) declares
CC BY-SA 4.0, separately from its program code. The accompanying comparison JSON
extracts and normalizes selected source fields; the ChamberSim-derived data is
attributed and shared under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
No source data was copied into the application catalog.
