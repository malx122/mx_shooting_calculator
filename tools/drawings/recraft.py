"""Asks Recraft for flat vector pictures of an animal:

    RECRAFT_API_KEY=... python3 tools/drawings/recraft.py "A bull moose ..." --n 3 --out /tmp/moose

The key comes from $RECRAFT_API_KEY or --key-file (a text or .rtf file
holding it); it is never printed. Each picture costs money (about 0.08 USD
for the vector model in 2026); the rights to them follow the plan the key
is on. A prompt that works: "A <animal> standing still, full body in strict
side view profile, facing right, all four legs visible and standing on the
ground, head held level. Flat vector illustration with clean simple shapes
and a few solid colours: <colours>. Plain white background, no ground, no
shadow, no text."""
import argparse, json, os, re, subprocess, sys, urllib.request

API = 'https://external.api.recraft.ai/v1/images/generations'


def key_from(path):
    if path.endswith('.rtf') and sys.platform == 'darwin':
        text = subprocess.run(['textutil', '-convert', 'txt', '-stdout', path], capture_output=True, text=True).stdout
    else:
        text = open(path).read()
    return re.findall(r'[A-Za-z0-9_\-]{20,}', text)[0]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('prompt')
    ap.add_argument('--n', type=int, default=3)
    ap.add_argument('--out', required=True, help='folder for the SVGs')
    ap.add_argument('--model', default='recraftv4_1_vector')
    ap.add_argument('--size', default='3:2')
    ap.add_argument('--colour', action='append', default=[], metavar='R,G,B', help='a preferred colour (repeatable)')
    ap.add_argument('--key-file')
    args = ap.parse_args()
    key = os.environ.get('RECRAFT_API_KEY') or (key_from(args.key_file) if args.key_file else None)
    if not key:
        sys.exit('no key: set RECRAFT_API_KEY or give --key-file')
    body = {"prompt": args.prompt, "model": args.model, "n": args.n, "size": args.size,
            "controls": {"background_color": {"rgb": [255, 255, 255]}}}
    if args.colour:
        body["controls"]["colors"] = [{"rgb": [int(v) for v in c.split(',')]} for c in args.colour]
    req = urllib.request.Request(API, data=json.dumps(body).encode(), method='POST',
                                 headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            res = json.load(r)
    except urllib.error.HTTPError as e:
        sys.exit(f'HTTP {e.code}: {e.read().decode()[:400]}')
    os.makedirs(args.out, exist_ok=True)
    for i, item in enumerate(res['data'], 1):
        path = os.path.join(args.out, f'{i}.svg')
        urllib.request.urlretrieve(item['url'], path)
        print(path)


if __name__ == '__main__':
    main()
