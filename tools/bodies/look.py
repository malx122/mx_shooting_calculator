"""look.py <mesh> <out.png>: textured mesh, face colours from the texture at each face's UV centre, 4 views."""
import sys, math, numpy as np, trimesh
from PIL import Image, ImageDraw
m = trimesh.load(sys.argv[1], force='mesh')
V = np.asarray(m.vertices, float); F = np.asarray(m.faces)
tex = m.visual.material.image if hasattr(m.visual.material, 'image') and m.visual.material.image is not None else m.visual.material.baseColorTexture
tex = tex.convert('RGB'); T = np.asarray(tex); h, w = T.shape[:2]
uv = np.asarray(m.visual.uv)[F].mean(1)
C = T[np.clip(((1 - uv[:, 1]) * h).astype(int), 0, h - 1), np.clip((uv[:, 0] * w).astype(int), 0, w - 1)]
light = np.array([0.4, 0.8, 0.45]); light /= np.linalg.norm(light)
def view(yaw, pitch, size=700):
    cy, sy, cp, sp = math.cos(yaw), math.sin(yaw), math.cos(pitch), math.sin(pitch)
    R = np.array([[1,0,0],[0,cp,-sp],[0,sp,cp]]) @ np.array([[cy,0,sy],[0,1,0],[-sy,0,cy]])
    P = (V - V.mean(0)) @ R.T
    s = size * 0.85 / max(np.ptp(P[:, 0]), np.ptp(P[:, 1]))
    img = Image.new('RGB', (size, size), (225, 232, 225)); dr = ImageDraw.Draw(img)
    tri = P[F]; n = np.cross(tri[:,1]-tri[:,0], tri[:,2]-tri[:,0]); n /= np.linalg.norm(n, axis=1, keepdims=True)+1e-12
    nw = np.cross(V[F[:,1]]-V[F[:,0]], V[F[:,2]]-V[F[:,0]]); nw /= np.linalg.norm(nw, axis=1, keepdims=True)+1e-12
    for i in np.argsort(tri[:, :, 2].mean(1)):
        if n[i, 2] <= 0: continue
        sh = 0.5 + 0.5 * max(0, nw[i] @ light)
        dr.polygon([(size/2 + p[0]*s, size/2 - p[1]*s) for p in tri[i]], fill=tuple(int(c*sh) for c in C[i]))
    return img
views = [view(a, b) for a, b in [(-0.7, 0.25), (math.pi/2, 0.0), (0.0, 0.1), (2.4, 0.3)]]
W = Image.new('RGB', (1400, 1400)); [W.paste(v, ((i%2)*700, (i//2)*700)) for i, v in enumerate(views)]; W.save(sys.argv[2])
