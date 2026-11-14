# Animal bodies in 3D

`crates/ballistics-store/catalog/bodies.json` holds a 3D body for some of the
animal drawings, shown on the target's page, in the 3D trajectory view and
through the scope. Each is a closed triangle mesh in the drawing's metres
(x to the head, y up, z towards the viewer, the head turned that way), one
colour per triangle, with its antler triangles listed.

The bodies come from Meshy (low-poly, textured GLB) and are fitted with
these scripts, run in a Python venv with numpy, scipy, pillow, trimesh,
rtree, scikit-image and fast-simplification. Adding an animal is two steps:

    python3 tools/bodies/add.py model.glb <drawing-id> [--no-legs]
    python3 tools/bodies/add.py --place <drawing-id> '{"placement":{...}}'

The first fits the model, puts it into `bodies.json` and writes the
hit-area editor to `target/bodies/<id>-editor.html`; open it in a browser,
move and scale the hit area onto the body, and give the second the
placement it shows. Each step runs only
`cargo test -p ballistics-store catalog_bodies`, which checks the mesh is
closed and outward, its placement between 0.6 and 1.5 times the fit, and the hit
area (or the aim point) on the body. Nothing else needs building or testing
until the change is committed.

- `fit.py` closes small holes (or, for a model of many loose pieces such as
  feathers, rebuilds it as one closed surface), turns the model, stands it
  on the ground, scales and moves it so its side view covers the drawing
  (antlers left out), then, unless `--no-legs`, lines up the front legs so
  the drawing's hit area lands behind the shoulder. Triangle colours come
  from the texture, cut to a palette. Check the `-overlay.png` it writes.
- `antlers.py` marks the antler triangles when the drawing has antlers:
  thin and high on the head, grown from the top. Check the picture it
  writes: the ears must stay brown.
- `zone_editor.py` writes the editor; `look.py` renders a model before
  fitting it.

Generate on a paid Meshy plan (the project's is Pro): its terms leave the
model to the project, while free-plan models are CC BY 4.0 and need a
credit. Don't publish the models to Meshy's community, which its terms say
would give that up. Note each model's origin in `NOTICE`.
