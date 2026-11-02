#!/usr/bin/env python3
"""Draws the reticles of the first-generation Vortex Viper PST scopes.

The Viper PST (2010-2017, before the Gen II) came as 1-4x24 (TMCQ),
2.5-10x32 FFP, 2.5-10x44, 4-16x50 (FFP and SFP) and 6-24x50 (FFP and SFP),
each in MOA and MRAD. Vortex printed one reticle sheet per model, and the
EBR-1 differs between them: the post width, the gap before the posts, the
line width and, on the second focal plane scopes, the reach of the
hash marks.

This script is the project's own drawing of each, built from the figures
Vortex prints on the sheet (line and post widths, the gap, the hash
spacing); the hash layout and lengths, which the sheets show but don't
dimension, were measured from the sheets' pictures against the printed
numbers. Nothing is traced.

    python3 tools/reticles/vortex_pst_gen1.py          # writes the catalog
    python3 tools/reticles/vortex_pst_gen1.py --check  # fails if out of date

It rewrites its own entries in crates/ballistics-store/catalog/reticles.json
and scopes/vortex.json and leaves every other entry as it is.
"""

import json
import math
import sys
from pathlib import Path

CATALOG = Path(__file__).resolve().parents[2] / "crates/ballistics-store/catalog"
VERIFIED = "2026-11-02"
EDGE = 500.0  # posts and the top line run to the edge of the field of view

OT = "https://www.optics-trade.eu/download/Vortex/VORTEX%20VIPER%20PST%20"


def sheet(name):
    return OT + name.replace(" ", "%20") + "%20Reticle%20Subtensions%20-%20Optics%20Trade.pdf"


def r(value):
    """Rounds away float noise so the JSON stays readable."""
    value = round(value, 4)
    return 0.0 if value == 0 else value


def line(a, b, width, lit=False):
    return {"type": "line", "from": [r(a[0]), r(a[1])], "to": [r(b[0]), r(b[1])],
            "width": width, "illuminated": lit}


def post(x0, y0, x1, y1):
    return {"type": "polygon",
            "points": [[r(x0), r(y0)], [r(x1), r(y0)], [r(x1), r(y1)], [r(x0), r(y1)]]}


def marks(spec, end):
    """Hash positions on one arm: spec is [(from, to, step, length)], later
    ranges override earlier ones at the same position."""
    out = {}
    for start, stop, step, length in spec:
        count = round((stop - start) / step)
        for i in range(count + 1):
            out[round(start + i * step, 4)] = length
    return sorted((pos, half) for pos, half in out.items() if 0 < pos <= end + 1e-9)


def ebr1(d):
    """An EBR-1: a lit fine crosshair with hash marks, a gap, then heavy
    posts left, right and below, and a fine line from the top edge."""
    fine, heavy, gap = d["fine"], d["heavy"], d["gap"]
    h, up, down = d["reach"]
    els = [line((-h, 0), (h, 0), fine, True), line((0, -down), (0, up), fine, True)]
    for pos, half in marks(d["horizontal"], h):
        els.append(line((pos, -half), (pos, half), fine, True))
        els.append(line((-pos, -half), (-pos, half), fine, True))
    for pos, half in marks(d["up"], up):
        els.append(line((-half, pos), (half, pos), fine, True))
    for pos, half in marks(d["down"], down):
        els.append(line((-half, -pos), (half, -pos), fine, True))
    w = heavy / 2
    els.append(post(-EDGE, -w, -(h + gap), w))
    els.append(post(h + gap, -w, EDGE, w))
    els.append(post(-w, -EDGE, w, -(down + gap)))
    els.append(line((0, up + gap), (0, EDGE), fine))
    if "post_marks" in d:
        offset, half = d["post_marks"]
        for side in (-1, 1):
            x = side * (h + gap + offset)
            els.append(line((x, -half), (x, half), fine))
        y = -(down + gap + offset)
        els.append(line((-half, y), (half, y), fine))
    return els


