"""Turns the researched Finnish law (law-finland.json) into the app's
catalog/hunting/finland.json. The law names areas in words; AREAS maps
each to the regions (maakunnat) it covers whole or in part."""
import json, sys
src, out = sys.argv[1], sys.argv[2]
d = json.load(open(src))
R = ["Uusimaa", "Varsinais-Suomi", "Satakunta", "Kanta-Häme", "Pirkanmaa", "Päijät-Häme",
     "Kymenlaakso", "Etelä-Karjala", "Etelä-Savo", "Pohjois-Savo", "Pohjois-Karjala",
     "Keski-Suomi", "Etelä-Pohjanmaa", "Pohjanmaa", "Keski-Pohjanmaa", "Pohjois-Pohjanmaa",
     "Kainuu", "Lappi"]
U, VS, Sa, KH, Pi, PH, Ky, EK, ES, PS, PK, KS, EP, Po, KP, PP, Ka, La = R
COAST = [U, VS, Sa, Ky, Po, KP, PP, La]
INLAND = [r for r in R if r not in COAST]
ALL_PART = ("part", R)
# area -> (whole, part), or "elsewhere" (the regions the species' other rows leave).
AREAS = {
    "koko maa": (R, []),
    "Enontekiön, Inarin ja Utsjoen kunnat": ([], [La]),
    "Etelä-Karjala, Etelä-Pohjanmaa, Etelä-Savo, Kainuu, Kanta-Häme, Keski-Pohjanmaa, Kymenlaakso, Pirkanmaa, Pohjanmaa, Pohjois-Karjala, Pohjois-Pohjanmaa, Pohjois-Savo, Päijät-Häme, Satakunta; Lappi lukuun ottamatta Inaria, Kittilää, Kolaria, Muoniota, Pelloa, Sodankylää ja Ylitorniota":
        ([EK, EP, ES, Ka, KH, KP, Ky, Pi, Po, PK, PP, PS, PH, Sa], [La]),
    "Etelä-Karjala, Etelä-Pohjanmaa, Kanta-Häme, Keski-Pohjanmaa, Keski-Suomi, Kymenlaakso, Pirkanmaa, Pohjanmaa, Päijät-Häme, Satakunta; Kainuu lukuun ottamatta Puolankaa ja Suomussalmea; Lappi lukuun ottamatta Inaria, Kittilää, Kolaria, Muoniota ja Sodankylää; Pohjois-Pohjanmaa lukuun ottamatta Kuusamoa, Pudasjärveä ja Taivalkoskea":
        ([EK, EP, KH, KP, KS, Ky, Pi, Po, PH, Sa], [Ka, La, PP]),
    "Etelä-Karjala, Keski-Suomi, Uusimaa, Varsinais-Suomi; Enontekiö, Inari, Kemijärvi, Kittilä, Kolari, Muonio, Pelkosenniemi, Posio, Salla, Savukoski, Sodankylä, Utsjoki":
        ([EK, KS, U, VS], [La]),
    # Joroinen and Varkaus: Pohjois-Savo; Iitti and Orimattila: Päijät-Häme.
    "Etelä-Karjala, Kymenlaakso; Enonkoski, Hirvensalmi, Juva, Mikkeli, Mäntyharju, Puumala, Rantasalmi, Savonlinna, Sulkava; Heinävesi, Joensuu, Kitee, Kontiolahti, Liperi, Rääkkylä, Tohmajärvi; Joroinen, Varkaus; Iitti, Orimattila; Askola, Lapinjärvi, Loviisa, Myrskylä, Porvoo, Pukkila":
        ([EK, Ky], [ES, PK, PS, PH, U]),
    "Etelä-Karjalan, Etelä-Savon, Kainuun, Kanta-Hämeen, Keski-Suomen, Pirkanmaan, Pohjois-Karjalan, Pohjois-Savon ja Päijät-Hämeen maakunnat; Etelä-Pohjanmaalla Alajärvi, Alavus, Evijärvi, Kuortane, Lappajärvi, Soini, Vimpeli ja Ähtäri; Keski-Pohjanmaalla Halsua, Lestijärvi, Perho ja Veteli; Lapin maakunta lukuun ottamatta Kemiä, Keminmaata, Torniota ja Simoa; Kymenlaaksossa Kouvola; Pohjois-Pohjanmaalla Haapajärvi, Haapavesi, Kuusamo, Kärsämäki, Nivala, Pudasjärvi, Pyhäjärvi, Pyhäntä, Reisjärvi, Siikalatva, Taivalkoski, Utajärvi ja Vaala":
        ([EK, ES, Ka, KH, KS, Pi, PK, PS, PH], [EP, KP, La, Ky, PP]),
    "Etelä-Pohjanmaa, Etelä-Savo, Kainuu, Kanta-Häme, Keski-Pohjanmaa, Pirkanmaa, Pohjanmaa, Pohjois-Karjala, Pohjois-Pohjanmaa, Pohjois-Savo, Päijät-Häme, Satakunta; Kemi, Keminmaa, Pello, Ranua, Rovaniemi, Simo, Tervola, Tornio, Ylitornio":
        ([EP, ES, Ka, KH, KP, Pi, Po, PK, PP, PS, PH, Sa], [La]),
    # Puolanka, Suomussalmi: Kainuu; Kuusamo, Pudasjärvi, Taivalkoski: Pohjois-Pohjanmaa.
    "Etelä-Savon, Pohjois-Savon ja Pohjois-Karjalan maakunnat; Puolanka, Suomussalmi, Inari, Kittilä, Kolari, Muonio, Sodankylä, Kuusamo, Pudasjärvi, Taivalkoski":
        ([ES, PS, PK], [Ka, La, PP]),
    "Ilomantsi": ([], [PK]),
    "Inari, Kittilä, Kolari, Muonio, Pello, Sodankylä, Ylitornio": ([], [La]),
    "Kainuun ja Pohjois-Pohjanmaan maakuntien alue lukuun ottamatta Kuusamon ja Taivalkosken kuntia": ([Ka], [PP]),
    # Juuka, Lieksa, Nurmes: Pohjois-Karjala; Rautavaara, Sonkajärvi: Pohjois-Savo.
    "Kainuun, Lapin ja Pohjois-Pohjanmaan maakunnat; Juuka, Lieksa, Nurmes, Rautavaara, Sonkajärvi; ei kuitenkaan Enontekiö, Inari, Utsjoki eikä valtatien E8 merenpuoleinen alue (paitsi seututien 813 ja valtatien E8 välinen alue)":
        ([Ka], [La, PP, PK, PS]),
    "Keski-Suomen ja Uudenmaan maakunnat": ([KS, U], []),
    "Kuusamon ja Taivalkosken kunnat sekä Lapin maakunta": ([La], [PP]),
    "Kymenlaakson maakunta": ([Ky], []),
    "Lapin maakunta": ([La], []),
    "Lounais-Suomen ja Suomenlahden kannanhoitoalueet": ([], [U, VS, Sa, Ky]),
    "Perämeren–Merenkurkun kannanhoitoalue (Pohjanmaan, Keski-Pohjanmaan, Pohjois-Pohjanmaan ja Lapin maakuntaan kuuluvat merialueet)": ([], [Po, KP, PP, La]),
    "Perämeren–Merenkurkun, Lounais-Suomen ja Suomenlahden kannanhoitoalueet (merialueet)": ([], COAST),
    "Pohjanmaan, Etelä-Pohjanmaan, Keski-Pohjanmaan ja Pohjois-Pohjanmaan maakunnat": ([Po, EP, KP, PP], []),
    "Pohjois-Karjala lukuun ottamatta Lieksaa, Nurmesta ja Juukaa; Puolanka, Suomussalmi, Inari, Kittilä, Kolari, Muonio, Sodankylä, Kuusamo, Pudasjärvi, Taivalkoski":
        ([], [PK, Ka, La, PP]),
    "Pohjois-Pohjanmaan, Kainuun ja Lapin maakunnat": ([PP, Ka, La], []),
    "Pohjois-Savo, Pohjois-Karjala, Etelä-Pohjanmaa, Keski-Pohjanmaa, Pohjanmaa, Keski-Suomi, Pohjois-Pohjanmaa, Kainuu ja Lappi lukuun ottamatta Enontekiötä, Inaria ja Utsjokea":
        ([PS, PK, EP, KP, Po, KS, PP, Ka], [La]),
    "Pohjois-Savon ja Pohjois-Karjalan maakunnat": ([PS, PK], []),
    "Pohjois-Savon, Pohjois-Karjalan, Pohjois-Pohjanmaan, Kainuun ja Lapin maakunnat": ([PS, PK, PP, Ka, La], []),
    "Uudenmaan maakunta": ([U], []),
    "Varsinais-Suomen maakunta": ([VS], []),
    "enintään 300 metrin etäisyydellä kalastuslain 4 §:n 15 ja 16 kohdassa tarkoitetuista pyydyksistä": ([], R),
    "ilmailulain 75 §:n 1 kohdassa tarkoitetut lentoasemat": ([], R),
    "kiintiöalueet poronhoitoalueen ulkopuolella": ([], R),
    "liitteenä olevaan karttaan erikseen merkitty alue (outer archipelago, boundary islands listed in the annex)": ([], COAST),
    "merialue sekä merialueella sijaitsevat saaret ja luodot": ([], COAST),
    "muu Pohjois-Pohjanmaan maakunta sekä Kainuun maakunta": ([Ka], [PP]),
    "muu maa (inland)": (INLAND, COAST),
    "muualla": "elsewhere",
    "muualla kuin 741/2026 1 §:ssä luetellulla alueella": "elsewhere",
    "muualla kuin Lapin maakunnassa": ([r for r in R if r != La], []),
    "muualla maassa": "elsewhere",
    "muualla maassa lukuun ottamatta Lapin maakuntaa": "elsewhere-but-lapland",
    "pelto, jonka sato on korjaamatta": ([], R),
    # The reindeer herding area: most of Lappi and the north of Pohjois-Pohjanmaa and Kainuu.
    "poronhoitoalue": ([], [La, PP, Ka]),
    "poronhoitoalue (itäinen ja läntinen)": ([], [La, PP, Ka]),
}

