"""Puts a drawing into the catalog's drawings.json, replacing the one with
the same id (or adding it at the end), keeping the file's layout:

    python3 tools/drawings/put.py drawing.json
"""
import json, os, sys

CATALOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..',
                       'crates', 'ballistics-store', 'catalog', 'drawings.json')


def put(new, path=CATALOG):
    s = open(path).read()
    ids = [x['id'] for x in json.loads(s)]
    c = lambda v: json.dumps(v, ensure_ascii=False, separators=(',', ':'))
    head = ', '.join(f'{json.dumps(k)}: {json.dumps(new[k], ensure_ascii=False)}'
                     for k in ['id', 'name', 'bird', 'shoulder_height_m', 'length_m', 'vital'])
    entry = '  {' + head + ', "parts": [\n' + ',\n'.join('    ' + c(p) for p in new['parts']) + '\n  ]'
    if new.get('zones'):
        entry += ', "zones": [\n' + ',\n'.join('    ' + c(z) for z in new['zones']) + '\n  ]'
    if new.get('rings'):
        entry += ', "rings": ' + c(new['rings'])
    marker = '  {"id": ' + json.dumps(new['id']) + ','
    if marker in s:
        start = s.index(marker)
        nxt = s.find('\n  {"id": ', start + 5)
        if nxt < 0:
            end = s.rindex('}') + 1
            entry += '}'
        else:
            end = nxt + 1
            entry += '},\n'
        s = s[:start] + entry + s[end:]
        action = 'replaced'
    else:
        end = s.rindex('}') + 1
        s = s[:end] + ',\n' + entry + '}' + s[end:]
        ids.append(new['id'])
        action = 'added'
    open(path, 'w').write(s)
    assert [x['id'] for x in json.load(open(path))] == ids, 'the catalog changed in an unexpected way'
    print(action, new['id'])


if __name__ == '__main__':
    data = json.load(open(sys.argv[1]))
    for drawing in (data if isinstance(data, list) else [data]):
        put(drawing)
