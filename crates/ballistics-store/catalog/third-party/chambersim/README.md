# ChamberSim projectile data collection

Credit: **ChamberSim Reloaded project database**.

Source: <https://github.com/Schildkroet/ChamberSim-Reloaded>, revision
`8e544ebcf30cb1d64caed95895e79369064358df`, imported 2026-10-26.

This adapted collection is licensed under **CC BY-SA 4.0**, separately from
the independent MIT/Apache-2.0 app code. See [ATTRIBUTION.txt](ATTRIBUTION.txt),
the upstream declaration in [UPSTREAM-LICENSE.md](UPSTREAM-LICENSE.md), and
the [license](https://creativecommons.org/licenses/by-sa/4.0/).
No ChamberSim program code is included.

## Selection

The first import contains **1,034 component records in 14 manufacturer files**.
All BCs are copied as recorded, without fitting or independent verification.
The source revision and exact source-record link are preserved in each record.

From 3,120 source records, the reproducible import excludes:

| Reason | Count |
| --- | ---: |
| Possible existing match in the comparison ledger | 411 |
| Additional possible family/weight/diameter match | 6 |
| Possible packaging duplicate within this import | 58 |
| No BC in the source | 1,443 |
| No existing catalog caliber with a matching diameter | 166 |
| Outside the existing catalog's BC bounds | 2 |

These are conservative selection decisions, not findings that excluded source
records are incorrect. Component and loaded-ammunition entries remain distinct.
Existing catalog records are not overwritten or supplemented by this import.

Weights are converted using `1 gr = 0.06479891 g`. Source lengths and their
uncertainty descriptions remain in notes; the active `length_mm` is null in
this first import. Construction and expansion limits are unknown, and no
lead-free claim is made. The source's `gValid` marker is not treated as physical
BC verification or a per-field manufacturer citation. `verified_on` is empty
because the import did not perform fresh manufacturer checks.

## Reproduction and provenance

Run from the repository root:

```sh
python3 tools/import_chambersim.py ../ChamberSim-Reloaded
```

The importer is this project's independent code. It only reads the source JSON
and license declaration. It requires the file hashes and source revision from
the reviewed comparison snapshot to match before writing anything. If those
inputs change, regenerate and review the comparison first.

[import-manifest.json](import-manifest.json) records every selection decision,
input hashes, comparison hash and caliber-file hash. IDs are derived from the
source manufacturer and product name; changing a BC alone does not change an ID.
The original comparison remains a baseline of the catalogs before this import.

`data_attribution` accompanies personal copies in SQLite, exports and sync.
Backup format 7 protects it from silent loss through older readers. Pre-existing
personal bullets have no attribution added and keep their original values.

The root NOTICE contains the distribution credit and license links and is
already included in desktop packages. The app's About window embeds the data
notice for both desktop and browser builds, including offline use. Imported
catalog entries and their personal copies display their license separately.
Catalog and personal-data listings through MCP include the attribution too.
Attribution source and license links must use HTTPS when records are saved or
imported from backups or sync.
