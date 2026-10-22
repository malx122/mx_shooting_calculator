# Animal drawings

How the catalog's animal drawings and hit zones are made
(`crates/ballistics-store/catalog/drawings.json`). The moose was the first
done this way (October 2026).

1. **A picture.** Generate a flat vector picture, or draw one: a strict side
   view, facing right, standing, plain white background, a few solid fills.
   `recraft.py` asks Recraft for it (the key from `$RECRAFT_API_KEY` or
   `--key-file`; the rights follow the plan the key is on, so note the
   origin in `NOTICE`):

       python3 tools/drawings/recraft.py "A bull moose standing still, ..." --n 3 --out /tmp/moose \
           --colour 67,59,50 --colour 169,159,142 --colour 212,185,142

2. **The hit zone.** Open `tools/hitzone-editor.html` in a browser, load a
   picture of the training figure (it stays on the computer), click its
   ring centre and a point on its outer ring to calibrate, and draw the
   zone's spline. *Download JSON* gives the points in cm; put them, with the
   rings' sizes and scores, in a zone file like `zones/moose-sjf.json`.

3. **Convert and put into the catalog.** `svg2drawing.py` turns the SVG into
   polygons in metres (gaps cut out, hooves added, the vital point placed as
   on the SJF figure) and adds the zone and rings around the vital point:

       python3 tools/drawings/svg2drawing.py /tmp/moose/1.svg --id moose --name "Moose (bull)" \
           --shoulder 1.87 --zone tools/drawings/zones/moose-sjf.json --write

   `--recolour`, `--leg`, `--antler`, `--body` and `--vital` override what
   it works out from the fills. `put.py` alone replaces a drawing from a
   JSON file. Then `cargo test -p ballistics-store` and
   `cargo test -p mxsc` check the drawing, and the Targets page shows it.

`raster.py` holds the pure-Python raster tools these use (bitset rows,
morphology, contour tracing, simplification).
