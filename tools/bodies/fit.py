"""fit.py <mesh.glb> <drawing-id> <out.json> [colours] [--no-legs] [--size=F]

A textured model into a catalog body: closed, turned (x to the head, y up,
z towards whoever looks at the drawing), standing at y = 0, scaled and moved
so its side view covers the drawing best (so the drawing's hit area lands on
the same part of the body), one colour per triangle from the texture,
reduced to a palette. For a four-legged animal it then lines up the front
legs; --no-legs leaves that out (birds). --size=F then scales it by F
about the aim point's foot, for a model whose pose fools the overlap into
a size the animal isn't (the red squirrel's raised tail).
"""
import sys, json, math, itertools, pathlib
import numpy as np, trimesh
from PIL import Image, ImageDraw
from scipy.cluster.vq import kmeans2

args = [a for a in sys.argv[1:] if not a.startswith("--")]
legs = "--no-legs" not in sys.argv
size = next((float(a.split("=", 1)[1]) for a in sys.argv if a.startswith("--size=")), 1.0)
src, did, out = args[:3]
ncol = int(args[3]) if len(args) > 3 else 32
CAT = pathlib.Path(__file__).resolve().parents[2] / "crates/ballistics-store/catalog/drawings.json"
d = next(x for x in json.load(open(CAT)) if x["id"] == did)

m = trimesh.load(src, force="mesh")
tex = np.asarray(m.visual.material.baseColorTexture.convert("RGB"), float)
h, w = tex.shape[:2]
uv = np.asarray(m.visual.uv)[np.asarray(m.faces)]
# Average the texture over a few points in each triangle.
samples = []
for a, b, c in [(1/3, 1/3, 1/3), (2/3, 1/6, 1/6), (1/6, 2/3, 1/6), (1/6, 1/6, 2/3)]:
    p = uv[:, 0] * a + uv[:, 1] * b + uv[:, 2] * c
    samples.append(tex[np.clip(((1 - p[:, 1]) * h).astype(int), 0, h - 1), np.clip((p[:, 0] * w).astype(int), 0, w - 1)])
rgb = np.mean(samples, 0)

body = trimesh.Trimesh(m.vertices, m.faces, process=False)
body.merge_vertices(merge_tex=True, merge_norm=True)
n0 = len(body.faces)
def close_holes(body):
    """Fill each hole with a fan, its triangles running against the hole's
    edges so the winding stays consistent."""
    from collections import Counter
    F = np.asarray(body.faces)
    half = [(int(a), int(b)) for f in F for a, b in ((f[0], f[1]), (f[1], f[2]), (f[2], f[0]))]
    count = Counter(tuple(sorted(e)) for e in half)
    nxt = {b: a for a, b in half if count[tuple(sorted((a, b)))] == 1}  # reversed boundary half-edges
    new = []
    verts = list(np.asarray(body.vertices))
    while nxt:
        start = next(iter(nxt)); loop = [start]
        while nxt[loop[-1]] != start:
            if nxt[loop[-1]] in loop:  # holes sharing a corner: no simple loop
                raise KeyError(nxt[loop[-1]])
            loop.append(nxt[loop[-1]])
        for v in loop: nxt.pop(v)
        if len(loop) == 3:
            new.append(loop)
            continue
        # A new corner in the hole's middle, so no diagonal can repeat an
        # edge the mesh already has.
        centre = len(verts); verts.append(np.asarray(body.vertices)[loop].mean(0))
        for i in range(len(loop)):
            new.append([loop[i], loop[(i + 1) % len(loop)], centre])
    if new:
        body.vertices = np.vstack([np.asarray(body.vertices), np.array(verts[len(body.vertices):]).reshape(-1, 3)])
        body.faces = np.vstack([F, np.array(new)])