def quarter_ring(inner, outer, cut, sx, sy, steps=12):
    """One lit quarter of the TMCQ's broken circle, cut straight at `cut`
    from both axes, as thin convex pieces (the app fills polygons as convex)."""
    def at(radius, t):
        a0 = math.asin(cut / radius)
        a1 = math.acos(cut / radius)
        a = a0 + (a1 - a0) * t
        return (sx * radius * math.cos(a), sy * radius * math.sin(a))
    els = []
    for i in range(steps):
        # Each piece overlaps the next a little, so no seam shows between them.
        t0, t1 = i / steps, min(1, (i + 1.3) / steps)
        pts = [at(inner, t0), at(outer, t0), at(outer, t1), at(inner, t1)]
        els.append({"type": "polygon", "points": [[r(x), r(y)] for x, y in pts],
                    "illuminated": True})
    return els


def tmcq(d):
    """The TMCQ: a lit dot inside a lit broken circle, and a fine black
    crosshair with hash marks starting just outside the dot."""
    fine = d["fine"]
    els = [{"type": "circle", "center": [0, 0], "radius": d["dot"] / 2, "fill": True,
            "width": None, "illuminated": True}]
    start, end = d["line"]
    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        els.append(line((dx * start, dy * start), (dx * end, dy * end), fine))
        for pos, half in marks(d["marks"], end):
            if dy == 0:
                els.append(line((dx * pos, -half), (dx * pos, half), fine))
            else:
                els.append(line((-half, dy * pos), (half, dy * pos), fine))
    inner, outer, cut = d["ring"]
    for sx, sy in ((1, 1), (-1, 1), (-1, -1), (1, -1)):
        els += quarter_ring(inner, outer, cut, sx, sy)
    return els


# Hash layouts shared between sheets; (from, to, step, half length).
FFP_MRAD_ARM = [(0.5, 5, 0.5, 0.1), (1, 5, 1, 0.2), (5.2, 8, 0.2, 0.1), (6, 8, 1, 0.2)]
FFP_MRAD = {
    "unit": "mrad", "reach": (8, 8, 9), "gap": 1,
    "horizontal": FFP_MRAD_ARM, "up": FFP_MRAD_ARM,
    "down": [(0.5, 9, 0.5, 0.1), (1, 9, 1, 0.2)],
    "post_marks": (1, 0.75),
    "layout": "Hash marks every .5 mrad out to 5 and every .2 from 5 to 8 on the "
              "horizontal arms and up, and every .5 down to 9 (measured: .4 mrad "
              "long at whole mrads, .2 between); a fine line crosses each post 1 "
              "mrad past its inner end, about 1.5 mrad long",
}


def moa_30(short, long_):
    return {
        "unit": "moa", "reach": (30, 30, 30),
        "horizontal": [(2, 8, 2, short), (10, 30, 10, long_), (15, 25, 10, short)],
        "up": [(2, 8, 2, short), (10, 10, 1, long_), (30, 30, 1, long_)],
        "down": [(2, 30, 2, short), (10, 30, 10, long_)],
    }


MOA_30_LAYOUT = ("hash marks every 2 MOA out to 10, then at 15, 20, 25 and 30 on the "
                 "horizontal arms; every 2 MOA down to 30; up, marks to 10 and an end "
                 "mark at 30")

FFP_SOURCE_NOTE = ("FFP: subtensions are constant at all magnifications. Lit = the "
                   "fine crosshair and its marks (red on Vortex's sheet).")


def sfp_note(mag):
    return (f"SFP: the subtensions are true at {mag}x, as the sheet says. Lit = the "
            "fine crosshair and its marks (red on Vortex's sheet).")


