import { accountApi } from "./account-api";

// Web push subscriptions are encoded urlsafe-base64; `PushManager.subscribe`'s
// `applicationServerKey` needs a raw `Uint8Array` instead.
function urlBase64ToUint8Array(base64String) {
  const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding).replace(/-/g, "+").replace(/_/g, "/");
  const rawData = atob(base64);
  return Uint8Array.from([...rawData].map((char) => char.charCodeAt(0)));
}

class PushNotifications {
  get supported() {
    return "serviceWorker" in navigator && "PushManager" in window;
  }

  async registration() {
    if (!this.supported) {
      return null;
    }
    return navigator.serviceWorker.register("/sw/push-service-worker.js");
  }

  /**
   * "denied" | "unsupported" | "subscribed" | "not-subscribed" — the account page renders
   * the toggle off this state rather than tracking permission and subscription separately.
   */
  async state() {
    if (!this.supported) {
      return "unsupported";
    }
    if (Notification.permission === "denied") {
      return "denied";
    }
    const registration = await this.registration();
    const subscription = await registration.pushManager.getSubscription();
    return subscription ? "subscribed" : "not-subscribed";
  }

  async subscribe() {
    const vapidResponse = await accountApi.vapidPublicKey();
    if (!vapidResponse.ok) {
      return { ok: false };
    }
    const { publicKey } = await vapidResponse.json();

    const permission = await Notification.requestPermission();
    if (permission !== "granted") {
      return { ok: false, denied: permission === "denied" };
    }

    const registration = await this.registration();
    const subscription = await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(publicKey),
    });

    const response = await accountApi.subscribePush(subscription);
    if (!response.ok) {
      await subscription.unsubscribe();
      return { ok: false };
    }
    return { ok: true };
  }

  async unsubscribe() {
    const registration = await this.registration();
    const subscription = await registration.pushManager.getSubscription();
    if (!subscription) {
      return { ok: true };
    }
    await accountApi.unsubscribePush(subscription.endpoint);
    await subscription.unsubscribe();
    return { ok: true };
  }
}

const pushNotifications = new PushNotifications();

export { pushNotifications };
