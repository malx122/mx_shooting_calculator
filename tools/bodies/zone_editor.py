"""The editor for where each 3D body stands against its drawing's hit area.

    python3 tools/bodies/zone_editor.py [port]

Serves the editor at http://127.0.0.1:8091/ (this computer only). Pick an
animal in the list; it shows the body straight from the side with the hit
area and rings on it, and the drawing with its hit area beside it (drop a
picture of the training figure there too: it stays on the computer). Drag
the hit area onto the body and scale it, then press Accept: the placement
goes into the body's "placement" in
crates/ballistics-store/catalog/bodies.json, the catalog body test runs in
the background, and the next animal still to place opens. An animal with a
placement counts as accepted; a new fit (add.py) removes it.

The hit area keeps its size and place against the aim point in the app:
moving or scaling it here moves and scales the body under it.
Needs numpy and pillow.
"""
import base64, http.server, io, json, math, pathlib, subprocess, sys, threading, urllib.parse

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import add  # noqa: E402  (writes bodies.json one body a line)

ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOG = ROOT / "crates/ballistics-store/catalog"
SUPER = 2  # supersampling for smooth edges


def png(img):
    buffer = io.BytesIO()
    img.save(buffer, "PNG", optimize=True)
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


def frame(points, ppm, margin=0.15):
    lo, hi = points.min(0), points.max(0)
    x0, y1 = lo[0] - margin, hi[1] + margin
    size = (int((hi[0] - lo[0] + 2 * margin) * ppm), int((hi[1] - lo[1] + 2 * margin) * ppm))
    return x0, y1, size


def side_view(body, ppm):
    """The body from its near side, flat-shaded, and how its pixels map to
    metres."""
    V = np.array(body["vertices"], float)
    F = np.array(body["triangles"])
    palette = [tuple(int(c[i:i + 2], 16) for i in (1, 3, 5)) for c in body["colours"]]
    x0, y1, size = frame(V[:, :2], ppm)
    img = Image.new("RGB", (size[0] * SUPER, size[1] * SUPER), (201, 210, 190))
    draw = ImageDraw.Draw(img)
    tri = V[F]
    normal = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
    normal /= np.linalg.norm(normal, axis=1, keepdims=True) + 1e-12
    light = np.array([-0.4, 0.8, 0.6]) / np.linalg.norm([-0.4, 0.8, 0.6])
    for i in np.argsort(tri[:, :, 2].mean(1)):
        if normal[i, 2] <= 0:
            continue
        shade = 0.55 + 0.45 * max(0.0, float(normal[i] @ light))
        colour = tuple(int(c * shade) for c in palette[body["face_colour"][i]])
        draw.polygon([((x - x0) * ppm * SUPER, (y1 - y) * ppm * SUPER) for x, y, _ in tri[i]], fill=colour)
    img = img.resize(size, Image.LANCZOS)
    antler = set(body.get("antler_faces", []))
    keep = [i for i in range(len(F)) if i not in antler]
    kept = V[np.unique(F[keep])][:, 1]
    return img, x0, y1, float(kept.max()), float(kept.min())


def drawing_view(drawing, ppm, circle=None):
    pts = np.array([p for part in drawing["parts"] for p in part["points"]])
    x0, y1, size = frame(pts, ppm)
    img = Image.new("RGBA", (size[0] * SUPER, size[1] * SUPER), (201, 210, 190, 255))
    draw = ImageDraw.Draw(img, "RGBA")
    at = lambda x, y: ((x - x0) * ppm * SUPER, (y1 - y) * ppm * SUPER)
    for part in drawing["parts"]:
        draw.polygon([at(x, y) for x, y in part["points"]], fill=part["fill"])
    for zone in drawing.get("zones", [])[::-1]:
        draw.polygon([at(x, y) for x, y in zone["points"]], fill=(110, 170, 110, 140), outline=(240, 236, 220, 255))
    if circle:
        cx, cy = at(*drawing["vital"])
        r = circle / 2 * ppm * SUPER
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(110, 170, 110, 140), outline=(240, 236, 220, 255), width=SUPER)
    rings = drawing.get("rings")
    if rings:
        cx, cy = at(*rings["center"])
        for ring in rings["rings"]:
            r = ring["diameter_m"] / 2 * ppm * SUPER
            draw.ellipse([cx - r, cy - r, cx + r, cy + r], outline=(240, 236, 220, 255), width=SUPER)
    return img.resize(size, Image.LANCZOS)


