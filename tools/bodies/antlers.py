"""antlers.py <body.json> <preview.png> [y_low] [lum] [thin] [seed]: mark the
antler triangles (thin faces high on the head, grown from those within `seed`
metres of the top, so ears lower down stay out) and show them red."""
import sys, json, numpy as np
from collections import defaultdict, deque
from PIL import Image, ImageDraw
path, png = sys.argv[1:3]
y_low = float(sys.argv[3]) if len(sys.argv) > 3 else 1.6
L = float(sys.argv[4]) if len(sys.argv) > 4 else 95
m = json.load(open(path)); V = np.array(m["vertices"]); F = np.array(m["triangles"])
cols = np.array([[int(c[i:i+2], 16) for i in (1, 3, 5)] for c in m["colours"]]); fc = np.array(m["face_colour"])
lum = (cols @ [0.3, 0.59, 0.11])[fc]
cen = V[F].mean(1)
head_x = np.percentile(V[:, 0], 60)
import trimesh
mesh = trimesh.Trimesh(V, F, process=False)
# Thickness: from each face's centre straight in, to the other side.
normals = mesh.face_normals
hits, rays, _ = mesh.ray.intersects_location(cen - normals * 1e-4, -normals, multiple_hits=False)
thick = np.full(len(F), 9.9); thick[rays] = np.linalg.norm(hits - cen[rays], axis=1)
THIN = float(sys.argv[5]) if len(sys.argv) > 5 else 0.06
ok = (cen[:, 1] > y_low) & (thick < THIN) & (cen[:, 0] > head_x - 1.0)
print("thin high faces", ok.sum())
edges = defaultdict(list)
for i, f in enumerate(F):
    for a, b in ((f[0], f[1]), (f[1], f[2]), (f[2], f[0])):
        edges[tuple(sorted((a, b)))].append(i)
nbr = defaultdict(list)
for fs in edges.values():
    for a in fs:
        for b in fs:
            if a != b: nbr[a].append(b)
SEED = float(sys.argv[6]) if len(sys.argv) > 6 else 0.25
seed = [i for i in np.flatnonzero(ok) if cen[i, 1] > V[:, 1].max() - SEED]
mark = np.zeros(len(F), bool); q = deque(seed); mark[seed] = True
while q:
    i = q.popleft()
    for j in nbr[i]:
        if not mark[j] and ok[j]:
            mark[j] = True; q.append(j)
for _ in range(2):
    for i in np.flatnonzero(~mark & (cen[:, 1] > y_low)):
        if sum(mark[j] for j in nbr[i]) >= 2: mark[i] = True
m["antler_faces"] = np.flatnonzero(mark).tolist()
json.dump(m, open(path, "w"), separators=(",", ":"))
print(f"{mark.sum()} antler triangles")
img = Image.new("RGB", (1400, 560), (230, 235, 230)); dr = ImageDraw.Draw(img); s = 220
for view, off in ((0, 0), (1, 700)):
    order = np.argsort(cen[:, 2] if view == 0 else -cen[:, 0])
    for i in order:
        pts = [(off + 350 + (x if view == 0 else z) * s * 0.8, 540 - y * s) for x, y, z in V[F[i]]]
        dr.polygon(pts, fill=(220, 40, 40) if mark[i] else tuple(cols[fc[i]]))
img.save(png)
