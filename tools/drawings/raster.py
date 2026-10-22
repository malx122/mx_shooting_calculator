"""Raster helpers for the drawing tools, in pure Python: shapes in metres are
rasterised into rows of bits, merged, cut, opened or closed, and traced back
into smooth polygons. `configure` sets the window; the defaults suit an
animal up to 4 m long and 2.8 m tall."""
import math

R = 400                 # pixels per metre (2.5 mm)
X0, Y0 = -2.0, -0.05    # world coordinates of pixel (0, 0): x right, y up
W, H = 1600, 1120       # pixels
FULL = (1 << W) - 1


def configure(r=None, x0=None, y0=None, width_m=None, height_m=None):
    """Sets the raster window: `r` pixels per metre, the lower-left corner
    (x0, y0) and the size in metres."""
    global R, X0, Y0, W, H, FULL
    if r is not None: R = r
    if x0 is not None: X0 = x0
    if y0 is not None: Y0 = y0
    if width_m is not None: W = int(width_m * R)
    if height_m is not None: H = int(height_m * R)
    FULL = (1 << W) - 1


def px(x, y):
    return (x - X0) * R, (y - Y0) * R


def world(i, j):
    return i / R + X0, j / R + Y0


class Ellipse:
    def __init__(self, cx, cy, rx, ry, tilt=0.0):
        self.cx, self.cy, self.rx, self.ry, self.t = cx, cy, rx, ry, math.radians(tilt)

    def bbox(self):
        m = max(self.rx, self.ry)
        return self.cx - m, self.cy - m, self.cx + m, self.cy + m

    def inside(self, x, y):
        dx, dy = x - self.cx, y - self.cy
        c, s = math.cos(self.t), math.sin(self.t)
        u, v = dx * c + dy * s, -dx * s + dy * c
        return (u / self.rx) ** 2 + (v / self.ry) ** 2 <= 1.0


class Capsule:
    """A segment from a to b with radius ra at a and rb at b (a tapered
    limb segment)."""
    def __init__(self, a, b, ra, rb=None):
        self.a, self.b, self.ra, self.rb = a, b, ra, ra if rb is None else rb

    def bbox(self):
        m = max(self.ra, self.rb)
        return (min(self.a[0], self.b[0]) - m, min(self.a[1], self.b[1]) - m,
                max(self.a[0], self.b[0]) + m, max(self.a[1], self.b[1]) + m)

    def inside(self, x, y):
        ax, ay = self.a
        bx, by = self.b
        dx, dy = bx - ax, by - ay
        L2 = dx * dx + dy * dy
        t = 0.0 if L2 == 0 else max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / L2))
        r = self.ra + (self.rb - self.ra) * t
        ex, ey = x - (ax + t * dx), y - (ay + t * dy)
        return ex * ex + ey * ey <= r * r


class Polygon:
    def __init__(self, pts):
        self.pts = pts

    def bbox(self):
        xs = [p[0] for p in self.pts]
        ys = [p[1] for p in self.pts]
        return min(xs), min(ys), max(xs), max(ys)

    def inside(self, x, y):
        inside = False
        pts = self.pts
        j = len(pts) - 1
        for i, (xi, yi) in enumerate(pts):
            xj, yj = pts[j]
            if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
                inside = not inside
            j = i
        return inside


def rasterise(shapes):
    """Rows of bits (bit i of rows[j] = pixel (i, j))."""
    rows = [0] * H
    for shape in shapes:
        x0, y0, x1, y1 = shape.bbox()
        i0, j0 = px(x0, y0)
        i1, j1 = px(x1, y1)
        for j in range(max(0, int(j0)), min(H, int(j1) + 2)):
            bits = 0
            y = world(0, j + 0.5)[1]
            for i in range(max(0, int(i0)), min(W, int(i1) + 2)):
                x = world(i + 0.5, 0)[0]
                if shape.inside(x, y):
                    bits |= 1 << i
            rows[j] |= bits
    return rows



def dilate(rows, r):
    out = [0] * H
    widths = [int(math.sqrt(max(0.0, r * r - dy * dy)) + 0.5) for dy in range(r + 1)]
    for j in range(H):
        acc = 0
        for dy in range(-r, r + 1):
            jj = j + dy
            if 0 <= jj < H and rows[jj]:
                row = rows[jj]
                w = widths[abs(dy)]
                h = row
                for d in range(1, w + 1):
                    h |= (row << d) | (row >> d)
                acc |= h
        out[j] = acc & FULL
    return out


def erode(rows, r):
    inv = [FULL ^ row for row in rows]
    return [FULL ^ row for row in dilate(inv, r)]


def close(rows, r):
    return erode(dilate(rows, r), r)


def open_(rows, r):
    return dilate(erode(rows, r), r)


def union(a, b):
    return [x | y for x, y in zip(a, b)]


def intersect(a, b):
    return [x & y for x, y in zip(a, b)]


def subtract(a, b):
    return [x & ~y for x, y in zip(a, b)]