def remesh(body, rgb, faces=5000):
    """A model of many loose or crossing pieces (feathers) as one closed
    surface: filled as voxels, thickened by one (so thin feathers don't
    pinch when simplified), smoothed, wrapped and simplified; each triangle
    takes the colour of the model's nearest one."""
    import fast_simplification
    from scipy import ndimage
    from skimage import measure
    grid = body.voxelized(np.ptp(body.vertices, 0).max() / 150).fill()
    field = ndimage.binary_dilation(np.pad(grid.matrix, 3), iterations=1)
    field = ndimage.gaussian_filter(field.astype(float), 0.8)
    V, F, _, _ = measure.marching_cubes(field, 0.4)
    surface = trimesh.Trimesh(trimesh.transform_points(V - 3, grid.transform), F)
    surface = max(surface.split(only_watertight=False), key=lambda part: len(part.faces))
    for aim in (faces, faces * 1.1, faces * 1.2, faces * 0.9, faces * 0.8, faces * 1.3, faces * 1.5, faces * 0.7):
        V2, F2 = fast_simplification.simplify(np.asarray(surface.vertices, np.float32), np.asarray(surface.faces), target_reduction=1 - aim / len(surface.faces))
        out = trimesh.Trimesh(V2, F2)
        E = np.concatenate([out.faces[:, [0, 1]], out.faces[:, [1, 2]], out.faces[:, [2, 0]]])
        edges, count = np.unique(E, axis=0, return_counts=True)
        seen = set(map(tuple, edges))
        if (count == 1).all() and all((b, a) in seen for a, b in edges):
            break
    else:
        raise SystemExit("the simplified surface isn't closed")
    _, _, nearest = trimesh.proximity.closest_point(body, out.triangles_center)
    print(f"rebuilt as one closed surface: {len(body.faces)} -> {len(out.faces)} triangles")
    return out, rgb[nearest]

try:
    close_holes(body)
    closed = body.is_watertight and body.is_winding_consistent
except KeyError:  # holes that share corners: not a simple surface
    closed = False
if closed and len(body.faces) > n0:  # new faces take their nearest old face's colour
    cen = body.triangles_center
    _, _, near = trimesh.proximity.closest_point(trimesh.Trimesh(body.vertices, body.faces[:n0], process=False), cen[n0:])
    rgb = np.vstack([rgb, rgb[near]])
if not closed:
    body = trimesh.Trimesh(m.vertices, m.faces, process=False)
    body.merge_vertices(merge_tex=True, merge_norm=True)
    body, rgb = remesh(body, rgb)
assert body.is_watertight, "still open"
if body.volume < 0:
    body.invert()

# Turn: the body's length along x, then the end with the highest point
# (antlers, head; the fit below checks it) to +x, y up (glTF is y-up); keep it a rotation. A model
# standing at a slant seen from above (the wolverine) is turned by its main
# axis there; one within 10 degrees of an axis keeps to the axis, so a
# turned head doesn't tilt it.
V = np.asarray(body.vertices, float)
def lengthwise(V):
    xz = V[:, [0, 2]] - V[:, [0, 2]].mean(0)
    w, vec = np.linalg.eigh(xz.T @ xz)
    angle = math.atan2(vec[1, 1], vec[0, 1])  # of the main axis, from x
    snap = round(angle / (math.pi / 2)) * (math.pi / 2)
    if abs(angle - snap) < math.radians(10):
        angle = snap
    c, s_ = math.cos(angle), math.sin(angle)
    # Turn about y by -angle: the main axis onto x.
    return V @ np.array([[c, 0, s_], [0, 1, 0], [-s_, 0, c]]).T
V = lengthwise(V)
top = V[np.argsort(V[:, 1])[-50:]]
if top[:, 0].mean() < V[:, 0].mean():  # head at -x: turn half round about y
    V = V * np.array([-1, 1, -1])
# The head turned towards the viewer (+z): mirror across the body's middle if
# it clearly looks away (the tip of the snout, antlers apart, against the
# body's median). A mirror keeps the side view, so the fit and placement stay
# valid. Done after the fit, where the drawing's height leaves the antlers out.
def mirror_to_viewer(V, F):
    body_ = V[V[:, 1] < height]
    length_ = np.ptp(body_[:, 0])
    snout = body_[body_[:, 0] > body_[:, 0].max() - 0.08 * length_]
    if snout[:, 2].mean() - np.median(body_[:, 2]) > -0.03 * length_:
        return V, F
    middle = (V[:, 2].min() + V[:, 2].max()) / 2
    V = V.copy(); V[:, 2] = 2 * middle - V[:, 2]
    print("mirrored, so the head turns towards the viewer")
    return V, F[:, ::-1]

