"""Write an editor for where a 3D body stands against its drawing's hit area.

    python3 tools/bodies/zone_editor.py moose /tmp/zone-editor.html

Open the page in a browser. It shows the body straight from the side with the
hit area and rings on it, and the drawing with its hit area beside it (drop
a picture of the training figure there too: it stays on the computer). Drag
the hit area onto the body and scale it; copy the placement it gives into the
body's "placement" in crates/ballistics-store/catalog/bodies.json.

The hit area and rings keep their size and place against the aim point in
the app: moving or scaling them here moves and scales the body under them.
Needs numpy and pillow.
"""
import base64, io, json, pathlib, sys

import numpy as np
from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOG = ROOT / "crates/ballistics-store/catalog"
SUPER = 2  # supersampling for smooth edges


def png(img):
    buffer = io.BytesIO()
    img.save(buffer, "PNG", optimize=True)
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


def frame(points, ppm, margin=0.15):
    lo, hi = points.min(0), points.max(0)
    x0, y1 = lo[0] - margin, hi[1] + margin
    size = (int((hi[0] - lo[0] + 2 * margin) * ppm), int((hi[1] - lo[1] + 2 * margin) * ppm))
    return x0, y1, size


def side_view(body, ppm):
    """The body from its near side, flat-shaded, and how its pixels map to
    metres."""
    V = np.array(body["vertices"], float)
    F = np.array(body["triangles"])
    palette = [tuple(int(c[i:i + 2], 16) for i in (1, 3, 5)) for c in body["colours"]]
    x0, y1, size = frame(V[:, :2], ppm)
    img = Image.new("RGB", (size[0] * SUPER, size[1] * SUPER), (201, 210, 190))
    draw = ImageDraw.Draw(img)
    tri = V[F]
    normal = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
    normal /= np.linalg.norm(normal, axis=1, keepdims=True) + 1e-12
    light = np.array([-0.4, 0.8, 0.6]) / np.linalg.norm([-0.4, 0.8, 0.6])
    for i in np.argsort(tri[:, :, 2].mean(1)):
        if normal[i, 2] <= 0:
            continue
        shade = 0.55 + 0.45 * max(0.0, float(normal[i] @ light))
        colour = tuple(int(c * shade) for c in palette[body["face_colour"][i]])
        draw.polygon([((x - x0) * ppm * SUPER, (y1 - y) * ppm * SUPER) for x, y, _ in tri[i]], fill=colour)
    img = img.resize(size, Image.LANCZOS)
    antler = set(body.get("antler_faces", []))
    keep = [i for i in range(len(F)) if i not in antler]
    kept = V[np.unique(F[keep])][:, 1]
    return img, x0, y1, float(kept.max()), float(kept.min())


def drawing_view(drawing, ppm, circle=None):
    pts = np.array([p for part in drawing["parts"] for p in part["points"]])
    x0, y1, size = frame(pts, ppm)
    img = Image.new("RGBA", (size[0] * SUPER, size[1] * SUPER), (201, 210, 190, 255))
    draw = ImageDraw.Draw(img, "RGBA")
    at = lambda x, y: ((x - x0) * ppm * SUPER, (y1 - y) * ppm * SUPER)
    for part in drawing["parts"]:
        draw.polygon([at(x, y) for x, y in part["points"]], fill=part["fill"])
    for zone in drawing.get("zones", [])[::-1]:
        draw.polygon([at(x, y) for x, y in zone["points"]], fill=(110, 170, 110, 140), outline=(240, 236, 220, 255))
    if circle:
        cx, cy = at(*drawing["vital"])
        r = circle / 2 * ppm * SUPER
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(110, 170, 110, 140), outline=(240, 236, 220, 255), width=SUPER)
    rings = drawing.get("rings")
    if rings:
        cx, cy = at(*rings["center"])
        for ring in rings["rings"]:
            r = ring["diameter_m"] / 2 * ppm * SUPER
            draw.ellipse([cx - r, cy - r, cx + r, cy + r], outline=(240, 236, 220, 255), width=SUPER)
    return img.resize(size, Image.LANCZOS)


def main():
    id_, out = sys.argv[1], sys.argv[2]
    body = next(b for b in json.load(open(CATALOG / "bodies.json")) if b["id"] == id_)
    drawing = next(d for d in json.load(open(CATALOG / "drawings.json")) if d["id"] == id_)
    ppm = 1400 / max(np.ptp(np.array(body["vertices"])[:, 0]), 0.1)
    side, x0, y1, top, bottom = side_view(body, ppm)
    # Without a hit area of its own, the app draws a vital circle at the aim
    # point, as big as the target says.
    circle = None if drawing.get("zones") else next(
        (t["vital_diameter_cm"] / 100 for t in json.load(open(CATALOG / "targets.json")) if t.get("silhouette") == id_), None)
    reference = drawing_view(drawing, 420 / max(np.ptp(np.array([p for part in drawing["parts"] for p in part["points"]])[:, 0]), 0.1), circle)
    data = {
        "id": id_,
        "name": drawing["name"],
        "side": png(side),
        "x0": x0, "y1": y1, "ppm": ppm,
        "top": top,
        "bottom": bottom,
        "antlers": bool(body.get("antler_faces")),
        "reference": png(reference),
        "vital": drawing["vital"],
        "zones": [z["points"] for z in drawing.get("zones", [])],
        "rings": drawing.get("rings"),
        "circle": circle,
        "placement": body.get("placement", {"offset": [0, 0], "scale": 1}),
    }
    html = (pathlib.Path(__file__).with_name("zone-editor.html")).read_text()
    pathlib.Path(out).write_text(html.replace("/*DATA*/null", json.dumps(data)))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