RETICLES = [
    {
        "id": "vortex-ebr-1-mrad-viper-pst-6-24x50-ffp",
        "name": "EBR-1 (MRAD) - Viper PST 6-24x50 FFP",
        "for_scope": "Vortex Viper PST (Gen I) 6-24x50 FFP",
        "mag": None, "source": sheet("6-24x50 FFP EBR-1 MRAD"), "part": "PST-R624F1-M-12A",
        **FFP_MRAD, "fine": 0.04, "heavy": 0.65,
        "published": "fine crosshair .04 mrad; heavy crosshair (posts) .65 mrad; gap "
                     "before the posts 1 mrad; hash spacing .2 and .5 mrad",
    },
    {
        "id": "vortex-ebr-1-mrad-viper-pst-4-16x50-ffp",
        "name": "EBR-1 (MRAD) - Viper PST 4-16x50 FFP",
        "for_scope": "Vortex Viper PST (Gen I) 4-16x50 FFP",
        "mag": None, "source": sheet("4-16x50 FFP EBR-1 MRAD"), "part": "PST-R416F1-M-12A",
        **FFP_MRAD, "fine": 0.06, "heavy": 0.8, "picture_posts": 0.63,
        "published": "fine crosshair .06 mrad; heavy crosshair (posts) .8 mrad; gap "
                     "before the posts 1 mrad; hash spacing .2 and .5 mrad",
    },
    {
        "id": "vortex-ebr-1-mrad-viper-pst-2-5-10x32-ffp",
        "name": "EBR-1 (MRAD) - Viper PST 2.5-10x32 FFP",
        "for_scope": "Vortex Viper PST (Gen I) 2.5-10x32 FFP",
        "mag": None, "source": sheet("2.5-10x32 FFP EBR-1 MRAD"), "part": "PST-R43104-12A",
        **FFP_MRAD, "fine": 0.06, "heavy": 0.5, "picture_posts": 0.63,
        "published": "fine crosshair .06 mrad; heavy crosshair (posts) .5 mrad; gap "
                     "before the posts 1 mrad; hash spacing .2 and .5 mrad",
    },
    {
        "id": "vortex-ebr-1-moa-viper-pst-6-24x50-ffp",
        "name": "EBR-1 (MOA) - Viper PST 6-24x50 FFP",
        "for_scope": "Vortex Viper PST (Gen I) 6-24x50 FFP",
        "mag": None, "source": sheet("6-24x50 FFP EBR-1 MOA"), "part": "PST-R624F1-A-12A",
        **moa_30(0.5, 1.4), "gap": 5, "fine": 0.18, "heavy": 3,
        "layout": MOA_30_LAYOUT + " (measured: 2.8 MOA long at 10, 20 and 30, 1 MOA "
                                  "otherwise)",
        "published": "fine crosshair .18 MOA; heavy crosshair (posts) 3 MOA; gap before "
                     "the posts 5 MOA; hash spacing 2 and 5 MOA",
    },
    {
        "id": "vortex-ebr-1-moa-viper-pst-ffp",
        "name": "EBR-1 (MOA) - Viper PST 2.5-10x32 / 4-16x50 FFP",
        "for_scope": "Vortex Viper PST (Gen I) 2.5-10x32 FFP and 4-16x50 FFP",
        "mag": None,
        "source": "https://308ar.com/wp-content/uploads/2015/08/"
                  "Vortex-Viper-PST-4-16x50-FFP-R416F1-A-12A-Reticle.pdf",
        "also_source": sheet("2.5-10x32 FFP EBR-1 MOA"),
        "part": "PST-R416F1-A-12A and PST-R43103-12A",
        **moa_30(0.5, 1.4), "gap": 10, "fine": 0.2, "heavy": 5,
        "layout": MOA_30_LAYOUT + " (measured: 2.8 MOA long at 10, 20 and 30, 1 MOA "
                                  "otherwise). Both sheets carry the identical picture",
        "published": "heavy crosshair (posts) 5 MOA; gap before the posts 10 MOA; hash "
                     "spacing 2 and 5 MOA; fine crosshair .2 MOA on the 2.5-10x32 sheet "
                     "and .21 MOA on the 4-16x50 sheet (drawn .2)",
    },
    {
        "id": "vortex-ebr-1-moa-viper-pst-6-24x50-sfp",
        "name": "EBR-1 (MOA) - Viper PST 6-24x50 SFP",
        "for_scope": "Vortex Viper PST (Gen I) 6-24x50 SFP",
        "mag": 24, "source": sheet("6-24x50 EBR-1 MOA"), "part": "PST-R624S1-A-12A",
        "unit": "moa", "reach": (10, 10, 10), "gap": 1, "fine": 0.06, "heavy": 1,
        "horizontal": [(1, 4, 1, 0.15), (5, 10, 5, 0.5), (7.5, 7.5, 1, 0.15)],
        "up": [(1, 10, 1, 0.15), (5, 10, 5, 0.5)],
        "down": [(1, 10, 1, 0.15), (5, 10, 5, 0.5)],
        "layout": "hash marks every MOA out to 5, then at 7.5 and 10 on the horizontal "
                  "arms, and every MOA to 10 up and down (measured: 1 MOA long at 5 and "
                  "10, .3 otherwise)",
        "published": "fine crosshair .06 MOA; heavy crosshair (posts) 1 MOA; gap before "
                     "the posts 1 MOA; hash spacing 1 and 2.5 MOA",
    },
    {
        "id": "vortex-ebr-1-mrad-viper-pst-6-24x50-sfp",
        "name": "EBR-1 (MRAD) - Viper PST 6-24x50 SFP",
        "for_scope": "Vortex Viper PST (Gen I) 6-24x50 SFP",
        "mag": 24, "source": sheet("6-24x50 EBR-1 MRAD"), "part": "PST-R624S1-M-12A",
        "unit": "mrad", "reach": (3, 3, 3), "gap": 0.3, "fine": 0.02, "heavy": 0.3,
        "horizontal": [(0.2, 0.8, 0.2, 0.05), (1, 3, 1, 0.12), (1.5, 2.5, 1, 0.05)],
        "up": [(0.2, 3, 0.2, 0.05), (1, 3, 1, 0.12)],
        "down": [(0.2, 3, 0.2, 0.05), (1, 3, 1, 0.12)],
        "layout": "hash marks every .2 mrad out to 1, then at 1.5, 2, 2.5 and 3 on the "
                  "horizontal arms, and every .2 to 3 up and down (measured: .24 mrad "
                  "long at whole mrads, .1 otherwise)",
        "published": "fine crosshair .02 mrad; heavy crosshair (posts) .3 mrad; gap "
                     "before the posts .3 mrad; hash spacing .2 and .5 mrad",
    },
    {
        "id": "vortex-ebr-1-moa-viper-pst-4-16x50-sfp",
        "name": "EBR-1 (MOA) - Viper PST 4-16x50 SFP",
        "for_scope": "Vortex Viper PST (Gen I) 4-16x50 SFP",
        "mag": 16, "source": sheet("4-16x50 EBR-1 MOA"), "part": "PST-R416S1-A-12A",
        "unit": "moa", "reach": (20, 20, 20), "gap": 1.5, "fine": 0.1, "heavy": 1.5,
        "horizontal": [(2, 8, 2, 0.25), (10, 20, 10, 0.75), (15, 15, 1, 0.25)],
        "up": [(2, 20, 2, 0.25), (10, 20, 10, 0.75)],
        "down": [(2, 20, 2, 0.25), (10, 20, 10, 0.75)],
        "layout": "hash marks every 2 MOA out to 10, then at 15 and 20 on the horizontal "
                  "arms, and every 2 MOA to 20 up and down (measured: 1.5 MOA long at 10 "
                  "and 20, .5 otherwise)",
        "published": "fine crosshair .1 MOA; heavy crosshair (posts) 1.5 MOA; gap before "
                     "the posts 1.5 MOA; hash spacing 2 and 5 MOA",
    },
    {
        "id": "vortex-ebr-1-mrad-viper-pst-4-16x50-sfp",
        "name": "EBR-1 (MRAD) - Viper PST 4-16x50 SFP",
        "for_scope": "Vortex Viper PST (Gen I) 4-16x50 SFP",
        "mag": 16, "source": sheet("4-16x50 EBR-1 MRAD"), "part": "PST-R416S1-M-12A",
        "unit": "mrad", "reach": (5, 5, 5), "gap": 0.5, "fine": 0.03, "heavy": 0.5,
        "horizontal": [(0.5, 1.5, 0.5, 0.07), (1, 5, 2, 0.15), (2, 4, 2, 0.25)],
        "up": [(0.5, 5, 0.5, 0.07), (1, 5, 2, 0.15), (2, 4, 2, 0.25)],
        "down": [(0.5, 5, 0.5, 0.07), (1, 5, 2, 0.15), (2, 4, 2, 0.25)],
        "layout": "hash marks at .5, 1, 1.5, then every mrad to 5 on the horizontal "
                  "arms, and every .5 to 5 up and down (measured: .5 mrad long at 2 and "
                  "4, .3 at odd mrads, .14 at the halves)",
        "published": "fine crosshair .03 mrad; heavy crosshair (posts) .5 mrad; gap "
                     "before the posts .5 mrad; hash spacing .5 and 1 mrad",
    },
    {
        "id": "vortex-ebr-1-moa-viper-pst-2-5-10x44",
        "name": "EBR-1 (MOA) - Viper PST 2.5-10x44",
        "for_scope": "Vortex Viper PST (Gen I) 2.5-10x44",
        "mag": 10, "source": sheet("2.5-10x44 EBR-1 MOA"), "part": "PST-R210S1-A-12A",
        **moa_30(0.5, 1.0), "gap": 2.5, "fine": 0.15, "heavy": 2.5,
        "layout": MOA_30_LAYOUT + " (measured: 2 MOA long at 10, 20 and 30, 1 MOA "
                                  "otherwise). The sheet's picture leaves out the fine "
                                  "line from the top edge that every other EBR-1 sheet, "
                                  "the same scope's MRAD sheet included, shows; it is "
                                  "drawn here",
        "published": "fine crosshair .15 MOA; heavy crosshair (posts) 2.5 MOA; gap before "
                     "the posts 2.5 MOA; hash spacing 2 and 5 MOA",
    },
    {
        "id": "vortex-ebr-1-mrad-viper-pst-2-5-10x44",
        "name": "EBR-1 (MRAD) - Viper PST 2.5-10x44",
        "for_scope": "Vortex Viper PST (Gen I) 2.5-10x44",
        "mag": 10, "source": sheet("2.5-10x44 EBR-1 MRAD"), "part": "PST-R210S1-M-12A",
        "unit": "mrad", "reach": (8, 8, 8), "gap": 0.7, "fine": 0.04, "heavy": 0.75,
        "horizontal": [(0.5, 1.5, 0.5, 0.1), (1, 7, 2, 0.2), (2, 8, 2, 0.3)],
        "up": [(0.5, 1.5, 0.5, 0.1), (1, 1, 1, 0.2), (2, 2, 1, 0.3), (8, 8, 1, 0.3)],
        "down": [(0.5, 8, 0.5, 0.1), (1, 7, 2, 0.2), (2, 8, 2, 0.3)],
        "layout": "hash marks at .5, 1, 1.5, then every mrad to 8 on the horizontal "
                  "arms; every .5 to 8 down; up, marks to 2 and an end mark at 8 "
                  "(measured: .6 mrad long at even mrads, .4 at odd ones, .2 at the "
                  "halves)",
        "published": "fine crosshair .04 mrad; heavy crosshair (posts) .75 mrad; gap "
                     "before the posts .7 mrad; hash spacing .5 and 1 mrad",
    },
    {
        "id": "vortex-tmcq-moa-viper-pst-1-4x24",
        "name": "TMCQ (MOA) - Viper PST 1-4x24",
        "for_scope": "Vortex Viper PST (Gen I) 1-4x24",
        "mag": 4, "source": sheet("1-4x24 TMCQ MOA"), "part": "PST-R14ST-A-12A",
        "kind": "tmcq", "unit": "moa", "fine": 0.2, "dot": 1, "line": (2, 30),
        "marks": [(4, 30, 2, 0.8), (10, 30, 10, 2)], "ring": (9, 11, 3.8),
        "layout": "the crosshair starts 2 MOA from the centre and runs to 30 with a mark "
                  "every 2 MOA from 4 (measured: 4 MOA long at 10, 20 and 30, 1.6 "
                  "otherwise); the broken circle's four quarters are cut straight 3.8 "
                  "MOA from each axis (measured). The sheet's picture ends the crosshair at 30 "
                  "MOA; anything farther out isn't known from it and isn't drawn",
        "published": "crosshair .2 MOA; centre dot 1 MOA; broken circle 18 MOA inside "
                     "and 22 MOA outside; hash spacing 2 MOA",
    },
    {
        "id": "vortex-tmcq-mrad-viper-pst-1-4x24",
        "name": "TMCQ (MRAD) - Viper PST 1-4x24",
        "for_scope": "Vortex Viper PST (Gen I) 1-4x24",
        "mag": 4, "source": sheet("1-4x24 TMCQ MRAD"), "part": "PST-R14ST-M-12A",
        "kind": "tmcq", "unit": "mrad", "fine": 0.06, "dot": 0.3, "line": (0.5, 6),
        "marks": [(1, 6, 0.5, 0.2), (2, 6, 2, 0.5)], "ring": (2.5, 3.25, 1),
        "layout": "the crosshair starts .5 mrad from the centre and runs to 6 with a mark "
                  "every .5 mrad from 1 (measured: 1 mrad long at 2, 4 and 6, .4 "
                  "otherwise); the broken circle's four quarters are cut straight 1 mrad "
                  "from each axis (measured). The sheet's picture ends the crosshair at 6 mrad; "
                  "anything farther out isn't known from it and isn't drawn",
        "published": "crosshair .06 mrad; centre dot .3 mrad; broken circle 5 mrad inside "
                     "and 6.5 mrad outside; hash spacing .5 mrad",
    },
]


