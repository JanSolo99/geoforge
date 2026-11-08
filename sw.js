// The UniGIS web app's service worker: it lets the installed app (and the page) start offline.
//
// This file is a template. `cargo xtask web-pwa dist/web`, run after `trunk build` by the deploy
// workflows, fills in VERSION and SHELL with the built files (Trunk names them by content hash) and
// writes the result to dist/web/sw.js. A plain `trunk serve` or `trunk build` has no service worker.
//
// - The app shell (index.html, the hashed .js and .wasm, the manifest) is cached when the worker
//   installs. Hashed files are served cache-first; the page itself network-first, so a new deploy
//   is picked up whenever there is a connection.
// - Every other request (map tiles, the AI Assistant, plugins, data the user opens, analytics) is
//   left to the browser: the worker never answers or stores it.
// - A new deploy's worker waits until the page asks it to take over (the "new version" notice in
//   index.html) or every UniGIS window has closed, so the app never changes under the user.

const VERSION = "b77d417a71765b36";
const SHELL = [
  "icons/apple-touch-icon.png",
  "icons/favicon.png",
  "icons/icon-192.png",
  "icons/icon-512.png",
  "icons/icon-maskable-512.png",
  "index.html",
  "manifest.webmanifest",
  "rustgis-web-c8f2143536b1636e.js",
  "rustgis-web-c8f2143536b1636e_bg.wasm",
  "snippets/rustgis-web-265adc5027117fbe/inline0.js",
];
// Caches are per origin, and pull request previews share one: name them by scope so one build
// never removes another's.
const PREFIX = "rustgis-" + self.registration.scope + "-";
const CACHE = PREFIX + VERSION;

const urlOf = (path) => new URL(path, self.registration.scope).href;
const PAGE = urlOf("index.html");
// How long the page waits for the network before a cached copy is served instead.
const PAGE_TIMEOUT_MS = 4000;
const CACHED = new Set(SHELL.map(urlOf));

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE).then(async (cache) => {
      await cache.addAll(SHELL.map((p) => new Request(urlOf(p), { cache: "reload" })));
    }),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith(PREFIX) && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("message", (event) => {
  if (event.data && event.data.type === "rustgis-update") self.skipWaiting();
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  if (req.mode === "navigate") {
    // Only the app's own page: `?data=` and the other start-up options are read by the app from
    // the address bar, so every query string gets the same page.
    const page = req.url.split(/[?#]/)[0];
    if (page !== self.registration.scope && page !== PAGE) return;
    // The cached page stays the one this worker installed with: a newer deploy's page would name
    // files this cache does not hold. A connection that stalls instead of failing gets the cached
    // page after a few seconds, so the app still starts.
    const cached = () => caches.match(PAGE, { cacheName: CACHE });
    event.respondWith(
      new Promise((resolve) => {
        let done = false;
        const finish = (resp) => {
          if (!done && resp) {
            done = true;
            resolve(resp);
          }
        };
        const timer = setTimeout(() => cached().then(finish), PAGE_TIMEOUT_MS);
        fetch(req).then(
          (resp) => {
            clearTimeout(timer);
            finish(resp);
          },
          () => {
            clearTimeout(timer);
            cached().then((hit) => finish(hit || Response.error()));
          },
        );
      }),
    );
    return;
  }
  const key = req.url.split("#")[0];
  if (!CACHED.has(key)) return;
  event.respondWith(caches.match(key, { cacheName: CACHE }).then((hit) => hit || fetch(req)));
});
