"""Draws docs/cartridge-letters.svg: the C.I.P. drawing letters, dimensioned on
the app's own cartridge model (not copied from any C.I.P. or SAAMI sheet).

    cargo run -q -p ballistics-store --example cartridge_outline -- 308-win > outline.json
    python3 tools/cartridge-drawing.py outline.json docs/cartridge-letters.svg

It expects a rimless, bottlenecked case with an extractor groove and a rear
rim bevel, as .308 Win has.
"""
import json
import math
import sys

outline = json.load(open(sys.argv[1]))
out_path = sys.argv[2]

v = outline["letters"]
R, R1, f, beta = v["R"], v["R1"], v["f"], v["beta"]
E1, emin, delta = v["E1"], v["e"], v["delta"]
E, P1, L1, P2, L2, H1, H2, L3, L6 = (v[k] for k in ("E", "P1", "L1", "P2", "L2", "H1", "H2", "L3", "L6"))
G1 = v["G1"]

points = [outline["start"]]
mats = []
for s in outline["stretches"]:
    for p in s["points"]:
        points.append(p)
        mats.append(s["material"])

INK = "#1d2a22"
DIM = "#2f6b8f"
CASE = "#ead9ab"
BULLET = "#e3b48e"
FONT = "font-family='Helvetica, Arial, sans-serif'"
W, H = 1060, 990
svg = [f"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {W} {H}' width='{W}' height='{H}'>",
       f"<rect width='{W}' height='{H}' fill='white'/>",
       "<defs><marker id='a' viewBox='0 0 10 10' refX='10' refY='5' markerWidth='7' markerHeight='7' "
       f"orient='auto-start-reverse'><path d='M0,1 L10,5 L0,9 z' fill='{DIM}'/></marker></defs>"]


def text(x, y, s, size=15, anchor="middle", color=INK, weight="normal", italic=False):
    style = " font-style='italic'" if italic else ""
    svg.append(f"<text x='{x:.1f}' y='{y:.1f}' font-size='{size}' text-anchor='{anchor}' fill='{color}' "
               f"font-weight='{weight}'{style} {FONT}>{s}</text>")


def line(x1, y1, x2, y2, color=DIM, width=1, dash=None, arrows=False):
    d = f" stroke-dasharray='{dash}'" if dash else ""
    m = " marker-start='url(#a)' marker-end='url(#a)'" if arrows else ""
    svg.append(f"<line x1='{x1:.1f}' y1='{y1:.1f}' x2='{x2:.1f}' y2='{y2:.1f}' stroke='{color}' "
               f"stroke-width='{width}'{d}{m}/>")


def path(pts, fill, close=True):
    d = "M" + " L".join(f"{x:.2f},{y:.2f}" for x, y in pts) + (" Z" if close else "")
    svg.append(f"<path d='{d}' fill='{fill}' stroke='{INK}' stroke-width='1.3' stroke-linejoin='round'/>")


# ---- Main view: the whole cartridge, head on the left.
S = 11.5            # px per mm
X0, Y0 = 70, 210    # case head on the axis
X = lambda z: X0 + z * S
Yt = lambda r: Y0 - r * S
Yb = lambda r: Y0 + r * S

text(W / 2, 34, f"Cartridge drawing letters (C.I.P.), drawn on the {outline['name']} model", 20, weight="bold")
text(W / 2, 58, "Maximum cartridge: each value is the largest the standard allows; a tolerance says how far below it a cartridge may be.", 13)

case_pts = [p for p, m in zip(points[1:], mats) if m != "Bullet"]
bullet_pts = [p for p, m in zip(points[1:], mats) if m == "Bullet"]
mouth = case_pts[-1]
case_outline = [(X(z), Yt(r)) for z, r in [(0, 0)] + case_pts] + [(X(z), Yb(r)) for z, r in reversed([(0, 0)] + case_pts)]
path(case_outline, CASE)
bul = [(X(mouth[0]), Yt(mouth[1]))] + [(X(z), Yt(r)) for z, r in bullet_pts] + \
      [(X(z), Yb(r)) for z, r in reversed(bullet_pts)] + [(X(mouth[0]), Yb(mouth[1]))]
