// Keeps MX Shooting Calculator working offline. The page is fetched from
// the network first, so a new release loads as soon as there is a
// connection; the app's files have their content hash in their names, so
// they are served from the cache once fetched.
const CACHE = 'mxsc';
// The app's own build files: mxsc-<hash>.js and mxsc-<hash>_bg.wasm.
const BUILD_FILE = /\/mxsc-[0-9a-f]+(_bg\.wasm|\.js)$/;

// On install, cache the page and the build files it names, so the app
// opens offline even after its very first visit.
self.addEventListener('install', (event) => {
  self.skipWaiting();
  event.waitUntil(
    (async () => {
      const page = await fetch('./', { cache: 'no-cache' });
      if (!page.ok) {
        return;
      }
      const html = await page.clone().text();
      await remember(new Request('./'), page);
      const files = new Set(html.match(/mxsc-[0-9a-f]+(?:_bg\.wasm|\.js)/g) || []);
      for (const file of ['manifest.webmanifest', 'pwa/icon-192.png', 'pwa/apple-touch-icon.png']) {
        files.add(file);
      }
      for (const file of files) {
        const response = await fetch(file);
        if (response.ok) {
          await remember(new Request(new URL(file, self.registration.scope)), response);
        }
      }
    })(),
  );
});
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

async function remember(request, response) {
  const cache = await caches.open(CACHE);
  await cache.put(request, response);
  // A new build replaces the previous one's files.
  const path = new URL(request.url).pathname;
  const kind = path.match(BUILD_FILE);
  if (kind) {
    for (const old of await cache.keys()) {
      const oldPath = new URL(old.url).pathname;
      const oldKind = oldPath.match(BUILD_FILE);
      if (oldKind && oldKind[1] === kind[1] && oldPath !== path) {
        await cache.delete(old);
      }
    }
  }
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET' || new URL(request.url).origin !== self.location.origin) {
    return;
  }
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request)
        .then((response) => {
          if (response.ok) {
            event.waitUntil(remember(request, response.clone()));
          }
          return response;
        })
        .catch(async () => (await caches.match(request)) || (await caches.match('./'))),
    );
    return;
  }
  event.respondWith(
    caches.match(request).then(
      (cached) =>
        cached ||
        fetch(request).then((response) => {
          if (response.ok) {
            event.waitUntil(remember(request, response.clone()));
          }
          return response;
        }),
    ),
  );
});