# Fit to the drawing: uniform scale and a shift along x, feet on the ground,
# by how well the side views overlap.
pts = np.array([p for part in d["parts"] for p in part["points"]])
lo, hi = pts.min(0), pts.max(0)
N = 400
cell = max(hi - lo) * 1.2 / N
origin = np.array([lo[0] - 0.1 * (hi - lo).max(), 0.0])
def raster(polys):
    img = Image.new("1", (N, N), 0); dr = ImageDraw.Draw(img)
    for poly in polys:
        dr.polygon([((x - origin[0]) / cell, N - 1 - (y - origin[1]) / cell) for x, y in poly], fill=1)
    return np.asarray(img)
want = raster([part["points"] for part in d["parts"] if not part.get("antler")])
F = np.asarray(body.faces)
V[:, 1] -= V[:, 1].min()
def side(V):
    return raster([V[f][:, :2] for f in F])
# Exclude the antlers from the fit too: what is above the drawing's highest
# non-antler point, scaled, is cut off before comparing. Both ends are tried
# as the head (half a turn about y), as the highest point can be a tail (the
# squirrel's).
height = max(p[1] for part in d["parts"] if not part.get("antler") for p in part["points"])
def fitted(U):
    best = None
    for s in np.linspace(0.8, 1.3, 26):
        for dx in np.linspace(-0.25, 0.25, 21):
            k = height / np.percentile(U[:, 1], 97) * s
            W = U * k; W[:, 0] += (lo[0] + hi[0]) / 2 - (W[:, 0].min() + W[:, 0].max()) / 2 + dx
            got = side(W) & (np.arange(N)[:, None] >= N - 1 - height / cell * 1.02)
            iou = (got & want).sum() / (got | want).sum()
            if best is None or iou > best[0]: best = (iou, k, W)
    return best
best = fitted(V)
turned_round = fitted(V * np.array([-1, 1, -1]))
# Only when clearly better, so a nearly symmetric body (a seal) isn't turned
# tail first by noise.
if turned_round[0] > best[0] + 0.02:
    best = turned_round
    print("turned round: the highest point is the tail")
iou, k, V = best
print(f"side-view overlap with the drawing (no antlers) {iou:.3f}, scale {k:.3f}")

# The hit area sits behind the front leg, so line the front legs up: the
# middle of what the side view covers in the front half, a quarter of the
# shoulder height up.
def front_legs(raster_img):
    row = raster_img[int(N - 1 - d["shoulder_height_m"] * 0.25 / cell)]
    xs = origin[0] + (np.flatnonzero(row) + 0.5) * cell
    middle = (lo[0] + hi[0]) / 2
    xs = xs[xs > middle]
    return (xs.min() + xs.max()) / 2
if legs:
    shift = front_legs(want) - front_legs(side(V))
    V[:, 0] += shift
    print(f"moved {shift:+.3f} m along to line up the front legs")

if size != 1:
    V[:, 0] = d["vital"][0] + (V[:, 0] - d["vital"][0]) * size
    V[:, 1] *= size
    V[:, 2] = V[:, 2].mean() + (V[:, 2] - V[:, 2].mean()) * size
    print(f"scaled by {size} (--size)")

V, F = mirror_to_viewer(V, F)

# Palette.
cent, label = kmeans2(rgb, ncol, seed=1, minit="++")
cols = ["#%02X%02X%02X" % tuple(int(round(c)) for c in cc) for cc in cent]
json.dump({"id": did,
           "vertices": np.round(V, 4).tolist(),
           "triangles": F.tolist(),
           "colours": cols,
           "face_colour": label.tolist()}, open(out, "w"), separators=(",", ":"))
print(f"{len(V)} vertices, {len(F)} triangles, {np.round(np.ptp(V,0),2)} m, {pathlib.Path(out).stat().st_size//1024} KB")
# Overlay for checking.
ov = np.zeros((N, N, 3), np.uint8) + 255
got = side(V)
ov[want] = (170, 170, 170); ov[got & ~want] = (220, 60, 60); ov[got & want] = (80, 80, 80)
vx, vy = d["vital"]
Image.fromarray(ov).save(out.replace(".json", "-overlay.png"))
