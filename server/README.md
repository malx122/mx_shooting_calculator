# Sync server

A Cloudflare Worker with a D1 (SQLite) database that keeps one copy of a
hunter's data per sync key, so their devices can sync. Live at
<https://mxsc-sync.malx122.workers.dev>.

The app encrypts the copy before sending it (XChaCha20-Poly1305 with a key
derived from the sync key) and stores it under a SHA-256 hash of the sync
key. The server never sees the key or anything readable, and keeps no
accounts. See `crates/ballistics-store/src/sync.rs` for the client side.

It also signs the app in to Google for syncing through the hunter's own
Google Drive (`src/google.js`). The app's Google client is a web client,
whose code exchange needs the client secret, so that happens here. The
tokens wait under a pick-up code the server makes, which goes back only to
the app that started the sign-in: to a port the desktop app listens on
(`127.0.0.1`), or to the web app's own `google.html`, which hands it to the
app's window and shows it to type in. A link someone else made therefore
gets them nothing. Tokens wait at most ten minutes, and one no app picked up
is deleted at the next sign-in request or the quarter-hourly clean-up.

And it passes the hunter's rifles on to a Garmin watch paired with the app
(`src/watch.js`; see `docs/watch.md`). To pair, the watch sends its public
key and shows the code it gets back, and keeps a token; the app, given the
code, sends its own public key, a writer secret and the profiles, each
encrypted to the watch's key. The watch fetches them with its token, asks
the hunter, and says how it went. A pairing it took stays as the link: the
app replaces the profiles with its writer, and the watch fetches them with
its token when it opens and says which version it has. The server keeps the
two public keys, SHA-256 hashes of the token and the writer (not the secrets
themselves), the encrypted profiles and their version, what the watch says
it has, and when the link was made and last used: nothing it could read. A
pairing the watch didn't take is deleted after ten minutes (when the next
pairing is made, or by the quarter-hourly clean-up: within half an hour at
most); a link, when either side forgets it or after 400 days unused.

## API

| Request | Answer |
|---|---|
| `GET /v1/copies/{id}` | `200` with the bytes and an `ETag` (the version), `404` no copy yet, `410` deleted |
| `PUT /v1/copies/{id}` with `If-None-Match: *` | `201`: the first copy; `412` if one exists |
| `PUT /v1/copies/{id}` with `If-Match: "<version>"` | `200` with the new `ETag`; `412` if another device wrote first |
| `DELETE /v1/copies/{id}` | `204`; later reads and writes get `410`, so no device starts it again |
| `GET /v1/google/start?session=S&reply=R` | redirects to Google's sign-in (S: 32 hex digits the app made; R: the desktop app's port, or `w0`–`w2` for the web app's sites) |
| `GET /v1/google/callback` | Google's redirect; sends the browser back to the app with S and a new pick-up code P |
| `POST /v1/google/session/P` | the tokens, once; `404` for an unknown, used or expired code |
| `POST /v1/google/refresh` | `{"refresh_token"}` → `{"access_token", "expires_in"}`; `401` to sign in again |
| `POST /v1/watch/pairings` | `{"key"}` (the watch's) → `200` `{"code", "token", "expires_in"}` |
| `GET /v1/watch/pairings/{code}` | `200` `{"key"}` (the watch's) waiting for the app; `409` the app has sent; `410` the watch has answered; `404` unknown or expired |
| `PUT /v1/watch/pairings/{code}` | `{"key", "writer", "version", "items"}` (the app's key) → `201`; `404` unknown or expired; `409` someone sent first; `410` the watch has answered; `507` full |
| `GET /v1/watch/link` (token) | `200` `{"waiting": true}` nothing sent yet; `200` `{"key", "version", "count"}`; `404` unknown, expired or declined |
| `GET /v1/watch/link/items/{i}` (token) | `200` `{"version", "box"}`; `404` |
| `POST /v1/watch/link/done` (token) | `{"done", "version"}` → `200` `{"taken": true}` when the link stands, else `{}` (also when unknown); `400` for a body that isn't JSON. To a pairing, `done` is `used`, `declined`, `unreadable` or `left` (anything else counts as `left`) and the first one stays; on a link, `used` or `unreadable` for the latest fetched |
| `GET /v1/watch/link/status` (writer) | `200` `{"done", "version", "watch_version", "watch_said"}`; `404` unknown, expired or forgotten |
| `PUT /v1/watch/link` (writer) | `{"version", "items"}` → `200` `{}`; `404` unknown; `409` not newer, or not taken yet |
| `DELETE /v1/watch/link` (token or writer) | `200` `{}`, also when unknown |

`{id}` is 64 lowercase hex digits. Copies are at most 1 MiB. CORS is open
to every origin (the copy is encrypted and the id is unguessable).

For the watch, everything is JSON, errors too (`{"error"}`), and the watch's
successes are all `200`: Garmin Connect relays other codes and empty bodies
unreliably. The token and the writer go as `Authorization: Bearer <64
lowercase hex digits>`; without one, `401`. A pairing lives ten minutes from
the watch's `POST` and ten more from the app's `PUT`, and the same `PUT`
sent again gets `201` again. Binary fields are in padded base64: a key is 64
bytes (a P-256 point, X then Y), `items` 1 to 16 boxes of at most 12 KiB
each, `{code}` 8 digits, `version` a whole number from 1 to 2³¹ − 1. A bad
body, key, writer, version or item gets `400`; a bad code or unknown secret,
`404` (but `DELETE` always answers `200`, and `POST .../done` too for a
JSON body).

One IP address may start 2 copies a minute, make 20 Google sign-in requests,
start 10 watch pairings, look up or send to 30 watch codes (which slows
anyone guessing codes) and make 120 requests in all (Cloudflare's rate
limiter, in `wrangler.toml`; nothing is stored), else `429`. When the
database reaches `MAX_DATABASE_BYTES` (about 80 % of the plan's size: raise
it on Workers Paid), new copies and new watch links get `507`, and the ones
already there keep going. A job every quarter of an hour forgets Google
sign-ins nobody picked up, watch pairings past their ten minutes, watch
links unused for 400 days, copies nobody has written for two years, and
deleted ones after 30 days.

## Develop

Needs Node 22.

```sh
cd server
npm install
npm test        # runs the worker locally (wrangler dev) and checks the API
npm run dev     # the worker on http://localhost:8787 with a local database
```

To try the app against a local server, build it with
`MXSC_SYNC_URL=http://127.0.0.1:8787 scripts/web.sh build`.

## Deploy

Release tags deploy it through `.github/workflows/sync-server.yml`, which
needs these repository secrets:

- `CLOUDFLARE_ACCOUNT_ID`: shown by `npx wrangler whoami`;
- `CLOUDFLARE_API_TOKEN`: a Cloudflare API token with **Workers Scripts:
  Edit** and **D1: Edit** for the account;
- `GOOGLE_WEB_CLIENT_SECRET`: the Google web client's secret, set as the
  worker's `GOOGLE_CLIENT_SECRET` (by hand:
  `npx wrangler secret put GOOGLE_CLIENT_SECRET`).

The Google client (Google Cloud Console → APIs & Services) needs the
**Google Drive API** enabled, the redirect URI
`https://mxsc-sync.malx122.workers.dev/v1/google/callback`, and the scope
`.../auth/drive.appdata` on its consent screen (the only one asked for: not
even `openid` or `email`). While the consent screen is
in testing, only its test users can sign in.

By hand, after `npx wrangler login`: `npm run deploy` (applies new database
migrations, then deploys). The database was made once with
`npx wrangler d1 create mxsc-sync`; its id is in `wrangler.toml`.
