"""Turns a flat-filled SVG of an animal (for example from Recraft) into a
catalog drawing for crates/ballistics-store/catalog/drawings.json.

    python3 tools/drawings/svg2drawing.py moose.svg --id moose --name "Moose (bull)" \\
        --shoulder 1.87 --zone tools/drawings/zones/moose-sjf.json --write

What it does: white shapes drawn over others become real gaps (the space
between the legs); every path becomes polygons in metres, feet at y = 0 and
the top of the back at --shoulder; pale legs get dark hooves; a dark far
leg can get a pale lower leg like the others; the vital point goes above
the front leg, 0.564 of the chest depth below the back, as the rings sit on
the SJF moose figure; a --zone file adds the hit area (a spline through its
points, in cm around the vital point) and the training rings.

Shapes are told apart by their fill: the body is the largest non-white
shape, antlers are shapes whose middle lies above the back, and pale legs
are the other shapes that reach the ground; --body, --antler and --leg
override that. Fills are written as in the SVG, e.g. "rgb(67,59,50)"."""
import argparse, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import raster as rs

HOOF = "#201913"
WHITE = 'rgb(255,255,255)'


def bezier(p0, p1, p2, p3, n=12):
    out = []
    for i in range(1, n + 1):
        t = i / n
        u = 1 - t
        out.append((u * u * u * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t * t * t * p3[0],
                    u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1]))
    return out


def flatten(d):
    """Subpaths of an SVG path (M, L, H, V, C, S, Q, T, Z) as point lists."""
    tokens = re.findall(r'[MmLlHhVvCcSsQqTtZz]|-?\d*\.?\d+(?:e-?\d+)?', d)
    i = 0
    cmd = None
    cur = (0.0, 0.0)
    start = None
    last_ctrl = None
    subs, pts = [], []

    def num():
        nonlocal i
        v = float(tokens[i])
        i += 1
        return v
    while i < len(tokens):
        t = tokens[i]
        if re.match(r'[A-Za-z]', t):
            cmd = t
            i += 1
            if cmd in 'Zz':
                if pts:
                    subs.append(pts)
                pts = []
                if start:
                    cur = start
                continue
        rel = cmd.islower()
        c = cmd.upper()
        if c == 'M':
            x, y = num(), num()
            if rel:
                x, y = cur[0] + x, cur[1] + y
            if pts:
                subs.append(pts)
            pts = [(x, y)]
            cur = start = (x, y)
            cmd = 'l' if rel else 'L'
        elif c == 'L':
            x, y = num(), num()
            if rel:
                x, y = cur[0] + x, cur[1] + y
            pts.append((x, y))
            cur = (x, y)
        elif c == 'H':
            x = num()
            x = cur[0] + x if rel else x
            pts.append((x, cur[1]))
            cur = (x, cur[1])
        elif c == 'V':
            y = num()
            y = cur[1] + y if rel else y
            pts.append((cur[0], y))
            cur = (cur[0], y)
        elif c in 'CS':
            if c == 'C':
                x1, y1 = num(), num()
            else:
                x1, y1 = (2 * cur[0] - last_ctrl[0], 2 * cur[1] - last_ctrl[1]) if last_ctrl else cur
            x2, y2, x, y = num(), num(), num(), num()
            if rel:
                if c == 'C':
                    x1, y1 = cur[0] + x1, cur[1] + y1
                x2, y2, x, y = cur[0] + x2, cur[1] + y2, cur[0] + x, cur[1] + y
            pts += bezier(cur, (x1, y1), (x2, y2), (x, y))
            last_ctrl = (x2, y2)
            cur = (x, y)
            continue
        elif c in 'QT':
            if c == 'Q':
                x1, y1 = num(), num()
            else:
                x1, y1 = (2 * cur[0] - last_ctrl[0], 2 * cur[1] - last_ctrl[1]) if last_ctrl else cur
            x, y = num(), num()
            if rel:
                if c == 'Q':
                    x1, y1 = cur[0] + x1, cur[1] + y1
                x, y = cur[0] + x, cur[1] + y
            c1 = (cur[0] + 2 / 3 * (x1 - cur[0]), cur[1] + 2 / 3 * (y1 - cur[1]))
            c2 = (x + 2 / 3 * (x1 - x), y + 2 / 3 * (y1 - y))
            pts += bezier(cur, c1, c2, (x, y))
            last_ctrl = (x1, y1)
            cur = (x, y)
            continue
        last_ctrl = None
    if pts:
        subs.append(pts)
    return subs


