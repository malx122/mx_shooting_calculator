#!/usr/bin/env python3
"""Import a separately licensed selection of the local ChamberSim projectile data.

No ChamberSim program code is used. Run after regenerating the comparison if
either input catalog changes; its hashes prevent applying stale exclusions.
"""

import argparse
import collections
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import unicodedata


ROOT = Path(__file__).resolve().parents[1]
COMPARISON = ROOT / "docs/chambersim-projectile-comparison-2026-10-26.json"
DESTINATION = ROOT / "crates/ballistics-store/catalog/third-party/chambersim"
REPOSITORY = "https://github.com/Schildkroet/ChamberSim-Reloaded"
GRAMS_PER_GRAIN = 0.06479891
CHANGES = (
    "Selected component records; renamed fields and converted grains to grams. "
    "Source lengths retained in notes, not used as confirmed lengths. "
    "Construction, lead-free status and expansion limits not established by this import. "
    "The initial import copied BC values without fitting or independent verification; "
    "personal copies may contain later user edits."
)


def normalized(text):
    text = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode()
    return " ".join(re.findall(r"[a-z0-9]+", text.lower()))


def maker(text):
    text = normalized(text).replace(" ", "")
    return "ppu" if text == "prvipartizan" else text


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def positive(record, key):
    value = record.get(key)
    return value if isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value > 0 else None