def rows(species_rows):
    result = []
    named = [AREAS[r["area"]] for r in species_rows
             if not isinstance(AREAS[r["area"]], str) and r["area"] != "koko maa"]
    covered_whole = {x for whole, _ in named for x in whole}
    covered_part = {x for _, part in named for x in part}
    for r in species_rows:
        area = AREAS[r["area"]]
        if isinstance(area, str):
            whole = [x for x in R if x not in covered_whole and x not in covered_part]
            part = [x for x in R if x in covered_part and x not in covered_whole]
            if area == "elsewhere-but-lapland":
                whole = [x for x in whole if x != La]
                part = [x for x in part if x != La]
        else:
            whole, part = area
        row = {"area": r["area"], "regions": list(whole), "part_of": list(part),
               "from": r["from"], "to": r["to"], "applies_to": r["applies_to"]}
        for key in ("note", "from_time", "to_time"):
            if r.get(key):
                row[key] = r[key]
        result.append(row)
    return result

def time(t):
    return {"relative_to": t["relative_to"], "minutes": t["minutes"]}

daily, notes = [], []
for r in d["daily_rules"]:
    s, e = r.get("start"), r.get("end")
    if s and e and s.get("relative_to") in ("sunrise", "sunset") and e.get("relative_to") in ("sunrise", "sunset"):
        rule = {"species": r["species"], "rule": r["rule"], "start": time(s), "end": time(e),
                "law": r["law"], "quote": r["quote"], "url": r["url"]}
        for key in ("valid_from", "valid_until"):
            if r.get(key):
                rule[key] = r[key]
        daily.append(rule)
    else:
        notes.append({"species": r["species"], "text": r["rule"], "law": r["law"],
                      "quote": r.get("quote", ""), "url": r.get("url", "")})