path(bul, BULLET)
line(X0 - 30, Y0, X(L6) + 25, Y0, INK, 0.8, "10 4 2 4")

# Lengths from the head, below.
line(X0, Yb(R1 / 2) + 6, X0, Yb(R1 / 2) + 82 + 3 * 40 + 8, DIM, 0.8)
for i, (lab, z, note) in enumerate([("L1", L1, "shoulder starts"), ("L2", L2, "neck starts"),
                                     ("L3", L3, "case length"), ("L6", L6, "overall length")]):
    y = Yb(R1 / 2) + 82 + i * 40
    r_at = {"L1": P2 / 2, "L2": H1 / 2, "L3": H2 / 2, "L6": 0}[lab]
    line(X(z), Yb(r_at) + 4, X(z), y + 6, DIM, 0.8)
    line(X0, y, X(z), y, arrows=True)
    text((X0 + X(z)) / 2, y - 6, f"<tspan font-weight='bold'>{lab}</tspan>  {note}", 14, color=DIM)

# Diameters, above: vertical dimensions across the case.
for lab, z, d, note in [("P1", E + 1.2, P1, "body at E"), ("P2", L1 - 0.6, P2, "body at L1"),
                        ("H1", L2 + 0.8, H1, "neck at L2"), ("H2", L3 - 0.5, H2, "mouth"),
                        ("G1", L3 + 3.2, G1, "bullet")]:
    x = X(z)
    line(x, Yt(d / 2), x, Yb(d / 2), arrows=True)
    line(x, Yt(d / 2) - 4, x, Yt(7.4) - 2, DIM, 0.8)
    text(x, Yt(7.4) - 22, f"<tspan font-weight='bold'>{lab}</tspan>", 15, color=DIM)
    text(x, Yt(7.4) - 7, note, 11, color=DIM)

# Shoulder roundings.
for lab, z, r, dx, dy in [("r1", L1, P2 / 2, -34, 40), ("r2", L2, H1 / 2, 30, 46)]:
    x, y = X(z), Yb(r)
    line(x, y, x + dx, y + dy, INK, 0.8)
    text(x + dx + (6 if dx > 0 else -6), y + dy + 12, f"<tspan font-weight='bold'>{lab}</tspan>", 14,
         anchor="start" if dx > 0 else "end")

# ---- Head detail: the upper half of the head, enlarged.
DS = 62
DX0, DY0 = 110, 860     # head face at DX0, axis at DY0 (below the view)
dX = lambda z: DX0 + z * DS
dY = lambda r: DY0 - (r - 3.4) * DS   # show radius 3.4 … 6.2 mm
text(DX0 - 40, 560, "Head and extractor groove, enlarged (upper half)", 16, anchor="start", weight="bold")
head = [(z, r) for z, r in case_pts if z <= 5.2 and r >= 3.4]
pts = [(dX(0), dY(3.4))] + [(dX(z), dY(r)) for z, r in head] + [(dX(5.2), dY(P1 / 2 - (P1 - P2) / 2 * (5.2 - E) / (L1 - E))), (dX(5.2), dY(3.4))]
path(pts, CASE)
# head face and axis direction
inset = f / math.tan(math.radians(beta))
line(dX(0), dY(3.4) + 8, dX(0), dY(R1 / 2) - 70, DIM, 0.8)
# lengths along the axis, under the detail
for i, (lab, z0, z1, note) in enumerate([("f", 0, f, "rear bevel length"), ("R", 0, R, "rim thickness"),
                                          ("E", 0, E, "where the body starts")]):
    y = dY(R1 / 2) - 22 - i * 30
    line(dX(z1), dY({"f": R1 / 2, "R": R1 / 2, "E": P1 / 2}[lab]) - 3, dX(z1), y - 6, DIM, 0.8)
    line(dX(z0), y, dX(z1), y, arrows=True)
    text(dX(z1) + 8, y + 5, f"<tspan font-weight='bold'>{lab}</tspan>  {note}", 13, anchor="start", color=DIM)
