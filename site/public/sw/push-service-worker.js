// Handles `push` and `notificationclick` for GroupScape's low-HP / wilderness-entry alerts.
// Registered from `push-notifications.js` — kept as its own top-level scope (not under any
// route) so it can receive push events regardless of which page the user last had open.

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
  event.waitUntil(
    self.registration.showNotification(title, {
      body: payload.body || "",
      icon: "/android-chrome-192x192.png",
      badge: "/favicon-32x32.png",
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((clients) => {
      if (clients.length > 0) {
        return clients[0].focus();
      }
      return self.clients.openWindow("/");
    }),
  );
});