def possible_family_match(source, components):
    name = normalized(source["pname"])
    for own in components:
        if maker(own["manufacturer"]) != maker(source["mname"]):
            continue
        grains = own["mass_gr"] or own["mass_g"] / GRAMS_PER_GRAIN
        if abs(grains - source["gmass"]) > 0.51 or abs(own["diameter_mm"] - source["gdia"]) >= 0.03:
            continue
        family = normalized(own["product"])
        if re.search(r"\b" + re.escape(family) + r"\b", name):
            return True
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path, default=ROOT.parent / "ChamberSim-Reloaded")
    args = parser.parse_args()
    source = args.source.resolve()
    comparison = json.loads(COMPARISON.read_text())
    source_directory = source / "data/projectiles"
    own_directory = ROOT / "crates/ballistics-store/catalog/bullets"
    for side, directory in [("ours", own_directory), ("chambersim", source_directory)]:
        expected = comparison["input_file_sha256"][side]
        actual = {path.name: digest(path) for path in directory.glob("*.json")}
        if actual != expected:
            raise SystemExit(f"{side} catalog changed: regenerate and review the comparison first")
    revision = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if revision != comparison["chambersim_revision"]:
        raise SystemExit("ChamberSim revision changed: review and regenerate the comparison first")
    upstream_license = (source / "data/LICENSE.md").read_text()
    if "CC BY-SA 4.0" not in upstream_license:
        raise SystemExit("ChamberSim data license changed: review before importing")
    components = [
        record
        for path in sorted(own_directory.glob("*.json"))
        for record in json.loads(path.read_text())
        if record["kind"] == "component"
    ]
    calibers_path = ROOT / "crates/ballistics-store/catalog/calibers.json"
    calibers = json.loads(calibers_path.read_text())
    excluded = {
        match["chambersim_key"]
        for row in comparison["component_comparisons"]
        for match in row["matches"]
    }
    records = collections.defaultdict(list)
    decisions = []
    selected = set()
    for path in sorted(source_directory.glob("*.json")):
        raw = path.read_text()
        lines = raw.splitlines()
        for index, row in enumerate(json.loads(raw)):
            key = f"{path.name}:{index}"
            bc_g1, bc_g7 = (positive(row, key) for key in ["g1bc", "g7bc"])
            reason = None
            if key in excluded:
                reason = "possible_existing_match_in_comparison"
            elif bc_g1 is None and bc_g7 is None:
                reason = "no_bc"
            elif not any(abs(row["gdia"] - caliber["bullet_diameter_mm"]) < 0.03 for caliber in calibers):
                reason = "no_supported_diameter"
            elif not all(bc is None or 0.05 <= bc < 1.2 for bc in [bc_g1, bc_g7]):
                reason = "outside_existing_catalog_bc_bounds"
            elif not (1 <= row["gdia"] <= 16 and 0.1 <= row["gmass"] * GRAMS_PER_GRAIN <= 100):
                reason = "outside_catalog_dimensions"
            elif possible_family_match(row, components):
                reason = "additional_possible_family_match"
            family = re.sub(r"\b19-\d+\b|\b\d{4,}[A-Z]*\b", "", row["pname"], flags=re.I)
            signature = (row["mname"], normalized(family), row["gmass"], row["gdia"], bc_g1, bc_g7)
            if reason is None and signature in selected:
                reason = "possible_packaging_duplicate"
            if reason is not None:
                decisions.append({"source_record": key, "decision": reason})
                continue
            selected.add(signature)
            identity = hashlib.sha256((row["mname"] + "\0" + row["pname"]).encode()).hexdigest()[:16]
            identifier = f"chambersim-{maker(row['mname'])}-{identity}"
            line = next(
                number for number, text in enumerate(lines, 1)
                if '"pname"' in text and json.loads("{" + text.strip().rstrip(",") + "}")["pname"] == row["pname"]
            )
            url = f"{REPOSITORY}/blob/{revision}/data/projectiles/{path.name}#L{line}"
            notes = (
                f"Imported from ChamberSim Reloaded on {comparison['date']}; no fresh manufacturer check. "
                f"Source record: {key} (zero-based index). "
                f"Source length: {row['glen']:g} mm; active length left unknown in this first import. "
                f"{row.get('descr', '').strip()} "
                "The source's general validation flag is not physical BC verification. "
                "Construction, lead-free status and expansion limits are unknown; "
                "the lead-free flag is not asserted."
            )
            record = {
                "id": identifier, "kind": "component", "manufacturer": row["mname"],
                "product": row["pname"], "caliber": None, "diameter_mm": row["gdia"],
                "mass_g": round(row["gmass"] * GRAMS_PER_GRAIN, 8), "mass_gr": row["gmass"],
                "length_mm": None, "bc_g1": bc_g1, "bc_g7": bc_g7,
                "muzzle_velocity_mps": None, "test_barrel_mm": None, "construction": "other",
                "lead_free": False, "min_expansion_mps": None, "source_url": url,
                "verified_on": "", "notes": notes,
                "data_attribution": {
                    "credit": "ChamberSim Reloaded project database", "source_url": url,
                    "source_revision": revision, "license": "CC-BY-SA-4.0",
                    "license_url": "https://creativecommons.org/licenses/by-sa/4.0/", "changes": CHANGES,
                },
            }
            filename = normalized(row["mname"]).replace(" ", "-") + ".json"
            records[filename].append(record)
            decisions.append({"source_record": key, "decision": "imported", "catalog_id": identifier})
    destination = DESTINATION / "bullets"
    destination.mkdir(parents=True, exist_ok=True)
    if {path.name for path in destination.glob("*.json")} - records.keys():
        raise SystemExit("Unexpected existing generated files: inspect them before regenerating")
    for filename, rows in records.items():
        (destination / filename).write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")
    (DESTINATION / "UPSTREAM-LICENSE.md").write_text(upstream_license)
    counts = dict(collections.Counter(row["decision"] for row in decisions))
    manifest = {
        "source_repository": REPOSITORY, "source_revision": revision,
        "imported_on": comparison["date"], "license": "CC-BY-SA-4.0",
        "changes": CHANGES, "input_file_sha256": comparison["input_file_sha256"],
        "comparison_sha256": digest(COMPARISON), "calibers_sha256": digest(calibers_path),
        "counts": counts, "decisions": decisions,
    }
    (DESTINATION / "import-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(counts, indent=2))
    print(f"Wrote {sum(map(len, records.values()))} records in {len(records)} files to {destination}")


if __name__ == "__main__":
    main()