# groove floor e, below the groove
yfl = dY(E1 / 2) + 30
line(dX(R), dY(E1 / 2) + 3, dX(R), yfl + 6, DIM, 0.8)
line(dX(R + emin), dY(E1 / 2) + 3, dX(R + emin), yfl + 6, DIM, 0.8)
line(dX(R), yfl, dX(R + emin), yfl, arrows=True)
text(dX(R + emin / 2), yfl + 20, "<tspan font-weight='bold'>e</tspan> min  groove floor", 13, color=DIM)
# diameters as labels with leaders
for lab, z, r, note, dx in [("R1", R * 0.95, R1 / 2, "rim diameter", 0), ("E1", R + emin * 0.5, E1 / 2, "groove diameter", 0),
                            ("P1", E + 0.6, P1 / 2, "body diameter", 0)]:
    x, y = dX(z), dY(r)
    tx, ty = {"R1": (dX(5.2) + 30, y - 30), "E1": (x + 170, y + 70), "P1": (x + 60, y + 44)}[lab]
    line(x, y, tx - 6, ty - 4, INK, 0.8)
    text(tx, ty, f"<tspan font-weight='bold'>{lab}</tspan>  {note}", 13, anchor="start")
# angles
# β: between the head face (radial line) and the rear bevel
bx, by = dX(0), dY(R1 / 2 - inset)
line(bx, by, bx - 46 * math.sin(math.radians(0)), by - 46, INK, 0.7, "3 3")
ang = math.radians(90 - beta)
ex, ey = bx + 60 * math.cos(ang), by - 60 * math.sin(ang)
line(bx, by, ex, ey, INK, 0.7, "3 3")
svg.append(f"<path d='M{bx:.1f},{by - 34:.1f} A34,34 0 0,1 {bx + 34 * math.cos(ang):.1f},{by - 34 * math.sin(ang):.1f}' fill='none' stroke='{DIM}' stroke-width='1.2'/>")
text(bx - 14, by - 26, "β", 16, color=DIM, weight="bold")
# δ: between a line parallel to the axis and the groove's front bevel
floor_end = E - (P1 - E1) / 2 / math.tan(math.radians(delta))
dx0, dy0 = dX(floor_end), dY(E1 / 2)
line(dx0, dy0, dx0 + 120, dy0, INK, 0.7, "3 3")
da = math.radians(delta)
svg.append(f"<path d='M{dx0 + 58:.1f},{dy0:.1f} A58,58 0 0,0 {dx0 + 58 * math.cos(da):.1f},{dy0 - 58 * math.sin(da):.1f}' fill='none' stroke='{DIM}' stroke-width='1.2'/>")
text(dx0 + 70, dy0 - 14, "δ", 16, color=DIM, weight="bold")

# ---- Notes, right column of the lower half.
NX, NY = 700, 590
notes = [
    ("bold", "Angles"),
    ("", "β  rear bevel, to the head face"),
    ("", "δ  groove's front bevel, to the axis"),
    ("bold", "Other cases"),
    ("", "Rimmed, rimfire: no groove;"),
    ("", "the body starts at R."),
    ("", "Belted: R3 is the belt's diameter;"),
    ("", "the groove's bevel rises to it,"),
    ("", "the belt runs to E."),
    ("", "Straight: no shoulder."),
    ("bold", "Tolerances"),
    ("", "\"L1\": −0.20 → the shoulder may"),
    ("", "start up to 0.20 mm nearer the head."),
    ("", "C.I.P. gives few (mostly L1, L2,"),
    ("", "P2, R); SAAMI gives almost all."),
]
y = NY
for w, s in notes:
    if w:
        y += 8
    text(NX, y, s, 15 if w else 13, anchor="start", weight=w or "normal")
    y += 21
text(W - 20, H - 14, "Drawn from the app's own model, not copied from a C.I.P. or SAAMI sheet.", 11, anchor="end", color="#666")

svg.append("</svg>")
open(out_path, "w").write("\n".join(svg) + "\n")
print("wrote", out_path)
