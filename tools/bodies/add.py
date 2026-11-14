"""Add or update an animal's 3D body in one go.

    python3 tools/bodies/add.py <model.glb> <drawing-id> [--no-legs]
    python3 tools/bodies/add.py --place <drawing-id> '{"placement":{...}}'

The first fits the model to the drawing (fit.py; --no-legs for birds),
marks the antlers if the drawing has them (antlers.py), puts the body into
crates/ballistics-store/catalog/bodies.json (replacing one with the same id,
placement back to the fit) and writes the hit-area editor. The second puts
the placement the editor gives into bodies.json. Both then run only the
catalog body test; nothing else needs building to add an animal.

Previews and the editor go to target/bodies/. Run it with the venv that has
numpy, scipy, trimesh, rtree, pillow, scikit-image and fast-simplification.
"""
import json, pathlib, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CATALOG = ROOT / "crates/ballistics-store/catalog"
BODIES = CATALOG / "bodies.json"
OUT = ROOT / "target/bodies"


def run(*args):
    subprocess.run([sys.executable, *map(str, args)], check=True)


def save(bodies):
    # One body a line, so a change shows as that line in the diff.
    BODIES.write_text("[\n" + ",\n".join(json.dumps(b, separators=(",", ":")) for b in bodies) + "\n]\n")


def put(body):
    bodies = [b for b in json.loads(BODIES.read_text()) if b["id"] != body["id"]]
    save(bodies + [body])


def test():
    subprocess.run(["cargo", "test", "-q", "-p", "ballistics-store", "catalog_bodies"], cwd=ROOT, check=True)


def add(model, id_, legs):
    drawing = next((d for d in json.loads((CATALOG / "drawings.json").read_text()) if d["id"] == id_), None)
    if drawing is None:
        sys.exit(f"no drawing {id_!r} in drawings.json")
    OUT.mkdir(parents=True, exist_ok=True)
    fitted = OUT / f"{id_}.json"
    run(HERE / "fit.py", model, id_, fitted, *([] if legs else ["--no-legs"]))
    print(f"check {OUT / (id_ + '-overlay.png')}")
    if any(part.get("antler") for part in drawing["parts"]):
        # Antlers grow above the withers, are thin for the animal's size and
        # reach above the ears.
        low = drawing["shoulder_height_m"] * 0.9
        thin = drawing["length_m"] * 0.025
        seed = drawing["shoulder_height_m"] * 0.11
        run(HERE / "antlers.py", fitted, OUT / f"{id_}-antlers.png", low, 0, thin, seed)
        print(f"check {OUT / (id_ + '-antlers.png')}: the ears must stay brown")
    put(json.loads(fitted.read_text()))
    page = OUT / f"{id_}-editor.html"
    run(HERE / "zone_editor.py", id_, page)
    print(f"place the hit area: open {page}")
    test()


def place(id_, text):
    placement = json.loads(text)
    placement = placement.get("placement", placement)
    bodies = json.loads(BODIES.read_text())
    body = next((b for b in bodies if b["id"] == id_), None)
    if body is None:
        sys.exit(f"no body {id_!r} in bodies.json")
    body["placement"] = {"offset": placement["offset"], "scale": placement["scale"]}
    save(bodies)
    test()


if __name__ == "__main__":
    args = sys.argv[1:]
    if args[:1] == ["--place"] and len(args) == 3:
        place(args[1], args[2])
    elif len(args) >= 2 and not args[0].startswith("--"):
        add(args[0], args[1], "--no-legs" not in args)
    else:
        sys.exit(__doc__)
