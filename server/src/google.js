// Signing in to Google for sync through Google Drive.
//
// The app's Google client is a web client, whose code exchange needs the
// client secret, so it happens here. The app never sees the secret, and the
// server keeps no tokens beyond a few minutes' hand-over:
//
//   GET  /v1/google/start?session=S   the app opens this in a browser; it
//                                     goes on to Google's sign-in
//   GET  /v1/google/callback          Google comes back here; the tokens
//                                     wait under S for the app
//   POST /v1/google/session/S         the app picks them up, once
//                                     (404 until the hunter has signed in)
//   POST /v1/google/refresh           {refresh_token} -> a new access token
//
// Signing out only forgets the tokens on the device: revoking them at
// Google would sign out every device. The hunter can remove the app's
// access in their Google account.
//
// S is a random 32-hex-digit id the app makes for each sign-in.

// Only the app's own hidden folder in the hunter's Drive: not even who they are.
const SCOPES = 'https://www.googleapis.com/auth/drive.appdata';
const SESSION = /^[0-9a-f]{32}$/;
const HAND_OVER_MS = 10 * 60 * 1000;

export async function google(request, env, url, reply, json) {
  const path = url.pathname;
  if (!env.GOOGLE_CLIENT_ID || !env.GOOGLE_CLIENT_SECRET) {
    return reply(503, 'Google sign-in is not set up on this server');
  }
  if (path === '/v1/google/start' && request.method === 'GET') {
    const session = url.searchParams.get('session') ?? '';
    if (!SESSION.test(session)) return reply(400, 'bad session');
    const auth = new URL('https://accounts.google.com/o/oauth2/v2/auth');
    auth.search = new URLSearchParams({
      client_id: env.GOOGLE_CLIENT_ID,
      redirect_uri: callback(url),
      response_type: 'code',
      scope: SCOPES,
      access_type: 'offline',
      // Ask again, so Google hands out a refresh token every time.
      prompt: 'consent select_account',
      state: session,
    }).toString();
    return Response.redirect(auth.toString(), 302);
  }
  if (path === '/v1/google/callback' && request.method === 'GET') {
    const session = url.searchParams.get('state') ?? '';
    const code = url.searchParams.get('code');
    if (!SESSION.test(session) || !code) {
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
    await env.DB.prepare(
      'INSERT INTO google_sessions (id, tokens, created_at) VALUES (?1, ?2, ?3) ON CONFLICT (id) DO NOTHING',
    )
      .bind(session, JSON.stringify(handOver), Date.now())
      .run();
    return page(200, 'Signed in', 'Go back to MX Shooting Calculator; it starts syncing in a moment. You can close this window.');
  }
  const pickUp = /^\/v1\/google\/session\/([0-9a-f]{32})$/.exec(path);
  if (pickUp && request.method === 'POST') {
    const row = await env.DB.prepare('DELETE FROM google_sessions WHERE id = ?1 AND created_at > ?2 RETURNING tokens')
      .bind(pickUp[1], Date.now() - HAND_OVER_MS)
      .first();
    return row ? json(200, JSON.parse(row.tokens)) : reply(404, 'not signed in yet');
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

async function exchange(env, fields) {
  const response = await fetch('https://oauth2.googleapis.com/token', {
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
