"""Draws a catalog drawing from a textured 3D model (a Meshy GLB), for an
animal that has a model but no drawing yet:

    python3 tools/drawings/mesh2drawing.py wapiti.glb --id wapiti \\
        --name "Wapiti (bull)" --shoulder 1.5 --antler-seed 2.2 --write

The model is seen straight from the side, head to the right, standing on
y = 0 and scaled so the top of the back (the top of the side outline's body
between its middle and the neck, antler tips above it apart) is at
--shoulder. Its texture is cut to
a few flat colours (--colours), each a polygon; antlers are found as in
tools/bodies/antlers.py and marked. The vital point goes above the front
legs, 0.564 of the chest depth below the back, as svg2drawing.py puts it.
Check the --png it writes. Then fit the same model to it with
tools/bodies/add.py. Needs the venv of tools/bodies (numpy, scipy, trimesh,
pillow).
"""
import argparse, json, math, os, subprocess, sys, tempfile

import numpy as np
import trimesh
from PIL import Image, ImageDraw, ImageFilter
from scipy.cluster.vq import kmeans2

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import raster as rs  # noqa: E402


def face_colours(m):
    tex = np.asarray(m.visual.material.baseColorTexture.convert("RGB"), float)
    h, w = tex.shape[:2]
    uv = np.asarray(m.visual.uv)[np.asarray(m.faces)]
    samples = []
    for a, b, c in [(1 / 3, 1 / 3, 1 / 3), (2 / 3, 1 / 6, 1 / 6), (1 / 6, 2 / 3, 1 / 6), (1 / 6, 1 / 6, 2 / 3)]:
        p = uv[:, 0] * a + uv[:, 1] * b + uv[:, 2] * c
        samples.append(tex[np.clip(((1 - p[:, 1]) * h).astype(int), 0, h - 1), np.clip((p[:, 0] * w).astype(int), 0, w - 1)])
    return np.mean(samples, 0)


def turned(V):
    """The body's length along x (its main axis seen from above, kept to an
    axis within 10 degrees of one, as tools/bodies/fit.py turns it), the end
    with the highest point (antlers, head) to +x, y up (glTF is y-up), feet
    at y = 0."""
    xz = V[:, [0, 2]] - V[:, [0, 2]].mean(0)
    vec = np.linalg.eigh(xz.T @ xz)[1]
    angle = math.atan2(vec[1, 1], vec[0, 1])
    snap = round(angle / (math.pi / 2)) * (math.pi / 2)
    if abs(angle - snap) < math.radians(10):
        angle = snap
    c, s = math.cos(angle), math.sin(angle)
    V = V @ np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]]).T
    top = V[np.argsort(V[:, 1])[-50:]]
    if top[:, 0].mean() < V[:, 0].mean():
        V = V * np.array([-1, 1, -1])
    V[:, 1] -= V[:, 1].min()
    return V


def antler_faces(V, F, colours, shoulder, length, thick=1.0, seed=1.0):
    """tools/bodies/antlers.py on the model, with add.py's settings."""
    labels = kmeans2(colours, 32, seed=1, minit="++")[1]
    cent = [colours[labels == k].mean(0) if (labels == k).any() else np.zeros(3) for k in range(32)]
    body = {"vertices": V.tolist(), "triangles": F.tolist(), "face_colour": labels.tolist(),
            "colours": ["#%02X%02X%02X" % tuple(int(round(v)) for v in c) for c in cent]}
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "body.json")
        json.dump(body, open(path, "w"))
        script = os.path.join(HERE, "..", "bodies", "antlers.py")
        args = [shoulder * 0.9, 0, length * 0.025 * thick, shoulder * 0.11 * seed]
        subprocess.run([sys.executable, script, path, os.path.join(tmp, "a.png"), *map(str, args)], check=True,
                       stdout=subprocess.DEVNULL)
        faces = json.load(open(path)).get("antler_faces", [])
        if not faces:  # horns are much thicker than antlers
            args[2] *= 2.5
            subprocess.run([sys.executable, script, path, os.path.join(tmp, "a.png"), *map(str, args)], check=True,
                           stdout=subprocess.DEVNULL)
            faces = json.load(open(path)).get("antler_faces", [])
    mark = np.zeros(len(F), bool)
    mark[faces] = True
    return mark


