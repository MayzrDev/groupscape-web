// Registers the install/offline-caching service worker (see src/service-worker.js, stamped with
// the app version and written to public/service-worker.js at build time). Kept separate from
// push-notifications.js's own service worker, which lives at /sw/push-service-worker.js and is
// only registered lazily when a user opts into push alerts - this one needs to control every
// page from first load, so it registers unconditionally here.
if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/service-worker.js", { updateViaCache: "none" });
  });

  // The new worker calls `skipWaiting()`/`clients.claim()` as soon as it activates (see
  // service-worker.js), so `controllerchange` fires the moment a new version takes over -
  // reload to pick it up. `refreshing` guards against this event firing more than once per page
  // life (it can only meaningfully happen once, since the reload tears down this script).
  let refreshing = false;
  navigator.serviceWorker.addEventListener("controllerchange", () => {
    if (refreshing) return;
    refreshing = true;
    window.location.reload();
  });
}
