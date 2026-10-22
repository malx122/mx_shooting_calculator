// Sync server for MX Shooting Calculator.
//
// It keeps one copy per sync key: bytes the app has already encrypted, under
// an id the app derives from the key (a SHA-256 hash, 64 hex digits). The
// server never sees the key or anything readable. Each copy has a version;
// a write must name the version it replaces, so two devices writing at once
// can't overwrite each other (the loser reads again and merges).
//
//   GET    /v1/copies/{id}   200 + bytes + ETag, 404 none yet, 410 deleted
//   PUT    /v1/copies/{id}   If-None-Match: *  (first copy)  or
//                            If-Match: "<version>" (replace that version)
//                            200/201 + new ETag, 412 someone wrote first
//   DELETE /v1/copies/{id}   204; later reads and writes get 410
//
// Copies nobody has written to for two years are forgotten, and deleted
// ones after 30 days.
//
// It also signs the app in to Google, for syncing through the hunter's own
// Google Drive instead (see google.js).

import { forgetStale, google } from './google.js';

// One copy holds everything a hunter keeps, the shot log included; D1 allows
// rows up to 2 MB.
const MAX_BYTES = 1024 * 1024;
const DAY_MS = 24 * 60 * 60 * 1000;
const COPY_PATH = /^\/v1\/copies\/([0-9a-f]{64})$/;

const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, PUT, POST, DELETE, OPTIONS',
  'Access-Control-Allow-Headers': 'If-Match, If-None-Match, Content-Type',
  'Access-Control-Expose-Headers': 'ETag',
  'Access-Control-Max-Age': '86400',
};

export default {
  async fetch(request, env) {
    try {
      return await handle(request, env);
    } catch (error) {
      console.error(error);
      return reply(500, 'server error');
    }
  },

  async scheduled(_event, env) {
    const now = Date.now();
    await env.DB.prepare(
      'DELETE FROM copies WHERE updated_at < ?1 OR (data IS NULL AND updated_at < ?2)',
    )
      .bind(now - 730 * DAY_MS, now - 30 * DAY_MS)
      .run();
    await forgetStale(env);
  },
};

async function handle(request, env) {
  if (request.method === 'OPTIONS') {
    return new Response(null, { status: 204, headers: CORS });
  }
  const url = new URL(request.url);
  if (url.pathname === '/') {
    return reply(200, 'MX Shooting Calculator sync');
  }
  if (url.pathname.startsWith('/v1/google/')) {
    return google(request, env, url, reply, json);
  }
  const match = COPY_PATH.exec(url.pathname);
  if (!match) {
    return reply(404, 'not found');
  }
  const id = match[1];
  const row = await env.DB.prepare('SELECT version, data IS NULL AS gone FROM copies WHERE id = ?1')
    .bind(id)
    .first();
  switch (request.method) {
    case 'GET':
      return read(env, id, row);
    case 'PUT':
      return write(request, env, id, row);
    case 'DELETE':
      return remove(env, id, row);
    default:
      return reply(405, 'method not allowed');
  }
}

async function read(env, id, row) {
  if (!row) return reply(404, 'no copy yet');
  if (row.gone) return reply(410, 'deleted');
  const copy = await env.DB.prepare('SELECT version, data FROM copies WHERE id = ?1').bind(id).first();
  const bytes = fromBase64(copy.data);
  return new Response(bytes, {
    status: 200,
    headers: {
      ...CORS,
      'Content-Type': 'application/octet-stream',
      'Cache-Control': 'no-store',
      ETag: etag(copy.version),
    },
  });
}

async function write(request, env, id, row) {
  if (row?.gone) return reply(410, 'deleted');
  const length = Number(request.headers.get('Content-Length') ?? 0);
  if (length > MAX_BYTES) return reply(413, 'too large');
  const body = new Uint8Array(await request.arrayBuffer());
  if (body.byteLength > MAX_BYTES) return reply(413, 'too large');
  if (body.byteLength === 0) return reply(400, 'empty');
  const data = toBase64(body);
  const now = Date.now();

  if (request.headers.get('If-None-Match') === '*') {
    const result = await env.DB.prepare(
      'INSERT INTO copies (id, version, data, updated_at) VALUES (?1, 1, ?2, ?3) ON CONFLICT (id) DO NOTHING',
    )
      .bind(id, data, now)
      .run();
    if (result.meta.changes !== 1) return reply(412, 'a copy already exists');
    return written(201, 1);
  }

  const ifMatch = request.headers.get('If-Match');
  if (ifMatch === null) return reply(428, 'say which version this replaces (If-Match)');
  const version = parseEtag(ifMatch);
  if (version === null) return reply(412, 'unknown version');
  const result = await env.DB.prepare(
    'UPDATE copies SET version = version + 1, data = ?2, updated_at = ?3 WHERE id = ?1 AND version = ?4 AND data IS NOT NULL',
  )
    .bind(id, data, now, version)
    .run();
  if (result.meta.changes !== 1) return reply(412, 'another device wrote first');
  return written(200, version + 1);
}

async function remove(env, id, row) {
  if (row && !row.gone) {
    await env.DB.prepare('UPDATE copies SET version = version + 1, data = NULL, updated_at = ?2 WHERE id = ?1')
      .bind(id, Date.now())
      .run();
  } else if (!row) {
    // Mark it, so a device that hasn't heard can't start the copy again.
    await env.DB.prepare('INSERT INTO copies (id, version, data, updated_at) VALUES (?1, 1, NULL, ?2)')
      .bind(id, Date.now())
      .run();
  }
  return new Response(null, { status: 204, headers: CORS });
}

function written(status, version) {
  return new Response(null, { status, headers: { ...CORS, ETag: etag(version) } });
}

function reply(status, message) {
  return new Response(message, {
    status,
    headers: { ...CORS, 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'no-store' },
  });
}

function json(status, value) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { ...CORS, 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

function etag(version) {
  return `"${version}"`;
}

function parseEtag(text) {
  const match = /^(?:W\/)?"?(\d+)"?$/.exec(text.trim());
  return match ? Number(match[1]) : null;
}

function toBase64(bytes) {
  let text = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

function fromBase64(text) {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}
