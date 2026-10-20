"""Turns the researched Swedish law (law-sweden.json) into the app's
catalog/hunting/sweden.json."""
import json, sys
src, out = sys.argv[1], sys.argv[2]
d = json.load(open(src))
COUNTIES = ["Stockholms län", "Uppsala län", "Södermanlands län", "Östergötlands län",
    "Jönköpings län", "Kronobergs län", "Kalmar län", "Gotlands län", "Blekinge län",
    "Skåne län", "Hallands län", "Västra Götalands län", "Värmlands län", "Örebro län",
    "Västmanlands län", "Dalarnas län", "Gävleborgs län", "Västernorrlands län",
    "Jämtlands län", "Västerbottens län", "Norrbottens län"]

def time(t):
    return {"relative_to": t["relative_to"], "minutes": t["minutes"]}

daily, notes = [], []
for r in d["daily_rules"]:
    s, e = r.get("start"), r.get("end")
    if s and e and "relative_to" in s and "relative_to" in e:
        rule = {"species": r["species"], "rule": r["rule"], "start": time(s), "end": time(e),
                "law": r["law"], "quote": r["quote"], "url": r["url"]}
        last = r.get("last_part_only")
        if last:
            rule["restricted"] = {"from": time(last["from"]), "to": time(last["to"]),
                                  "text": "only stalking or watching (smyg- eller vaktjakt)"}
        daily.append(rule)
    else:
        notes.append({"species": r["species"], "text": r["rule"], "law": r["law"],
                      "quote": r.get("quote", ""), "url": r.get("url", "")})

def rows(species_rows):
    partial = {c for r in species_rows if r.get("subarea") for c in r["counties"] if c in COUNTIES}
    result = []
    for r in species_rows:
        resolved = r.get("counties_resolved")
        named = [c for c in r["counties"] if c in COUNTIES]
        if r.get("subarea"):
            regions, part_of = [], resolved or named
        elif resolved is not None:
            regions = resolved
            # "The rest of the country" also takes in what other rows leave
            # of the counties they cover in part.
            part_of = sorted(partial - set(resolved), key=COUNTIES.index) if not named else []
        else:
            regions, part_of = named, []
        row = {"area": r["area"], "regions": regions, "part_of": part_of,
               "from": r["from"], "to": "end-of-02" if r.get("leap_year_to") else r["to"],
               "applies_to": r["applies_to"]}
        for key in ("note", "method", "from_time"):
            if r.get(key):
                row[key] = r[key]
        if r.get("periods"):
            row["periods"] = [p for p in r["periods"] if p.get("method") and p["method"] != "any"]
        bad = [c for c in regions + part_of if c not in COUNTIES]
        assert not bad, (r["area"], bad)
        result.append(row)
    return result

def name(url):
    for key, title in [
        ("jaktforordning-1987905", "Jaktförordning (1987:905), riksdagen.se"),
        ("jaktlag-1987259", "Jaktlag (1987:259), riksdagen.se"),
        ("SFS2026-1693.pdf", "SFS 2026:1693, official PDF"),
        ("20261693", "SFS 2026:1693 (new seasons from 1 July 2027)"),
        ("nfs-2022-4", "NFS 2022:4, Naturvårdsverket (licence hunting)"),
        ("saljakten", "Naturvårdsverket: the 2026 seal hunts"),
        ("bjornjakten", "Naturvårdsverket: the 2026 bear hunt"),
        ("andringar-av-jakttider", "Naturvårdsverket: proposed changes to the seasons"),
        ("forslag-pa-jakttider", "Naturvårdsverket: proposed seasons"),
        ("paragraf9", "Svenska Jägareförbundet: 9 § (time of day)"),
        ("nya-jakttider", "Regeringen: new hunting seasons decided (2026)"),
    ]:
        if key in url:
            return title
    return url

seasons = []
for s in d["seasons"]:
    season = {"species": s["species"], "rows": rows(s["rows"]), "licence_only": s["licence_only"]}
    if s.get("note"):
        season["note"] = s["note"]
    if s.get("rows_from_2027_07_01"):
        season["change"] = {"from": "2027-07-01", "rows": rows(s["rows_from_2027_07_01"])}
        if s.get("future_note"):
            season["change"]["note"] = s["future_note"]
    seasons.append(season)

law = {
    "as_of": d["as_of"],
    "version": d["source_amended_note"],
    "regions": COUNTIES,
    "utc_offset_hours": 1.0,
    "sources": [{"name": name(u), "url": u} for u in d["sources"]],
    "daily": daily,
    "notes": notes,
    "seasons": seasons,
}
json.dump(law, open(out, "w"), ensure_ascii=False, indent=1)
print(len(daily), "daily rules,", len(notes), "notes,", len(seasons), "species")