def reticle_entry(d):
    tmcq_kind = d.get("kind") == "tmcq"
    sources = d["source"] + (f" and {d['also_source']}" if "also_source" in d else "")
    if tmcq_kind:
        plane = (f"SFP: the subtensions are true at {d['mag']}x, as the sheet says. Lit = "
                 "the centre dot and the broken circle (red on Vortex's sheet).")
    else:
        plane = FFP_SOURCE_NOTE if d["mag"] is None else sfp_note(d["mag"])
    notes = (f"Drawn for MX Shooting Calculator from the figures on Vortex's reticle "
             f"sheet {d['part']} (Viper PST, first generation), mirrored at {sources}; "
             f"the layout was read from the sheet's picture, not traced. PUBLISHED and "
             f"used: {d['published']}. MEASURED from the picture against the printed "
             f"numbers: {d['layout']}. {plane}")
    if "picture_posts" in d:
        notes += (f" The sheet's picture draws the posts about {d['picture_posts']} "
                  f"{d['unit']} wide (the same picture as the 6-24x50 FFP's); the "
                  f"printed {d['heavy']} is used.")
    return {
        "manufacturer": "Vortex",
        "name": d["name"],
        "maker_reticle_name": "TMCQ" if tmcq_kind else "EBR-1",
        "for_scope": d["for_scope"],
        "published_at_magnification": d["mag"],
        "unit": d["unit"],
        "source_url": d["source"],
        "notes": notes,
        "id": d["id"],
        "verified_on": VERIFIED,
        "elements": tmcq(d) if tmcq_kind else ebr1(d),
    }


