# Watch app: the hunter's rifles on a Garmin watch

The watch app (`watch/`, Monkey C for Garmin Connect IQ, first target the
epix (Gen 2)) shows the clicks and centimetres for a range, a wind and the
weather. It doesn't solve the flight itself: the phone or desktop app works
out a **profile** for every rifle and load, and sends them to the watch
through the app's sync server. On the watch the hunter chooses the rifle and
sets the range, the wind and the temperature; the watch reads the air
pressure from its own barometer.

The watch **pairs** with one device once. After that, that device sends the
profiles again whenever what they are made from changes (a rifle, a load,
the units, the target's minimum energy), while the app is open; the watch
picks them up when it opens.

## Pairing

1. On the watch: **Pair with phone**. The watch makes a P-256 key pair,
   sends the public key to the sync server, and shows the 8-digit **code**
   the server gives back, and keeps the secret **token** that came with it.
2. In the app: **Garmin watch**, type the code. The app fetches the watch's
   public key, makes its own key pair and a secret **writer**, encrypts each
   profile for the watch and sends them, its public key and the writer
   under the code.
3. The watch, which asks every few seconds, fetches them with its token,
   checks and decrypts each, and lists the rifles: **Use these rifles?** On
   yes it tells the server, and only once the server answers that it has
   taken the pairing does the watch keep the rifles, its key pair, the token
   and the app's public key; the pairing is now a **link**. On no (or when
   the box couldn't be read) it tells the server so. A watch that leaves
   early says so too.
4. The app, which asks about the link every few seconds, hears how it went.
   It keeps its key pair, the writer and the watch's public key in a
   setting on that device only (never synced or exported).

## Updates

The app fingerprints what the profiles are made from (the rifles, bullets
and loads, the target's minimum energy and velocity, whether spin drift and
the jump are counted, the units and the farthest range); a few seconds after
it changes, the app makes the profiles again and sends them as a newer
**version**, encrypted with the same keys. When MX Shooting opens on the
watch, or on **Update from phone**, the watch asks whether there is a newer
version, fetches it, checks that it comes from the app's key it kept, and
uses it without asking. It tells the server which version it has; the app
shows whether the watch has the latest.

Either side can **forget** the link (the app: **Forget this watch**; the
watch: **Forget the phone**), which deletes it on the server as soon as the
server hears; until it does, the app (while open) and the watch (each time
it opens) keep trying. The watch keeps the rifles it has. A link neither
side has used for 400 days is deleted.

A rifle and load that can't be worked out is left out (the app says which
and why); the others still go. If sending fails, the app tries the same
profiles again after a minute, then waits twice as long each time, up to
half an hour.

**What the server holds.** For a link: the two public keys, the hashes of
the token and the writer, the encrypted profiles and their version, what
the watch said it has, and when it was made and last used. It could read
none of the profiles, though it sees how many there are, their sizes (which
follow the range they go out to) and when they change. A pairing the watch didn't take is deleted after ten
minutes (when the next pairing is made, or by the quarter-hourly clean-up:
within half an hour at most).

**What the encryption protects against.** The server stores nothing it
could read, and anyone watching the traffic sees nothing readable. When
pairing, the two public keys are not checked against each other: a server
that set out to deceive could hand each side a key of its own. That is why
the watch lists the rifles and asks before it pairs, and why the app says to
check them. After that the keys are fixed: only the paired app can make
profiles the watch takes, and only that watch can read them. The watch only
takes a version newer than the one it has, so an old one can't be played
back to it. Someone who guessed a live code could pair a watch with rifles
of their own; the code lookups are slowed per IP address, the real app then
gets "already sent", and the watch still asks first.

The watch's requests go through Garmin Connect on the hunter's phone, as
all Connect IQ web requests do.

## Server API

All bodies are JSON. Binary fields are standard base64 with padding.
Requests marked *token* or *writer* carry it as `Authorization: Bearer
<64 hex digits>`.

