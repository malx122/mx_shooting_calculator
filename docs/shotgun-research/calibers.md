# Shotgun calibers (gauge/chamber) for the built-in catalog: sources and gaps

Researched 2026-11-21. The data is in `shotgun_calibers.json`, next to this file: 20 entries,
the 16 asked for plus 12/67, 16/67, 28/76 and .410/65.

## How the fields were filled

- **chamber_mm**: the C.I.P. minimum chamber length L (tolerance +2.00 mm), e.g. 69.90 for
  12/70. The C.I.P. maximum case length l is 0.1-0.2 mm shorter (69.80 −2.50 for 12/70); both
  are in each entry's `notes`.
- **bore_mm / bore_range_mm**: the C.I.P. bore diameter B, which is a minimum, and B plus its
  tolerance: 10: 19.30 +0.70, 12: 18.20 +0.70, 16: 16.80 +0.50, 20: 15.70 +0.50, 24: 14.70 +0.50,
  28: 13.80 +0.50, 32: 12.70 +0.50, .410: 10.20 +0.50. The SAAMI bore figures are in `notes`
  (12 ga .725 +.020 in = 18.42-18.92 mm). Since 2017 C.I.P. also accepts oversize bores if the
  bore is engraved on the barrel, e.g. "12/76-19,3" (decision XXXIV-17/18).
- **pmax_bar**: C.I.P. Pmax (transducer). **pmax_magnum_bar**: the C.I.P. high-performance limit
  of 1050 bar. C.I.P. says it "is valid for all calibres", but only for guns with superior proof
  (1320 bar). Lengths of 73 mm or more (12/76, 12/89, 10/89, 20/76, 28/76, .410/76) already have
  Pmax 1050.
- **steel_standard / steel_hp**: C.I.P. decision XXXII-45 (2014), which is in force.
  - The **standard** class covers only gauges 12, 16 and 20 with cases shorter than 73 mm.
    Every other gauge or length gets `null`.
  - **High-performance** limits exist only for the lengths C.I.P. lists: 10/89, 12/70, 12/73,
    12/76, 12/89, 16/70, 20/70, 20/76, 28/70, 28/76 and .410/76.
  - High-performance steel has **no maximum shot size**. Larger shot is allowed only with a
    choke of 0.5 mm or less. So `shot_max_mm` is `null`, and I added one key,
    **`shot_max_any_choke_mm`**: the largest shot allowed with any choke. Velocity and momentum
    are measured at 2.5 m (V2.5, M2.5).
- **nordic_use**: a judgement. No source gives shares by gauge for Sweden or Finland (see Gaps).

### Key numbers

| Class | Gauge/length | v max (m/s) | Momentum max (N·s) | Shot |
|---|---|---|---|---|
| Standard | 12, < 73 mm | 425 | 12.0 | ≤ 3.25 mm (+2 %) |
| Standard | 16, < 73 mm | 390 | 9.5 | ≤ 3.00 mm (+2 %) |
| Standard | 20, < 73 mm | 410 | 9.3 | ≤ 3.00 mm (+2 %) |
| HP | 10/89 | 440 | 19.0 | > 4 mm: choke ≤ 0.5 |
| HP | 12/70 | 430 | 13.5 | > 4 mm: choke ≤ 0.5 |
| HP | 12/73, 12/76 | 430 | 15.0 | > 4 mm: choke ≤ 0.5 |
| HP | 12/89 | 430 | 19.0 | > 4 mm: choke ≤ 0.5 |
| HP | 16/70 | 420 | 12.0 | > 3.5 mm: choke ≤ 0.5 |
| HP | 20/70 | 410 | 11.0 | > 3.25 mm: choke ≤ 0.5 |
| HP | 20/76 | 430 | 12.0 | > 3.25 mm: choke ≤ 0.5 |
| HP | 28/70 | 400 | 7.0 | > 3 mm: choke ≤ 0.5 |
| HP | 28/76 | 430 | 8.5 | > 3 mm: choke ≤ 0.5 |
| HP | .410/76 | 430 | 4.2 | > 2.5 mm: choke ≤ 0.5 |

Proof pressures:
- Standard proof: 930 bar (12), 980 bar (16), 1040 bar (20 and smaller).
- Superior proof: 1320 bar.
- Steel proof: 1320 bar with hard steel balls of 4.6 mm (10/12), 3.8 mm (16/20/28) or 2.8 mm
  (.410), at a minimum momentum (e.g. 15 N·s for 12/70 and 17.5 N·s for 12/76).

## Questions asked

**Is the shell length measured fired, after opening?** Yes. The nominal length is the length of
the opened case:
- SAAMI drawings give the cartridge as "2.760 −.100 in (70.10 mm) UNCRIMPED" against a minimum
  chamber of 2.750 in. The loaded 12 ga 2¾" shell is only 2.405 in (folded crimp) or 2.450 in
  (rolled crimp).
