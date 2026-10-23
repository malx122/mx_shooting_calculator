// Signing in to Google for sync through Google Drive.
//
// The app's Google client is a web client, whose code exchange needs the
// client secret, so it happens here. The app never sees the secret, and the
// server keeps no tokens beyond a few minutes' hand-over:
//
//   GET  /v1/google/start?session=S&reply=R
//                                     the app opens this in a browser; it
//                                     goes on to Google's sign-in
//   GET  /v1/google/callback          Google comes back here; the tokens
//                                     wait under a new pick-up code P,
//                                     which goes back to the app only
//   POST /v1/google/session/P         the app picks them up, once
//   POST /v1/google/refresh           {refresh_token} -> a new access token
//
// S is a random 32-hex-digit id the app makes for each sign-in, so it knows
// the answer is its own. R says how P reaches the app, so that only the
// device that started the sign-in gets it, not whoever made the link:
//
//   R = a port      the desktop app listens on 127.0.0.1 at that port; the
//                   browser is sent there with S and P
//   R = w0, w1, w2  the web app at RETURNS[n] started it; the browser is
//                   sent to that page on the app's own site, with S and P
//                   in the fragment, and it hands them to the app's window
//
// That page also shows P, to type into the app if it can't reach the app's
// window (an app installed on a phone's home screen).
//
// Signing out only forgets the tokens on the device: revoking them at
// Google would sign out every device. The hunter can remove the app's
// access in their Google account.

// Only the app's own hidden folder in the hunter's Drive: not even who they are.
const SCOPES = 'https://www.googleapis.com/auth/drive.appdata';
const SESSION = /^[0-9a-f]{32}$/;
const STATE = /^([0-9a-f]{32})\.(\d{4,5}|w[0-2])$/;
// Crockford's base 32, without letters that look like digits.
const CODE_LETTERS = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const CODE = /^[0-9A-HJKMNP-TV-Z]{10}$/;
const HAND_OVER_MS = 10 * 60 * 1000;
// The web app's page for the hand-back: published, and locally while
// developing (crates/app/google.html).
const RETURNS = [
  'https://malx122.github.io/mx_shooting_calculator/google.html',
  'http://127.0.0.1:8080/google.html',
  'http://localhost:8080/google.html',
];

export async function google(request, env, url, reply, json) {
  const path = url.pathname;
  if (!env.GOOGLE_CLIENT_ID || !env.GOOGLE_CLIENT_SECRET) {
    return reply(503, 'Google sign-in is not set up on this server');
  }
  // Hand-overs nobody picked up go at once, not only at the next clean-up.
  await forgetStale(env);
  if (path === '/v1/google/start' && request.method === 'GET') {
    const session = url.searchParams.get('session') ?? '';
    if (!SESSION.test(session)) return reply(400, 'bad session');
    const state = `${session}.${url.searchParams.get('reply') ?? ''}`;
    if (!replyTo(state)) {
      return page(400, 'Update the app', 'This version of MX Shooting Calculator can no longer sign in to Google. Update it, then try again.');
    }
    const auth = new URL('https://accounts.google.com/o/oauth2/v2/auth');
    auth.search = new URLSearchParams({
      client_id: env.GOOGLE_CLIENT_ID,
      redirect_uri: callback(url),
      response_type: 'code',
      scope: SCOPES,
      access_type: 'offline',
      // Ask again, so Google hands out a refresh token every time.
      prompt: 'consent select_account',
      state,
    }).toString();
    return Response.redirect(auth.toString(), 302);
  }
  if (path === '/v1/google/callback' && request.method === 'GET') {
    const to = replyTo(url.searchParams.get('state') ?? '');
    const code = url.searchParams.get('code');
    if (!to || !code) {
      return page(400, 'Sign-in was cancelled', 'You can close this window and try again in the app.');
    }
    const tokens = await exchange(env, {
      grant_type: 'authorization_code',
      code,
      redirect_uri: callback(url),
    });
    if (!tokens.refresh_token) {
      const why = tokens.error ? ` (Google said: ${escape(tokens.error)})` : '';
      return page(502, 'Sign-in failed', `Google sent no lasting sign-in${why}. Try again from the app.`);
    }
    const handOver = {
      refresh_token: tokens.refresh_token,
      access_token: tokens.access_token,
      expires_in: tokens.expires_in,
      // What the hunter allowed: Google lets them leave Drive unticked.
      scope: tokens.scope,
    };
    const pickUp = newCode();
    await env.DB.prepare('INSERT INTO google_sessions (id, tokens, created_at) VALUES (?1, ?2, ?3)')
      .bind(pickUp, JSON.stringify(handOver), Date.now())
      .run();
    return handBack(to, pickUp);
  }
  const pickUp = /^\/v1\/google\/session\/([0-9A-Z]{10})$/.exec(path);
  if (pickUp && request.method === 'POST') {
    if (!CODE.test(pickUp[1])) return reply(404, 'no such sign-in');
    const row = await env.DB.prepare('DELETE FROM google_sessions WHERE id = ?1 AND created_at > ?2 RETURNING tokens')
      .bind(pickUp[1], Date.now() - HAND_OVER_MS)
      .first();
    return row ? json(200, JSON.parse(row.tokens)) : reply(404, 'no such sign-in');
  }
  if (path === '/v1/google/refresh' && request.method === 'POST') {
    const { refresh_token } = await request.json().catch(() => ({}));
    if (typeof refresh_token !== 'string' || !refresh_token) return reply(400, 'no refresh_token');
    const tokens = await exchange(env, { grant_type: 'refresh_token', refresh_token });
    if (!tokens.access_token) {
      // invalid_grant: revoked or expired; the app asks to sign in again.
      return json(401, { error: tokens.error ?? 'refresh failed' });
    }
    return json(200, { access_token: tokens.access_token, expires_in: tokens.expires_in });
  }
  return reply(404, 'not found');
}