| Request | Answer |
|---|---|
| `POST /v1/watch/pairings` `{"key"}` | `200` `{"code", "token", "expires_in"}` |
| `GET /v1/watch/pairings/{code}` | `200` `{"key"}` waiting for the app; `409` the app has sent; `410` the watch has answered; `404` unknown or expired |
| `PUT /v1/watch/pairings/{code}` `{"key", "writer", "version", "items"}` | `201`; `404` unknown or expired; `409` already sent; `410` the watch has answered; `507` the server is full |
| `GET /v1/watch/link` *token* | `200` `{"waiting": true}` nothing sent yet; `200` `{"key", "version", "count"}`; `404` unknown, expired or declined |
| `GET /v1/watch/link/items/{i}` *token* | `200` `{"version", "box"}`, `i` from 0; `404` |
| `POST /v1/watch/link/done` *token* `{"done", "version"}` | `200` `{"taken": true}` when the link stands; else `200` `{}`, also when unknown; `400` a body that isn't JSON |
| `GET /v1/watch/link/status` *writer* | `200` `{"done", "version", "watch_version", "watch_said"}`; `404` unknown, expired, or forgotten |
| `PUT /v1/watch/link` *writer* `{"version", "items"}` | `200` `{}`; `404` unknown; `409` not newer than the version there, or the watch hasn't taken the pairing yet |
| `DELETE /v1/watch/link` *token or writer* | `200` `{}`, also when unknown |

On the link requests, a missing or malformed `Authorization` is `401`, so a
watch never takes it for a link that has gone (`404`). The same `PUT` to a
pairing sent again (an answer lost on the way) gets `201` again.

`done`: to a pairing, `used` (the hunter took the rifles), `declined` (the
hunter said no), `unreadable` (a box didn't open or parse) or `left` (the
watch stopped waiting; also anything else, and `used` before anything was
sent). The first answer stays. A pairing taken becomes the link and its code
is freed, and the watch, told `{"taken": true}`, keeps it; one that has run
out first answers `{}`, and the watch then keeps nothing; one declined, unreadable or left loses the app's key and the
items at once, and the rest after the ten minutes. Afterwards, on a link,
`used` with a version says the watch has that version, and `unreadable` that
it couldn't use the latest; `watch_version` and `watch_said` give the last
of these to the app.

The watch only ever gets `200` with a JSON body for success: Garmin Connect
relays other success codes and empty bodies unreliably. Errors are JSON
too (`{"error"}`).

- `key` in the `POST`: the watch's public key; in the `PUT` and the link:
  the app's. 64 bytes: X then Y, each 32 bytes big-endian (SEC1 uncompressed
  without the leading `0x04`).
- `code`: 8 decimal digits, unique among live pairings. Shown on the watch
  as `1234 5678`; the app accepts it with or without spaces.
- `token`, `writer`: 64 lowercase hex digits (32 random bytes); the server
  keeps only their SHA-256. The server makes the token, the app the writer.
- `version`: a whole number from 1 to 2³¹ − 1 (what a watch's numbers
  hold), larger with every update (the app uses the Unix time in seconds, or
  one more than the last).
- `items`: 1 to 16 boxes (below), one per profile, in order; each at most
  12 KiB decoded. Kept in separate answers so none is too large for a watch.
- A pairing lives 10 minutes (`expires_in`: seconds) from the watch's
  `POST`, and 10 minutes again from the app's `PUT`, for the hunter to
  answer; unless taken.
- All of a watch's profiles together are at most 96 KiB (the app checks);
  each is kept in the watch's storage as one value.
- Rate limits per IP address (`429` past them): 10 new pairings a minute;
  30 requests a minute by code (`GET` and `PUT` on `/pairings/{code}`), to
  slow anyone guessing codes; and the server's general 120 requests a
  minute. The watch retries its `POST .../done` and `DELETE` on `429` too.
  While pairing, the watch asks every 3 seconds at most, the app every 5.

## Encryption

- `shared`: the ECDH shared secret between the watch's key pair and the
  app's, the X coordinate, 32 bytes big-endian.
- `enc_key = HMAC-SHA256(key = shared, "mxsc watch v1 enc")` (32 bytes).
- `mac_key = HMAC-SHA256(key = shared, "mxsc watch v1 mac")` (32 bytes).
- `box = iv (16) || ciphertext || tag (32)`, where
  - `ciphertext = AES-256-CBC(enc_key, iv, profile with PKCS#7 padding)`,
    `iv` random for every box;
  - `tag = HMAC-SHA256(mac_key, watch_key (64) || app_key (64) || iv ||
    ciphertext)`.

The watch checks the tag before it decrypts, and refuses a box if it
doesn't match or the padding is wrong. Each profile names its bundle's
version, its place and the bundle's size, and the watch checks all three, so
boxes can't be mixed between bundles or places.