def components(rows):
    """Connected components (4-neighbour), largest first, as row lists."""
    seen = [0] * H
    comps = []
    for j in range(H):
        rem = rows[j] & ~seen[j]
        while rem:
            i = (rem & -rem).bit_length() - 1
            comp = [0] * H
            stack = [(i, j)]
            while stack:
                ci, cj = stack.pop()
                if not (0 <= ci < W and 0 <= cj < H):
                    continue
                bit = 1 << ci
                if not (rows[cj] & bit) or (seen[cj] & bit):
                    continue
                # fill the horizontal run
                lo = ci
                while lo > 0 and rows[cj] & (1 << (lo - 1)) and not (seen[cj] & (1 << (lo - 1))):
                    lo -= 1
                hi = ci
                while hi < W - 1 and rows[cj] & (1 << (hi + 1)) and not (seen[cj] & (1 << (hi + 1))):
                    hi += 1
                run = ((1 << (hi - lo + 1)) - 1) << lo
                seen[cj] |= run
                comp[cj] |= run
                for k in range(lo, hi + 1):
                    stack.append((k, cj - 1))
                    stack.append((k, cj + 1))
            comps.append(comp)
            rem = rows[j] & ~seen[j]
    comps.sort(key=lambda c: -sum(bin(r).count('1') for r in c))
    return comps


def outline(rows):
    """The outer boundary of the mask's largest component as pixel corners,
    by crack following along pixel edges with the region on the left
    (anticlockwise). Corner (i, j) is the lower-left corner of pixel (i, j)."""
    comp = components(rows)[0]

    def on(i, j):
        return 0 <= i < W and 0 <= j < H and (comp[j] >> i) & 1

    j0 = next(j for j in range(H) if comp[j])
    i0 = (comp[j0] & -comp[j0]).bit_length() - 1
    # heading: 0 east, 1 north, 2 west, 3 south
    step = [(1, 0), (0, 1), (-1, 0), (0, -1)]
    # the pixels left and right of the edge ahead, by heading
    left = [lambda i, j: (i, j), lambda i, j: (i - 1, j), lambda i, j: (i - 1, j - 1), lambda i, j: (i, j - 1)]
    right = [lambda i, j: (i, j - 1), lambda i, j: (i, j), lambda i, j: (i - 1, j), lambda i, j: (i - 1, j - 1)]
    i, j, d = i0, j0, 0
    pts = []
    for _ in range(400000):
        if on(*left[d](i, j)):
            if on(*right[d](i, j)):
                d = (d - 1) % 4          # region on both sides ahead: turn right
                continue
            if pts and (i, j, d) == (i0, j0, 0):
                break                    # back at the start, about to repeat it
            pts.append((i, j))
            i += step[d][0]
            j += step[d][1]
        else:
            d = (d + 1) % 4              # nothing on the left ahead: turn left
    return pts


def smooth(pts, k=3, passes=2):
    n = len(pts)
    for _ in range(passes):
        out = []
        for i in range(n):
            sx = sy = 0.0
            for d in range(-k, k + 1):
                p = pts[(i + d) % n]
                sx += p[0]
                sy += p[1]
            out.append((sx / (2 * k + 1), sy / (2 * k + 1)))
        pts = out
    return pts


def simplify(pts, tol):
    """Douglas-Peucker on a closed ring (iterative)."""
    n = len(pts)
    if n < 4:
        return pts
    b = max(range(n), key=lambda i: (pts[i][0] - pts[0][0]) ** 2 + (pts[i][1] - pts[0][1]) ** 2)
    ring = pts[b:] + pts[:b]            # start at the point farthest from pts[0]
    ring.append(ring[0])
    keep = [False] * len(ring)
    keep[0] = keep[-1] = True
    # split first at the point farthest from the start, then recurse on
    # each piece with an explicit stack
    stack = [(0, len(ring) - 1)]
    while stack:
        lo, hi = stack.pop()
        if hi - lo < 2:
            continue
        (ax, ay), (bx, by) = ring[lo], ring[hi]
        dx, dy = bx - ax, by - ay
        L = math.hypot(dx, dy)
        best, bi = -1.0, lo
        for i in range(lo + 1, hi):
            x, y = ring[i]
            if L < 1e-9:
                d = math.hypot(x - ax, y - ay)
            else:
                d = abs(dx * (ay - y) - (ax - x) * dy) / L
            if d > best:
                best, bi = d, i
        if best > tol:
            keep[bi] = True
            stack.append((lo, bi))
            stack.append((bi, hi))
    return [p for p, k in zip(ring[:-1], keep[:-1]) if k]


def polygon(rows, close_r=0, smooth_k=3, tol=0.8):
    """One smooth world-space polygon of the mask's largest component."""
    if close_r:
        rows = close(rows, close_r)
    pts = outline(rows)
    pts = smooth(pts, smooth_k)
    pts = simplify(pts, tol)
    return [[round(world(i, j)[0], 4), round(world(i, j)[1], 4)] for i, j in pts]


def polygons(rows, close_r=0, smooth_k=3, tol=0.8, min_px=30):
    """Every component as a polygon."""
    if close_r:
        rows = close(rows, close_r)
    out = []
    for comp in components(rows):
        if sum(bin(r).count('1') for r in comp) < min_px:
            continue
        pts = simplify(smooth(outline(comp), smooth_k), tol)
        out.append([[round(world(i, j)[0], 4), round(world(i, j)[1], 4)] for i, j in pts])
    return out
