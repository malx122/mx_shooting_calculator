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
whose code exchange needs the client secret, so that happens here; tokens
wait at most ten minutes for the app to pick them up, and the server keeps
none after that.

## API

| Request | Answer |
|---|---|
| `GET /v1/copies/{id}` | `200` with the bytes and an `ETag` (the version), `404` no copy yet, `410` deleted |
| `PUT /v1/copies/{id}` with `If-None-Match: *` | `201`: the first copy; `412` if one exists |
| `PUT /v1/copies/{id}` with `If-Match: "<version>"` | `200` with the new `ETag`; `412` if another device wrote first |
| `DELETE /v1/copies/{id}` | `204`; later reads and writes get `410`, so no device starts it again |
| `GET /v1/google/start?session=S` | redirects to Google's sign-in (S: 32 hex digits the app made) |
| `GET /v1/google/callback` | Google's redirect; the tokens wait under S |
| `POST /v1/google/session/S` | the tokens, once; `404` until signed in |
| `POST /v1/google/refresh` | `{"refresh_token"}` → `{"access_token", "expires_in"}`; `401` to sign in again |

`{id}` is 64 lowercase hex digits. Copies are at most 256 KiB. CORS is open
to every origin (the copy is encrypted and the id is unguessable). A weekly
job forgets copies nobody has written for two years, and deleted ones after
30 days.

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