Connect IQ keeps P-256 numbers little-endian: the watch reverses each
32-byte half of a public key (X, Y) on the way out and in, and the shared
secret it gets back, so the wire is big-endian as above.

## The watch profile (format 2)

Little-endian throughout. `f32` is an IEEE 754 single.

| Bytes | Field |
|---|---|
| 1 | format, `2` |
| 4 | the bundle's version, `u32` (at most 2³¹ − 1) |
| 1 | this profile's place in the bundle, from 0 |
| 1 | how many profiles the bundle has, 1 to 16 |
| 8 | id: the first 8 bytes of SHA-256 of the rifle's id, `/`, the bullet's id (the watch keeps the hunter's choice across updates by it) |
| 1 | click unit: `0` mil, `1` MOA |
| 4 | click value, `f32`, in the click unit |
| 1 | range unit for the watch: `0` m, `1` yd |
| 1 | wind unit: `0` m/s, `1` km/h, `2` mph |
| 1 | short length unit: `0` cm, `1` in |
| 1 | temperature unit: `0` °C, `1` °F |
| 1 | pressure unit: `0` hPa, `1` mmHg, `2` inHg |
| 2 | `step`: metres between rows, `u16` |
| 2 | `n`: rows, `u16`, 4 to 120 |
| 1 | `nt`: grid temperatures, 2 to 8 |
| nt | the temperatures, °C, `i8`, rising |
| 1 | `np`: grid pressures, 2 to 8 |
| 2 × np | the pressures where the hunter stands, hPa, `u16`, rising |
| 4 × 8 | each column's scale, `f32`, mrad per step |
| 2 × nt × np | per cell (temperatures outer, pressures inner): the ethical limit, m, `u16`; all `0` for none (no minimum energy); `3000` for 3000 m or more |
| 1 | name length in bytes, at most 64 |
| … | name, UTF-8 (rifle and load) |
| 16 × n × nt × np | the values |

The app sends a grid of −30, −8, 13 and 35 °C and 700, 875 and 1050 hPa
(sea level to about 3000 m) and a row every 25 m. The values are `i16`, in
steps of their column's scale; for each cell (temperatures outer, pressures
inner), for each row, eight columns, all in milliradians. Row `i` is at
`(i + 1) × step` metres (the first one step from the muzzle).

1. **elevation** to dial up in calm air (negative: down);
2. **windage** to dial right in calm air (the spin drift);
3. **jump**: more elevation per 1 m/s of wind from the right (3 o'clock);
4. **drift**: more windage per 1 m/s of wind from the right;
5. more elevation per 1 m/s of wind head-on (12 o'clock);
6. more windage per 1 m/s of wind head-on;
7. more elevation per (1 m/s across × 1 m/s head-on): how a wind from
   between differs from its two parts alone;
8. the same for windage.

Each cell is solved in its weather (the cartridges at the air's temperature,
humidity 50 %, level, without cant or Coriolis); the wind columns are the
differences 4 m/s winds from 3, from 12 and from half past one make.

## On the watch

For range `r` (metres), a wind of `s` m/s from clock `c`, air temperature
`T` (°C) and pressure `P` (hPa):

- hold `T` and `P` within the grid (the watch says when they are past it);
- each grid temperature's weight is its Lagrange basis polynomial through
  all the grid's temperatures at `T`; the same for the pressures at `P`;
- along the rows: `x = r ÷ step − 1` (no answer below 0 or past `n − 1`),
  `k = clamp(floor(x) − 1, 0, n − 4)`, and rows `k` to `k + 3` weighted by
  their Lagrange polynomials at `x`;
- each column is the weighted sum of `value × (row + 1) × step` over the
  cells and the four rows, divided by `r` (the curve goes through the
  centimetres, which bend less than the angles near the muzzle);
- crosswind `x = s × sin(c × 30°)`, head wind `h = s × cos(c × 30°)`;
- with `C1` … `C8` the columns: elevation = `C1 + C3 x + C5 h + C7 x h`;
  windage = `C2 + C4 x + C6 h + C8 x h`;
- the ethical limit is straight lines between the cells around `T` and `P`;
- clicks = round(angle ÷ click value), the angle in the click unit (1 MOA =
  π/10800 rad);
- centimetres at the target = angle (mrad) × `r` ÷ 10.

Against solving each shot this loses about 0.01 mil out to 600 m, and 0.2 %
in strong winds (the wind columns are slopes over a 4 m/s wind); the app's
tests check it for every demo rifle.