- RWS: loaded shells are about 6 mm (roll crimp) or 11 mm (star crimp) shorter than the nominal
  case length.
- C.I.P. gives the case maximum l just under the chamber minimum L. It does not state "after
  firing" in words.

**Shorter in longer, never longer in shorter?** Confirmed by SAAMI's interchangeability
guidance: firing 2¾" shells in 3" or 3½" chambers "is acceptable. The reverse is NOT TRUE", because
the longer shell opens into the forcing cone and raises pressure.

There is one C.I.P. exception the other way: lead-loaded cases of 67.5 mm may be fired in 65 mm
chambers. That is the reason the 12/67 and 16/67 lengths exist.

Two more points for the app:
- A gun chambered 12/76 can fire 12/70 standard steel. High-performance steel still needs the
  fleur-de-lys steel proof.
- Different gauges are never interchangeable (the "12/20 burst").

**Naming:**
- Swedish shops (varuste.net/sv) use categories like "12/70 normalpatroner" and "12/76 Magnum",
  and "Kaliber 12" as the gauge. Naturvårdsverket writes "kaliber 12/76".
- Finnish shops (Kärkkäinen) write "Kaliiperi: 12/76". The guides of Motonet and varuste.net say
  "70, 76 (ns. Magnum) ja 89 (ns. Supermagnum)".
- Writing "kal. 12/70" is common, but I did not capture a shop page that shows it, so it is
  unverified.
- ".410" is also sold as "36", mainly for the shorter shells, e.g. Fiocchi "36/63,5". The C.I.P.
  sheet gives "Alternative Names: 36".

**Swedish law:** confirmed. NFS 2023:8 3 kap. 4 § reads: "Tillåtna kalibrar för hagelgevär är
12, 16, 20, 24, 28, 32 eller .410." It says nothing about chamber or shell length. Related rules:
- 5 § requires barrels of at least 48 cm.
- 12 § limits shot to 4.1 mm (US no. 1). So high-performance steel larger than 4.1 mm is not
  legal for hunting in Sweden anyway.
- 14 § limits 24, 28, 32 and .410 to listed small game.
- 10 gauge is not allowed.

**Finland:** metsästysasetus 17 § (as amended by 703/2019):
- gauges 10 to 20 are allowed for game in general;
- gauges smaller than 20 only for the listed small game;
- nothing smaller than 36 (.410).

So 10 gauge is legal in Finland but not in Sweden.

## Sources

C.I.P. (primary):
- TDCC Table VII list: http://bobp.cip-bobp.org/en/tdcc_public?page=1&cartridge_type_id=7
  (also gives PK = 1.15 Pmax and PE = 1.25 Pmax, plus the 67.5 mm / 65 mm notice)
- Calibre sheets:
  - 12: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/12-55-en.pdf (rev. 2018-10-17)
  - 20: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabvii-20-en.pdf (rev. 2020-11-10)
  - 28: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabvii-28-eng-2.pdf (rev. 2017-05-17)
  - .410: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/410-en-2.pdf (rev. 2016-05-18)
  - 10: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page3.pdf
  - 16: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page6.pdf (rev. 2010-05-26)
  - 24: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page8.pdf
  - 32: http://bobp.cip-bobp.org/uploads/tdcc/tab-vii/tabviical-en-page10.pdf
- Proof barrels and measuring points (CR 4): http://bobp.cip-bobp.org/uploads/annexe/annexeiii-en-cr4.pdf
- Lead-free shot and steel, decision XXXII-45 (2014), as published in the Spanish BOE:
  https://www.boe.es/boe/dias/2015/09/09/pdfs/BOE-A-2015-9720.pdf
- The earlier rule, decision XXIX (2007), with 20 gauge at 390 m/s and 2.60 mm:
  https://www.boe.es/boe/dias/2012/04/26/pdfs/BOE-A-2012-5524.pdf
- Oversize bore marking, decisions XXXIV-17/18: https://www.boe.es/boe/dias/2020/02/17/pdfs/BOE-A-2020-2289.pdf
- A Saint-Étienne proof-house test report applying the rules (markings, the 3.25/3.00/3.00 mm limits):
  https://bobp.cip-bobp.org/uploads/homologation/rapport-2019-16-ctc-44-83-sp_5cb5f49080bc2.pdf
- Members and proof houses: https://cip-bobp.org/en/membres and
  https://bobp.cip-bobp.org/uploads/list_proofhouse.pdf. Finland is a member, with "The Proof
  House of Finland", Ministry of the Interior, Riihimäki. Sweden is not a member and has no
  proof house.
- The 2003 Table VII, with PE = 1.30 Pmax and superior proof at 1370 bar:
  https://www.tiropratico.com/CIP/Tabelle%20CIP/caccia_tiro_press.pdf