def spline(points, steps=8):
    """Catmull-Rom through the points, closed."""
    n = len(points)
    out = []
    for i in range(n):
        p0, p1, p2, p3 = points[(i - 1) % n], points[i], points[(i + 1) % n], points[(i + 2) % n]
        for s in range(steps):
            t = s / steps
            t2, t3 = t * t, t * t * t
            out.append(tuple(0.5 * (2 * p1[k] + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2
                                    + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3) for k in (0, 1)))
    return out


def hex_of(fill):
    nums = re.findall(r'\d+', fill)
    return '#%02X%02X%02X' % tuple(int(v) for v in nums[:3]) if len(nums) >= 3 else fill


def rows_bounds(mask):
    ys = [j for j in range(rs.H) if mask[j]]
    if not ys:
        return None
    xs = [i for j in ys for i in ((mask[j] & -mask[j]).bit_length() - 1, mask[j].bit_length() - 1)]
    return rs.world(min(xs), min(ys)), rs.world(max(xs), max(ys))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('svg')
    ap.add_argument('--id', required=True)
    ap.add_argument('--name', required=True, help='e.g. "Moose (bull)"')
    ap.add_argument('--shoulder', type=float, required=True, help='height of the top of the back, metres')
    ap.add_argument('--zone', help='a zone file: {"name", "points_cm", "rings"}')
    ap.add_argument('--body', help='the body fill (default: the largest shape)')
    ap.add_argument('--leg', action='append', default=[], help='a pale leg fill (repeatable)')
    ap.add_argument('--antler', action='append', default=[], help='an antler fill (repeatable)')
    ap.add_argument('--recolour', action='append', default=[], metavar='OLD=NEW', help='e.g. "rgb(131,108,105)=#8C8375"')
    ap.add_argument('--antler-recolour', action='append', default=[], metavar='OLD=NEW',
                    help='a recolour applied to antler shapes only (a far antler sharing the legs\' grey)')
    ap.add_argument('--far-leg', default='#8C8375', help='colour of pale legs made for dark far legs; "none" to skip')
    ap.add_argument('--no-hooves', action='store_true')
    ap.add_argument('--vital', help='X,Y in metres, instead of the SJF rule')
    ap.add_argument('--out', help='write the drawing to this JSON file')
    ap.add_argument('--write', action='store_true', help='put it into the catalog (tools/drawings/put.py)')
    args = ap.parse_args()

    svg = open(args.svg).read()
    shapes = []
    for m in re.finditer(r'<path\b([^>]*)>', svg):
        attrs = m.group(1)
        fill = re.search(r'fill="([^"]*)"', attrs)
        d = re.search(r' d="([^"]*)"', attrs)
        if not fill or not d:
            continue
        tr = re.search(r'translate\(([-\d.]+),([-\d.]+)\)', attrs)
        dx, dy = (float(tr.group(1)), float(tr.group(2))) if tr else (0.0, 0.0)
        shapes.append((fill.group(1), [[(x + dx, y + dy) for x, y in sub] for sub in flatten(d.group(1))]))
    # a white rectangle first is the background
    if shapes and shapes[0][0] == WHITE:
        shapes = shapes[1:]
    recolour = dict(item.split('=', 1) for item in args.recolour)
    antler_recolour = dict(item.split('=', 1) for item in args.antler_recolour)

    # metres: feet at the lowest point, the body's top at --shoulder
    def area(subs):
        return sum(abs(sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(sub, sub[1:] + sub[:1]))) / 2 for sub in subs)
    body_fill = args.body or max((s for s in shapes if s[0] != WHITE), key=lambda s: area(s[1]))[0]
    allpts = [p for fill, subs in shapes if fill != WHITE for sub in subs for p in sub]
    ground = max(p[1] for p in allpts)
    top = min(p[1] for fill, subs in shapes if fill == body_fill for sub in subs for p in sub)
    scale = args.shoulder / (ground - top)
    xs = [p[0] for p in allpts]
    x_mid = (min(xs) + max(xs)) / 2
    length = (max(xs) - min(xs)) * scale
    to_m = lambda p: ((p[0] - x_mid) * scale, (ground - p[1]) * scale)

    rs.configure(r=400, x0=-length / 2 - 0.2, y0=-0.05, width_m=length + 0.4, height_m=args.shoulder * 1.6)
    masks = [(fill, rs.rasterise([rs.Polygon([to_m(p) for p in sub]) for sub in subs])) for fill, subs in shapes]

    # which shapes are what
    def kind(fill, mask):
        (x0, y0), (x1, y1) = rows_bounds(mask)
        if fill == body_fill:
            return 'body'
        if fill in args.antler or (not args.antler and (y0 + y1) / 2 > args.shoulder):
            return 'antler'                       # sits above the back
        if fill in args.leg or (not args.leg and y0 < 0.03 and y1 < args.shoulder * 0.65):
            return 'leg'
        return 'other'

    parts = []
    legs = []
    for k, (fill, mask) in enumerate(masks):
        if fill == WHITE:
            continue
        for fill2, m2 in masks[k + 1:]:
            if fill2 == WHITE:
                mask = rs.subtract(mask, m2)       # white on top cuts a gap
        if not rows_bounds(mask):
            continue
        what = kind(fill, mask)
        colour = recolour.get(fill, hex_of(fill))
        if what == 'antler':
            colour = antler_recolour.get(fill, colour)
        for poly in rs.polygons(mask, smooth_k=1, tol=0.6, min_px=40):
            part = {"fill": colour, "points": poly}
            if what == 'antler':
                part["antler"] = True
            parts.append(part)
        if what == 'leg':
            legs.append(mask)
            if not args.no_hooves:
                hoof = [row if rs.world(0, j)[1] < 0.08 else 0 for j, row in enumerate(mask)]
                for poly in rs.polygons(hoof, smooth_k=1, tol=0.6, min_px=40):
                    parts.append({"fill": HOOF, "points": poly})

    body_mask = next(m for f, m in masks if f == body_fill)
    # a dark far leg gets a pale lower leg like the others
    if args.far_leg != 'none' and legs:
        knee = min(rows_bounds(m)[1][1] for m in legs)
        low = [row if rs.world(0, j)[1] < knee * 0.82 else 0 for j, row in enumerate(body_mask)]
        for m in legs:
            low = rs.subtract(low, rs.dilate(m, 6))
        low = rs.open_(low, 4)
        for poly in rs.polygons(low, smooth_k=1, tol=0.6, min_px=2000):
            parts.append({"fill": args.far_leg, "points": poly})
            if not args.no_hooves:
                hoof = [row if rs.world(0, j)[1] < 0.08 else 0 for j, row in enumerate(low)]
                for hp in rs.polygons(hoof, smooth_k=1, tol=0.6, min_px=200):
                    parts.append({"fill": HOOF, "points": hp})

    # the vital point
    if args.vital:
        vital = [float(v) for v in args.vital.split(',')]
    else:
        front = max(legs, key=lambda m: rows_bounds(m)[1][0])
        cols = [i for j in range(rs.H) if front[j] for i in range(rs.W) if (front[j] >> i) & 1]
        leg_x = rs.world(sum(cols) / len(cols), 0)[0]
        col = int((leg_x - 0.12 - rs.X0) * rs.R)
        back = rs.world(0, max(j for j in range(rs.H) if (body_mask[j] >> col) & 1))[1]
        col2 = int((leg_x - 0.45 - rs.X0) * rs.R)
        belly = rs.world(0, min(j for j in range(rs.H) if (body_mask[j] >> col2) & 1))[1]
        vital = [round(leg_x - 0.02, 3), round(back - 0.564 * (back - belly), 3)]

    drawing = {"id": args.id, "name": args.name, "bird": False, "shoulder_height_m": args.shoulder,
               "length_m": round(length, 3), "vital": vital, "parts": parts}
    if args.zone:
        zone = json.load(open(args.zone))
        pts = [[round(vital[0] + x / 100, 4), round(vital[1] + y / 100, 4)] for x, y in spline(zone['points_cm'])]
        drawing["zones"] = [{"name": zone.get("name", "hit area"), "points": pts}]
        if zone.get("rings"):
            r = zone["rings"]
            drawing["rings"] = {"name": r["name"], "center": vital,
                                "rings": [{"diameter_m": d / 100, "score": s} for d, s in zip(r["diameters_cm"], r["scores"])]}
    print(f"{len(parts)} parts, {sum(len(p['points']) for p in parts)} points; length {length:.2f} m; vital {vital}")
    if args.out:
        json.dump([drawing], open(args.out, 'w'), indent=1, ensure_ascii=False)
    if args.write:
        import put
        put.put(drawing)


if __name__ == '__main__':
    main()
