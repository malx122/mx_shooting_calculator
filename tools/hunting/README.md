# Hunting seasons and hours

The app's hunting data (`crates/ballistics-store/catalog/hunting/<country>.json`)
is made from the researched law files here:

- `law-sweden.json`: Jaktförordning (1987:905) with its bilagor, Jaktlag
  (1987:259), and Naturvårdsverket's licence rules, read from riksdagen.se and
  svenskforfattningssamling.se.
- `law-finland.json`: Metsästyslaki 615/1993, Metsästysasetus 666/1993 and the
  ministry's (MMM) decrees, read from finlex.fi, cross-checked on riista.fi.

Each file quotes the paragraphs it relies on and lists its sources, what was
uncertain (`unclear`) and the date it was read (`as_of`). Laws are not
protected by copyright in either country (upphovsrättslagen 9 §,
tekijänoikeuslaki 9 §).

To update, after the law or a decree changes (Finland's goose, grouse and seal
decrees run out every year on 31 July):

1. Update the law file: the seasons, the quotes, `as_of`.
2. Run the converters from the repository root:

       python3 tools/hunting/convert_sweden.py tools/hunting/law-sweden.json crates/ballistics-store/catalog/hunting/sweden.json
       python3 tools/hunting/convert_finland.py tools/hunting/law-finland.json crates/ballistics-store/catalog/hunting/finland.json

3. Run the tests (`cargo test -p ballistics-store hunting`): they check that
   every species and region named is known.

Finland's areas are written in words; `convert_finland.py` maps each one to the
regions (maakunnat) it covers, whole or in part, by hand. A new area wording
needs a line there.