def scope(model, lo, hi, obj, plane, unit, reticle, source, click, click_note):
    focal = "ffp" if plane == "ffp" else "sfp"
    slug = model.lower().replace(".", "-").replace(" ", "-")
    return {
        "manufacturer": "Vortex",
        "model": f"Viper PST Gen I {model} ({unit.upper()})",
        "magnification_min": lo,
        "magnification_max": hi,
        "objective_mm": obj,
        "tube_mm": 30,
        "focal_plane": focal,
        "reticle_reference_magnification": None if focal == "ffp" else hi,
        "reticles": [reticle],
        "click_unit": unit,
        "click_value": click,
        "elevation_travel": None,
        "fov_m_at_100m": None,
        "illuminated": True,
        "source_url": source,
        "notes": (f"First-generation Viper PST (before the Gen II); discontinued. Focal "
                  f"plane, magnification and reticle from Vortex's reticle sheet for "
                  f"this model (linked). {click_note} Travel and field of view not "
                  f"retrieved. 30 mm tube and illumination assumed (not on the sheet; "
                  f"the sheet draws the lit parts of the reticle in red)."),
        "id": f"vortex-viper-pst-i-{slug}-{unit}",
        "verified_on": VERIFIED,
    }


ASSUMED = "Click value assumed from Vortex's standard ({}); not on the sheet."
SCOPES = [
    scope("1-4x24", 1, 4, 24, "sfp", "moa", "vortex-tmcq-moa-viper-pst-1-4x24",
          sheet("1-4x24 TMCQ MOA"), 0.5, "1/2 MOA clicks, as the reticle sheet says."),
    scope("1-4x24", 1, 4, 24, "sfp", "mrad", "vortex-tmcq-mrad-viper-pst-1-4x24",
          sheet("1-4x24 TMCQ MRAD"), 0.2, ".2 mrad clicks, as the reticle sheet says."),
    scope("2.5-10x32 FFP", 2.5, 10, 32, "ffp", "moa", "vortex-ebr-1-moa-viper-pst-ffp",
          sheet("2.5-10x32 FFP EBR-1 MOA"), 0.25, ASSUMED.format("1/4 MOA")),
    scope("2.5-10x32 FFP", 2.5, 10, 32, "ffp", "mrad",
          "vortex-ebr-1-mrad-viper-pst-2-5-10x32-ffp",
          sheet("2.5-10x32 FFP EBR-1 MRAD"), 0.1, ASSUMED.format("0.1 MRAD")),
    scope("2.5-10x44", 2.5, 10, 44, "sfp", "moa", "vortex-ebr-1-moa-viper-pst-2-5-10x44",
          sheet("2.5-10x44 EBR-1 MOA"), 0.25, ASSUMED.format("1/4 MOA")),
    scope("2.5-10x44", 2.5, 10, 44, "sfp", "mrad", "vortex-ebr-1-mrad-viper-pst-2-5-10x44",
          sheet("2.5-10x44 EBR-1 MRAD"), 0.1, ASSUMED.format("0.1 MRAD")),
    scope("4-16x50 FFP", 4, 16, 50, "ffp", "moa", "vortex-ebr-1-moa-viper-pst-ffp",
          "https://308ar.com/wp-content/uploads/2015/08/"
          "Vortex-Viper-PST-4-16x50-FFP-R416F1-A-12A-Reticle.pdf",
          0.25, ASSUMED.format("1/4 MOA")),
    scope("4-16x50 FFP", 4, 16, 50, "ffp", "mrad", "vortex-ebr-1-mrad-viper-pst-4-16x50-ffp",
          sheet("4-16x50 FFP EBR-1 MRAD"), 0.1, ASSUMED.format("0.1 MRAD")),
    scope("4-16x50 SFP", 4, 16, 50, "sfp", "moa", "vortex-ebr-1-moa-viper-pst-4-16x50-sfp",
          sheet("4-16x50 EBR-1 MOA"), 0.25, ASSUMED.format("1/4 MOA")),
    scope("4-16x50 SFP", 4, 16, 50, "sfp", "mrad", "vortex-ebr-1-mrad-viper-pst-4-16x50-sfp",
          sheet("4-16x50 EBR-1 MRAD"), 0.1, ASSUMED.format("0.1 MRAD")),
    scope("6-24x50 FFP", 6, 24, 50, "ffp", "moa", "vortex-ebr-1-moa-viper-pst-6-24x50-ffp",
          sheet("6-24x50 FFP EBR-1 MOA"), 0.25, ASSUMED.format("1/4 MOA")),
    scope("6-24x50 FFP", 6, 24, 50, "ffp", "mrad", "vortex-ebr-1-mrad-viper-pst-6-24x50-ffp",
          sheet("6-24x50 FFP EBR-1 MRAD"), 0.1, ASSUMED.format("0.1 MRAD")),
    scope("6-24x50 SFP", 6, 24, 50, "sfp", "moa", "vortex-ebr-1-moa-viper-pst-6-24x50-sfp",
          sheet("6-24x50 EBR-1 MOA"), 0.25, ASSUMED.format("1/4 MOA")),
    scope("6-24x50 SFP", 6, 24, 50, "sfp", "mrad", "vortex-ebr-1-mrad-viper-pst-6-24x50-sfp",
          sheet("6-24x50 EBR-1 MRAD"), 0.1, ASSUMED.format("0.1 MRAD")),
]