def bits(mask):
    """A boolean image (row 0 at the top) as raster.py rows (row 0 at the bottom)."""
    rows = []
    for row in mask[::-1]:
        rows.append(int.from_bytes(np.packbits(row[::-1]).tobytes(), "big") >> ((-len(row)) % 8))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("model")
    ap.add_argument("--id", required=True)
    ap.add_argument("--name", required=True)
    ap.add_argument("--shoulder", type=float, required=True, help="height of the top of the back, metres")
    ap.add_argument("--colours", type=int, default=3, help="flat colours for the body (antlers get one more)")
    ap.add_argument("--bird", action="store_true")
    ap.add_argument("--antler-thick", type=float, default=1.0, help="antlers thicker than add.py takes them, times")
    ap.add_argument("--antler-seed", type=float, default=1.0,
                    help="look for antler tips this many times deeper below the top than add.py does")
    ap.add_argument("--png", help="a picture of the drawing, to check it")
    ap.add_argument("--out", help="write the drawing to this JSON file")
    ap.add_argument("--write", action="store_true", help="put it into the catalog (put.py)")
    args = ap.parse_args()

    m = trimesh.load(args.model, force="mesh")
    rgb = face_colours(m)
    # One surface: corners split for the texture joined again (as fit.py
    # does), so the antler search can grow across the seams.
    joined = trimesh.Trimesh(m.vertices, m.faces, process=False)
    joined.merge_vertices(merge_tex=True, merge_norm=True)
    V, F = turned(np.asarray(joined.vertices, float)), np.asarray(joined.faces)

    def withers(V):
        """The top of the back: in the side outline between the middle of
        the body and the neck, the top of each column's longest run (the
        body, not an antler tip above it), highest of them."""
        ppm = 200 / np.ptp(V[:, 1])
        size = (int(np.ptp(V[:, 0]) * ppm) + 2, int(np.ptp(V[:, 1]) * ppm) + 2)
        img = Image.new("1", size, 0)
        d = ImageDraw.Draw(img)
        x0 = V[:, 0].min()
        for t in V[F]:
            d.polygon([((x - x0) * ppm, size[1] - 1 - y * ppm) for x, y, _ in t], fill=1)
        a = np.asarray(img)
        tops = []
        for i in range(int(size[0] * 0.40), int(size[0] * 0.62)):
            col = np.concatenate([[0], a[::-1, i].astype(int), [0]])
            e = np.flatnonzero(np.diff(col))
            lo, hi = max(zip(e[::2], e[1::2]), key=lambda r: r[1] - r[0])
            tops.append(hi / ppm)
        return float(np.percentile(tops, 90))

    V = V * (args.shoulder / withers(V))
    V[:, 0] -= (V[:, 0].min() + V[:, 0].max()) / 2
    length = float(np.ptp(V[:, 0]))
    antler = antler_faces(V, F, rgb, args.shoulder, length, args.antler_thick, args.antler_seed)

    # Flat colours: the body's few, the antlers' one.
    k = args.colours
    cent, label = kmeans2(rgb[~antler], k, seed=1, minit="++")
    cls = np.full(len(F), k)
    cls[~antler] = label
    fills = [tuple(int(round(v)) for v in c) for c in cent] + [tuple(int(round(v)) for v in rgb[antler].mean(0)) if antler.any() else (0, 0, 0)]

    # The near side, one colour index a pixel (255: background), smoothed so
    # the low-poly mottle merges into patches.
    rs.configure(r=400, x0=V[:, 0].min() - 0.1, y0=-0.05, width_m=length + 0.2, height_m=V[:, 1].max() + 0.1)
    img = Image.new("L", (rs.W, rs.H), 255)
    draw = ImageDraw.Draw(img)
    tri = V[F]
    for i in np.argsort(tri[:, :, 2].mean(1)):  # seen from +z, far faces first
        draw.polygon([((x - rs.X0) * rs.R, rs.H - 1 - (y - rs.Y0) * rs.R) for x, y, _ in tri[i]], fill=int(cls[i]))
    body_img = np.asarray(img.filter(ImageFilter.ModeFilter(17)))
    raw = np.asarray(img)
    inside = raw != 255

    parts = []
    main_class = int(np.bincount(body_img[inside & (body_img < k)].ravel(), minlength=k).argmax())
    hexes = ["#%02X%02X%02X" % c for c in fills]
    # The whole animal (antlers apart) in its main colour, then the other
    # patches on top, so no gaps show between them.
    whole = inside & (raw != k)
    for poly in rs.polygons(bits(whole), smooth_k=1, tol=0.6, min_px=40):
        parts.append({"fill": hexes[main_class], "points": poly})
    for c in range(k):
        if c == main_class:
            continue
        for poly in rs.polygons(bits((body_img == c) & whole), close_r=3, smooth_k=2, tol=0.8, min_px=3000):
            parts.append({"fill": hexes[c], "points": poly})
    for poly in rs.polygons(bits(inside & (raw == k)), smooth_k=1, tol=0.6, min_px=40):
        parts.append({"fill": hexes[k], "points": poly, "antler": True})

    # The vital point: above the front legs, 0.564 of the chest depth below
    # the back (svg2drawing.py's rule, scaled from the moose).
    scale = args.shoulder / 1.87
    row = whole[rs.H - 1 - int((args.shoulder * 0.25 - rs.Y0) * rs.R)]
    xs = (np.flatnonzero(row) + 0.5) / rs.R + rs.X0
    xs = xs[xs > 0]
    if not len(xs):
        sys.exit("no front legs found a quarter of the shoulder height up: is the model standing, head to one end?")
    leg_x = (xs.min() + xs.max()) / 2

    def trunk(x):
        """Back and belly heights at x: the column's longest run (the body,
        not an antler above it or a leg below)."""
        col = np.concatenate([[False], whole[::-1, int((x - rs.X0) * rs.R)], [False]])
        edges = np.flatnonzero(np.diff(col.astype(int)))
        runs = list(zip(edges[::2], edges[1::2]))
        low, high = max(runs, key=lambda r: r[1] - r[0])
        return high / rs.R + rs.Y0, low / rs.R + rs.Y0

    back_y = trunk(leg_x - 0.12 * scale)[0]
    belly_y = trunk(leg_x - 0.45 * scale)[1]
    vital = [round(float(leg_x - 0.02 * scale), 3), round(float(back_y - 0.564 * (back_y - belly_y)), 3)]

    drawing = {"id": args.id, "name": args.name, "bird": args.bird, "shoulder_height_m": args.shoulder,
               "length_m": round(length, 3), "vital": vital, "parts": parts}
    print(f"{len(parts)} parts, {sum(len(p['points']) for p in parts)} points; length {length:.2f} m; "
          f"{int(antler.sum())} antler triangles; vital {vital}")
    if args.png:
        ppm = 300
        pic = Image.new("RGB", (int((length + 0.2) * ppm), int((V[:, 1].max() + 0.1) * ppm)), (230, 235, 230))
        d = ImageDraw.Draw(pic)
        at = lambda x, y: ((x - rs.X0) * ppm, pic.height - (y - rs.Y0) * ppm)
        for part in parts:
            d.polygon([at(x, y) for x, y in part["points"]], fill=part["fill"])
        cx, cy = at(*vital)
        d.ellipse([cx - 6, cy - 6, cx + 6, cy + 6], outline=(220, 40, 40), width=3)
        pic.save(args.png)
    if args.out:
        json.dump([drawing], open(args.out, "w"), indent=1, ensure_ascii=False)
    if args.write:
        import put
        put.put(drawing)


if __name__ == "__main__":
    main()
