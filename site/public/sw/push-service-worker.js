// Handles `push` and `notificationclick` for GroupScape's low-HP / wilderness-entry alerts.
// Registered from `push-notifications.js` — kept as its own top-level scope (not under any
// route) so it can receive push events regardless of which page the user last had open.

// Per-type vibration pattern: low-HP is the urgent one (repeating buzz), wilderness entry is a
// single informational pulse. `push.rs` sends `type`/`requireInteraction`; `tag` is derived here
// from `type` rather than sent over the wire, so a second low-HP alert always replaces the first
// instead of stacking (see `renotify` below).
const ALERT_VIBRATE = {
  low_hp: [200, 100, 200, 100, 200],
  wilderness_entry: [120],
};

self.addEventListener("push", (event) => {
  if (!event.data) {
    return;
  }
  let payload;
  try {
    payload = event.data.json();
  } catch (error) {
    return;
  }
  const title = payload.title || "GroupScape";
  const type = payload.type || "alert";
  event.waitUntil(
    self.registration.showNotification(title, {
      body: payload.body || "",
      icon: "/android-chrome-192x192.png",
      badge: "/favicon-32x32.png",
      tag: `alert-${type}`,
      renotify: true,
      requireInteraction: Boolean(payload.requireInteraction),
      vibrate: ALERT_VIBRATE[type] || [120],
      actions: [
        { action: "open", title: "Open GroupScape" },
        { action: "dismiss", title: "Dismiss" },
      ],
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  if (event.action === "dismiss") {
    return;
  }
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((clients) => {
      if (clients.length > 0) {
        return clients[0].focus();
      }
      return self.clients.openWindow("/");
    }),
  );
});
