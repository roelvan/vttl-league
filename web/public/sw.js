// Service worker: keeps the app shell on the device so the app opens without network, and
// behind the hall's captive portal (which answers every page with its own login page).
// The API is never cached here: the app keeps its own data in local storage.

const CACHE = 'vl-shell-v1';

self.addEventListener('install', (e) => {
  self.skipWaiting();
  e.waitUntil(caches.open(CACHE).then((c) => c.add(new Request('/', { cache: 'reload' }))));
});

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)))).then(() => self.clients.claim()),
  );
});

const ours = (res) => res && res.ok && res.headers.get('x-vl-app') === '1';

self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url);
  if (e.request.method !== 'GET' || url.origin !== location.origin || url.pathname.startsWith('/api/')) return;
  if (url.pathname.startsWith('/assets/')) e.respondWith(asset(e.request));
  else if (e.request.mode === 'navigate') e.respondWith(page(e.request));
});

/** Hashed build files never change: cache first. */
async function asset(req) {
  const hit = await caches.match(req);
  if (hit) return hit;
  const res = await fetch(req);
  if (ours(res)) (await caches.open(CACHE)).put(req, res.clone());
  return res;
}

/** Pages: network first, but only an answer from our server counts; else the cached shell. */
async function page(req) {
  try {
    const res = await fetch(req);
    if (ours(res)) {
      (await caches.open(CACHE)).put('/', res.clone());
      return res;
    }
  } catch {}
  return (await caches.match('/')) ?? Response.error();
}
