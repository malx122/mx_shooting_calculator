"""Add or update animals' 3D bodies in one go.

    python3 tools/bodies/add.py <model.glb> <drawing-id> [<model.glb> <drawing-id> ...]
    python3 tools/bodies/add.py --place <drawing-id> '{"placement":{...}}'

The first fits each model to its drawing (fit.py; a bird's legs are left
alone), marks the antlers if the drawing has them (antlers.py) and puts the
body into crates/ballistics-store/catalog/bodies.json (replacing one with
the same id, placement back to the fit). Place the hit areas with the editor
(zone_editor.py), which saves the placements itself; the second form does
the same from the command line. Both then run only the catalog body test;
nothing else needs building to add an animal.

Fits and previews go to target/bodies/. Run it with the venv that has
numpy, scipy, trimesh, rtree, pillow, scikit-image and fast-simplification.
"""
import json, os, pathlib, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CATALOG = ROOT / "crates/ballistics-store/catalog"
BODIES = CATALOG / "bodies.json"
OUT = ROOT / "target/bodies"
# Models whose lower antler tips sit further below the top than the usual
# search band reaches (the wapiti's far antler); the drawing was made with the
# same setting (tools/drawings/mesh2drawing.py --antler-seed).
DEEPER = {"wapiti": 2.2}
# Models the overlap fit makes the wrong size, scaled after it (fit.py
# --size): the squirrel's raised tail made it 31 cm nose to rump, against
# 19-23 cm, and the editor scales from 0.6 only.
SIZE = {"red-squirrel": 0.68}


def run(*args):
    subprocess.run([sys.executable, *map(str, args)], check=True)


def save(bodies):
    # One body a line, so a change shows as that line in the diff; written
    # beside it and moved over, so a reader never sees half a file.
    text = "[\n" + ",\n".join(json.dumps(b, separators=(",", ":"), allow_nan=False) for b in bodies) + "\n]\n"
    tmp = BODIES.with_suffix(".json.tmp")
    tmp.write_text(text)
    os.replace(tmp, BODIES)


def put(body):
    bodies = [b for b in json.loads(BODIES.read_text()) if b["id"] != body["id"]]
    save(bodies + [body])


def test():
    subprocess.run(["cargo", "test", "-q", "-p", "ballistics-store", "catalog_bodies"], cwd=ROOT, check=True)


def add(model, id_):
    drawing = next((d for d in json.loads((CATALOG / "drawings.json").read_text()) if d["id"] == id_), None)
    if drawing is None:
        sys.exit(f"no drawing {id_!r} in drawings.json")
    OUT.mkdir(parents=True, exist_ok=True)
    fitted = OUT / f"{id_}.json"
    run(HERE / "fit.py", model, id_, fitted, *(["--no-legs"] if drawing.get("bird") else []),
        *([f"--size={SIZE[id_]}"] if id_ in SIZE else []))
    print(f"check {OUT / (id_ + '-overlay.png')}")
    if any(part.get("antler") for part in drawing["parts"]):
        # Antlers grow above the withers, are thin for the animal's size and
        # reach above the ears.
        low = drawing["shoulder_height_m"] * 0.9
        thin = drawing["length_m"] * 0.025
        seed = drawing["shoulder_height_m"] * 0.11 * DEEPER.get(id_, 1)
        run(HERE / "antlers.py", fitted, OUT / f"{id_}-antlers.png", low, 0, thin, seed)
        if not json.loads(fitted.read_text()).get("antler_faces"):
            # Horns (the mouflon's) are much thicker than antlers.
            run(HERE / "antlers.py", fitted, OUT / f"{id_}-antlers.png", low, 0, thin * 2.5, seed)
        print(f"check {OUT / (id_ + '-antlers.png')}: the ears must stay brown")
    put(json.loads(fitted.read_text()))


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
    elif len(args) >= 2 and len(args) % 2 == 0 and not args[0].startswith("--"):
        for model, id_ in zip(args[::2], args[1::2]):
            add(model, id_)
        test()
        print("place the hit areas: python3 tools/bodies/zone_editor.py")
    else:
        sys.exit(__doc__)