SAAMI:
- ANSI/SAAMI Z299.2 (2019, approved 2025):
  https://saami.org/wp-content/uploads/2025/09/ANSI-SAAMI-Z299.2-Shotshell-2019-Approved-2025-09-23.pdf
  - MAP: 10 ga 3½" 11,000 psi; 12 ga 2¾" and 3" 11,500; 12 ga 3½" 14,000; 16 ga 11,500;
    20 ga 12,000; 28 ga 12,500; .410 2½" 12,500; .410 3" 13,500.
  - SAAMI has no 24 or 32 gauge, and no 65 mm lengths except .410 2½".
- Interchangeability:
  https://saami.org/wp-content/uploads/2024/11/SAAMI-Generally-Accepted-Firearms-and-Ammunition-Interchangeability-10-8-2021.pdf

Proof houses:
- British Proof Authority equivalence table (GWCT copy), with the high-performance choke limits
  by bore: https://www.gwct.org.uk/media/1106151/Proofing.pdf
- Beschussamt München, 2017. Its 20 gauge figures are outdated:
  https://www.lmg.bayern.de/mam/informationen/beschusswesen/mue_info_stahlschrot.pdf
- Beschussamt Ulm, 2018, choke and shot limits per chamber incl. 10/89, 28/76, .410/76:
  https://ljv-sh.de/wp-content/uploads/VF_504_M_Info-Verwendung-Bleifreie-Schrote_19-02-2017.pdf
- Beschussamt Ulm, 20 gauge sheet, 2023:
  https://rpt.baden-wuerttemberg.de/fileadmin/RP-Internet/Tuebingen/Abteilung_10/_DocumentLibraries/Dokumente_Beschusswesen/Stahlschrot_Kaliber_20.pdf

Makers:
- Eley Hawk, proof and C.I.P.: https://eleyhawkltd.com/insight/proof-and-cip. Old crusher
  figures of 650/900 bar correspond to the transducer values 740/1050; old proof marks are 850
  and 1200 bar.
- RWS glossary, case length and crimps: https://rws-ammunition.com/en/infotainment/glossary/C/case-length
- Fiocchi, "36/63,5" naming: https://fiocchi.com/en/870883.html

Law and Nordic use:
- NFS 2023:8: https://www.naturvardsverket.se/4ace7f/globalassets/nfs/2023/nfs-2023-8.pdf
- Naturvårdsverket, "Jaga blyfritt med hagel":
  https://www.naturvardsverket.se/vagledning-och-stod/jakt-och-vilt/jaga-blyfritt-med-hagel/
- Finnish decree 703/2019, amending 666/1993 17 §: https://data.finlex.fi/api/media/statute/40667/mainPdf/main.pdf
- varuste.net, choosing shotgun shells: https://varuste.net/haulikonpatruunan-valinta
- Motonet guide: https://www.motonet.fi/artikkelit/haulikon-patruunoiden-valintaopas

## Gaps and cautions

- **Nordic use ratings are judgement, not data.** The sources only say that:
  - 12 is "vår vanligaste hagelkaliber" and 12/70 is common (Naturvårdsverket);
  - 12, 16 and 20 are the commonest gauges and 28 and .410 are specialities (varuste.net);
  - 70, 76 and 89 mm are today's lengths and 60/65 mm are found in old guns.

  The ratings for 12/65, 16/65, 16/67, 20/65 and 12/89 are the least certain. I found no Swedish
  or Finnish statistics on gauges or shell sales.
- **Naturvårdsverket says high-performance guns are "trycktestade till 1370 bar".** That is the
  old C.I.P. superior proof (1.30 × 1050). The current value is 1320 bar (1.25 × Pmax).
- **The Munich proof-house sheet (2017) still shows the 2007 rule** for 20 gauge standard steel
  (390 m/s, 2.60 mm). The data uses the 2014 decision: 410 m/s and 3.00 mm.
- **Eley's extract of the British Proof Authority table** gives 20 gauge ≤ 70 mm standard steel at
  ≤ 410 m/s and ≤ 3.00 mm. That agrees with the 2014 decision.
- **C.I.P. defines no standard-steel class for 10, 24, 28, 32 or .410**, and no high-performance
  class for 65/67 mm lengths, 24 or 32 gauge, .410 below 76 mm, or 16/76 and 20/89. Those are
  `null`. It does not mean steel is forbidden. The ammunition simply has no C.I.P. steel limits.
- **The steel-proof minimum momenta** (e.g. 15 N·s for 12/70) are proof-test values, not limits
  for factory loads. They are only in the notes above, not in the JSON.
- **Don't ship the C.I.P. sheets themselves.** The calibre sheets on the C.I.P. site are marked
  "Reproduction forbidden". The figures are taken from them as facts. The steel decision was
  read in its public BOE copy.
- **Maker pages (Beretta, Browning) added nothing primary.** Retailer specs only confirm 76 mm
  chambers on 12 and 20 gauge Beretta over/unders. Fiocchi, RWS and Eley were used as cited
  above.
- **No Swedish proof house exists, and the Finnish one has no public steel-shot page** that I
  could find.
