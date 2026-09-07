// Install/offline-caching service worker for GroupScape. Separate from the push service worker
// at /sw/push-service-worker.js (see push-notifications.js) - that one only needs to exist to
// receive push events and doesn't need to control any page, so merging the two would only add
// risk to a feature that already works.
//
// `{{version}}` is substituted at build time (see build.js) with package.json's version, so every
// release gets its own shell cache and stale ones are dropped on activate. The runtime cache is
// deliberately NOT versioned - item icons/map tiles/wiki data are immutable by id, so there's no
// reason to throw away perfectly good cached entries just because the shell changed.
const SHELL_CACHE = "groupscape-shell-v{{version}}";
const RUNTIME_CACHE = "groupscape-runtime";

// The whole app shell is one self-contained HTML document (CSS/JS inlined at build time - see
// htmlBuildPlugin in build.js), so there's no separate bundle to precache: "/" IS the shell.
const PRECACHE_URLS = [
  "/",
  "/site.webmanifest",
  "/favicon.ico",
  "/favicon-16x16.png",
  "/favicon-32x32.png",
  "/apple-touch-icon.png",
  "/android-chrome-192x192.png",
  "/android-chrome-256x256.png",
  "/logo.png",
  "/fonts/RuneScape-Chat-07.ttf",
  "/fonts/RuneScape-Chat-Bold-07.ttf",
  "/fonts/runescape-tiny.ttf",
];

// Assets under these prefixes are static and immutable by id/filename (item icons, map tiles,
// static wiki data) - safe to cache-first and keep forever, unlike the live `/api/*` group data.
const RUNTIME_CACHE_PREFIXES = ["/icons/", "/map/", "/data/", "/fonts/", "/ui/", "/images/"];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(SHELL_CACHE)
      .then((cache) => cache.addAll(PRECACHE_URLS))
      .then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key.startsWith("groupscape-shell-") && key !== SHELL_CACHE)
            .map((key) => caches.delete(key))
        )
      )
      .then(() => self.clients.claim())
  );
});

function isRuntimeCacheable(url) {
  return RUNTIME_CACHE_PREFIXES.some((prefix) => url.pathname.startsWith(prefix));
}

self.addEventListener("fetch", (event) => {
  const { request } = event;
  const url = new URL(request.url);

  if (request.method !== "GET" || url.origin !== self.location.origin) {
    return;
  }

  // Navigations: network-first so an online user always gets the latest shell, falling back to
  // the cached shell when offline. Every route is client-side rendered from "/" (see
  // scripts/server.js's catch-all), so the cached root document works for any path.
  if (request.mode === "navigate") {
    event.respondWith(fetch(request).catch(() => caches.match("/", { cacheName: SHELL_CACHE })));
    return;
  }

  if (isRuntimeCacheable(url)) {
    event.respondWith(
      caches.match(request, { cacheName: RUNTIME_CACHE }).then((cached) => {
        if (cached) return cached;
        return fetch(request).then((response) => {
          if (response.ok) {
            const responseClone = response.clone();
            caches.open(RUNTIME_CACHE).then((cache) => cache.put(request, responseClone));
          }
          return response;
        });
      })
    );
  }
});