def write_reticles(entries):
    """reticles.json keeps one element per line."""
    out = ["["]
    for n, e in enumerate(entries):
        out.append("{")
        keys = list(e)
        for i, k in enumerate(keys):
            comma = "," if i < len(keys) - 1 else ""
            if k == "elements":
                out.append(' "elements": [')
                els = e[k]
                for j, el in enumerate(els):
                    out.append("   " + json.dumps(el, ensure_ascii=False)
                               + ("," if j < len(els) - 1 else ""))
                out.append(" ]" + comma)
            else:
                out.append(f" {json.dumps(k)}: {json.dumps(e[k], ensure_ascii=False)}{comma}")
        out.append("}" + ("," if n < len(entries) - 1 else ""))
    out.append("]")
    return "\n".join(out) + "\n"


def merged(existing, ours):
    """Our entries replace their old selves where they are; new ones go after
    the maker's last entry."""
    by_id = {e["id"]: e for e in ours}
    out = [by_id.pop(e["id"], e) for e in existing]
    last = max(i for i, e in enumerate(out) if e["manufacturer"] == "Vortex")
    new = [e for e in ours if e["id"] in by_id]
    return out[:last + 1] + new + out[last + 1:]


def main():
    check = "--check" in sys.argv
    reticles_path = CATALOG / "reticles.json"
    scopes_path = CATALOG / "scopes/vortex.json"
    reticles = write_reticles(merged(json.loads(reticles_path.read_text()),
                                     [reticle_entry(d) for d in RETICLES]))
    scopes = json.dumps(merged(json.loads(scopes_path.read_text()), SCOPES),
                        indent=1, ensure_ascii=False) + "\n"
    stale = [p for p, text in ((reticles_path, reticles), (scopes_path, scopes))
             if p.read_text() != text]
    if check:
        if stale:
            sys.exit("out of date: " + ", ".join(str(p) for p in stale))
        return
    for path, text in ((reticles_path, reticles), (scopes_path, scopes)):
        path.write_text(text)
    print(f"{len(RETICLES)} reticles and {len(SCOPES)} scopes written")


if __name__ == "__main__":
    main()