def data(id_, bodies, drawings, targets):
    """Everything the page needs for one animal."""
    body = next(b for b in bodies if b["id"] == id_)
    drawing = next(d for d in drawings if d["id"] == id_)
    ppm = 1400 / max(np.ptp(np.array(body["vertices"])[:, 0]), 0.1)
    side, x0, y1, top, bottom = side_view(body, ppm)
    # Without a hit area of its own, the app draws a vital circle at the aim
    # point, as big as the target says.
    circle = None if drawing.get("zones") else next(
        (t["vital_diameter_cm"] / 100 for t in targets if t.get("silhouette") == id_), None)
    outline = np.array([p for part in drawing["parts"] for p in part["points"]])
    reference = drawing_view(drawing, 420 / max(np.ptp(outline[:, 0]), 0.1), circle)
    return {
        "id": id_,
        "name": drawing["name"],
        "side": png(side),
        "x0": x0, "y1": y1, "ppm": ppm,
        "top": top,
        "bottom": bottom,
        "antlers": bool(body.get("antler_faces")),
        "reference": png(reference),
        "vital": drawing["vital"],
        "zones": [z["points"] for z in drawing.get("zones", [])],
        "rings": drawing.get("rings"),
        "circle": circle,
        "placement": body.get("placement", {"offset": [0, 0], "scale": 1}),
    }


def load(name):
    return json.loads((CATALOG / name).read_text())


class Check:
    """The catalog body test, run in the background after each Accept (one
    at a time; Accepts during a run start one more run after it)."""

    def __init__(self):
        self.lock = threading.Lock()
        self.state, self.output, self.again, self.running = "none", "", False, False

    def start(self):
        with self.lock:
            if self.running:
                self.again = True
                return
            self.running, self.state = True, "running"
        threading.Thread(target=self.run, daemon=True).start()

    def run(self):
        state, output = "failed", ""
        try:
            while True:
                done = subprocess.run(["cargo", "test", "-q", "-p", "ballistics-store", "catalog_bodies"],
                                      cwd=ROOT, capture_output=True, text=True)
                with self.lock:
                    if not self.again:
                        break
                    self.again = False
            state = "passed" if done.returncode == 0 else "failed"
            output = (done.stdout + done.stderr)[-3000:]
        except Exception as e:  # cargo missing, and the like: say so, don't hang
            output = repr(e)
        finally:
            with self.lock:
                self.running, self.state, self.output = False, state, output

    def report(self):
        with self.lock:
            return {"state": self.state, "output": self.output}


CHECK = Check()
SAVING = threading.Lock()


def accept(id_, placement):
    offset, scale = placement["offset"], float(placement["scale"])
    offset = [float(v) for v in offset]
    if len(offset) != 2 or not all(map(math.isfinite, offset + [scale])) or not 0.6 <= scale < 1.5:
        raise ValueError("a placement needs two finite offsets and a scale from 0.6 to 1.5")
    with SAVING:
        bodies = load("bodies.json")
        body = next((b for b in bodies if b["id"] == id_), None)
        if body is None:
            raise ValueError(f"no body {id_!r}")
        body["placement"] = {"offset": offset, "scale": scale}
        add.save(bodies)
    CHECK.start()


class Editor(http.server.BaseHTTPRequestHandler):
    def send(self, status, body, kind="application/json"):
        raw = body.encode() if isinstance(body, str) else body
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(raw)

    def local(self):
        # Only for pages this server gave out: a site that points its own
        # name at 127.0.0.1 still sends that name as the Host.
        host = self.headers.get("Host", "")
        if host in (f"127.0.0.1:{self.server.server_port}", f"localhost:{self.server.server_port}"):
            return True
        self.send(403, "{}")
        return False

    def do_GET(self):
        if not self.local():
            return
        try:
            self.get(urllib.parse.unquote(urllib.parse.urlsplit(self.path).path))
        except Exception as e:  # say what went wrong rather than drop the connection
            self.send(500, json.dumps({"error": repr(e)}))

    def get(self, path):
        if path == "/":
            self.send(200, pathlib.Path(__file__).with_name("zone-editor.html").read_text(), "text/html; charset=utf-8")
        elif path == "/animals":
            names = {d["id"]: d["name"] for d in load("drawings.json")}
            self.send(200, json.dumps([{"id": b["id"], "name": names.get(b["id"], b["id"]), "accepted": "placement" in b}
                                       for b in load("bodies.json")]))
        elif path.startswith("/animal/"):
            id_ = path[len("/animal/"):]
            try:
                self.send(200, json.dumps(data(id_, load("bodies.json"), load("drawings.json"), load("targets.json"))))
            except StopIteration:
                self.send(404, json.dumps({"error": f"no body {id_!r}"}))
        elif path == "/check":
            self.send(200, json.dumps(CHECK.report()))
        else:
            self.send(404, "{}")

    def do_POST(self):
        # Only from the editor's own page: a form on another site can't send
        # JSON with this header without the browser asking first.
        if not self.local():
            return
        path = urllib.parse.unquote(urllib.parse.urlsplit(self.path).path)
        if not path.startswith("/accept/") or self.headers.get("Content-Type") != "application/json":
            return self.send(400, "{}")
        try:
            placement = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
            accept(path[len("/accept/"):], placement)
        except (StopIteration, KeyError, TypeError, ValueError) as e:
            return self.send(400, json.dumps({"error": repr(e)}))
        except Exception as e:
            return self.send(500, json.dumps({"error": repr(e)}))
        self.send(200, "{}")

    def log_message(self, *args):
        pass


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8091
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Editor)
    print(f"the hit-area editor: http://127.0.0.1:{port}/  (Ctrl-C to stop)")
    server.serve_forever()


if __name__ == "__main__":
    main()