if d.get("no_daily_time_limit"):
    ntl = d["no_daily_time_limit"]
    notes.append({"species": ntl["species"], "text": "Finnish law sets no time of day for "
                  "hunting this game: only the season's dates (and for ducks and geese the noon "
                  "start on 20 August and the waterfowl twilight ban).",
                  "law": ntl.get("law", "Metsästyslaki 615/1993, Metsästysasetus 666/1993"),
                  "quote": "", "url": ntl.get("url", "")})

seasons = []
for s in d["seasons"]:
    season = {"species": s["species"], "rows": rows(s["rows"]),
              "licence_only": bool(s.get("protected")),
              "permit_needed": bool(s.get("licence")) and not s.get("protected")}
    for key in ("note", "law", "url"):
        if s.get(key):
            season[key] = s[key]
    if s.get("temporary_decree"):
        decree = s["temporary_decree"]
        season["decree"] = {"name": decree["name"], "valid_until": decree["valid_until"][:10]}
    seasons.append(season)

law = {
    "as_of": d["as_of"],
    "version": "Metsästyslaki 615/1993 and Metsästysasetus 666/1993 as consolidated on Finlex "
               + d["as_of"] + ", with the ministry's decrees for the hunting year 2026–2027 (1 August–31 July).",
    "regions": R,
    "utc_offset_hours": 2.0,
    "sources": [{"name": src_["name"], "url": src_["url"]} for src_ in d["sources"]],
    "daily": daily,
    "notes": notes,
    "seasons": seasons,
}
json.dump(law, open(out, "w"), ensure_ascii=False, indent=1)
print(len(daily), "daily rules,", len(notes), "notes,", len(seasons), "species")