/// Forgets hand-overs nobody picked up.
export async function forgetStale(env) {
  await env.DB.prepare('DELETE FROM google_sessions WHERE created_at < ?1')
    .bind(Date.now() - HAND_OVER_MS)
    .run();
}

function callback(url) {
  return `${url.origin}/v1/google/callback`;
}

/// Where the sign-in in `state` goes back to: {session, port} for the
/// desktop app, {session, page} for the web app; null if unknown.
function replyTo(state) {
  const match = STATE.exec(state);
  if (!match) return null;
  const [, session, to] = match;
  if (to.startsWith('w')) return { session, page: RETURNS[Number(to.slice(1))] };
  const port = Number(to);
  return port >= 1024 && port <= 65535 ? { session, port } : null;
}

/// A pick-up code: 10 random letters of CODE_LETTERS (50 bits).
function newCode() {
  const bytes = crypto.getRandomValues(new Uint8Array(10));
  return Array.from(bytes, (byte) => CODE_LETTERS[byte & 31]).join('');
}

/// Sends the browser, with the pick-up code, back to the app that started
/// the sign-in.
function handBack(to, pickUp) {
  const fields = new URLSearchParams({ session: to.session, pickup: pickUp }).toString();
  const app = to.port ? `http://127.0.0.1:${to.port}/google?${fields}` : `${to.page}#${fields}`;
  return new Response(null, { status: 302, headers: { Location: app, 'Cache-Control': 'no-store' } });
}

async function exchange(env, fields) {
  // GOOGLE_TOKEN_URL: a stand-in for Google in the tests only.
  const response = await fetch(env.GOOGLE_TOKEN_URL ?? 'https://oauth2.googleapis.com/token', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      client_id: env.GOOGLE_CLIENT_ID,
      client_secret: env.GOOGLE_CLIENT_SECRET,
      ...fields,
    }),
  });
  return response.json();
}

function escape(text) {
  return String(text).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

function page(status, title, text) {
  const html = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>${title}</title>
<body style="font-family:system-ui,sans-serif;background:#16261d;color:#ece6d6;display:grid;place-items:center;min-height:90vh;margin:0">
<main style="max-width:28rem;padding:1.5rem;text-align:center"><h1 style="color:#e0682b">${title}</h1><p>${text}</p></main>
<script>setTimeout(() => window.close(), 1500)</script>`;
  return new Response(html, { status, headers: { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' } });
}
