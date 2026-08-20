import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { accountApi } from "../src/data/account-api";
import { pushNotifications } from "../src/data/push-notifications";

describe("pushNotifications", () => {
  let originalServiceWorker;
  let originalPushManager;
  let originalNotification;

  beforeEach(() => {
    originalServiceWorker = navigator.serviceWorker;
    originalPushManager = window.PushManager;
    originalNotification = window.Notification;
    window.PushManager = class {};
  });

  afterEach(() => {
    Object.defineProperty(navigator, "serviceWorker", { value: originalServiceWorker, configurable: true });
    window.PushManager = originalPushManager;
    window.Notification = originalNotification;
    vi.restoreAllMocks();
  });

  it("reports unsupported when serviceWorker/PushManager are unavailable", async () => {
    delete navigator.serviceWorker;
    delete window.PushManager;

    expect(await pushNotifications.state()).toBe("unsupported");
  });

  it("reports denied when Notification.permission is denied", async () => {
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register: vi.fn() },
      configurable: true,
    });
    window.Notification = { permission: "denied" };

    expect(await pushNotifications.state()).toBe("denied");
  });

  it("reports subscribed when a push subscription already exists", async () => {
    const getSubscription = vi.fn().mockResolvedValue({ endpoint: "https://push.example/x" });
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register: vi.fn().mockResolvedValue({ pushManager: { getSubscription } }) },
      configurable: true,
    });
    window.Notification = { permission: "default" };

    expect(await pushNotifications.state()).toBe("subscribed");
  });

  it("reports not-subscribed when permission is granted but no subscription exists", async () => {
    const getSubscription = vi.fn().mockResolvedValue(null);
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register: vi.fn().mockResolvedValue({ pushManager: { getSubscription } }) },
      configurable: true,
    });
    window.Notification = { permission: "default" };

    expect(await pushNotifications.state()).toBe("not-subscribed");
  });

  it("subscribe requests permission, subscribes, and posts the subscription", async () => {
    vi.spyOn(accountApi, "vapidPublicKey").mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ publicKey: "AAAA" }),
    });
    vi.spyOn(accountApi, "subscribePush").mockResolvedValue({ ok: true });
    const subscribe = vi.fn().mockResolvedValue({ endpoint: "https://push.example/x", unsubscribe: vi.fn() });
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register: vi.fn().mockResolvedValue({ pushManager: { subscribe } }) },
      configurable: true,
    });
    window.Notification = { requestPermission: vi.fn().mockResolvedValue("granted") };

    const result = await pushNotifications.subscribe();

    expect(subscribe).toHaveBeenCalledWith(expect.objectContaining({ userVisibleOnly: true }));
    expect(accountApi.subscribePush).toHaveBeenCalled();
    expect(result.ok).toBe(true);
  });

  it("subscribe reports denied without subscribing when permission is refused", async () => {
    vi.spyOn(accountApi, "vapidPublicKey").mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ publicKey: "AAAA" }),
    });
    const subscribePushSpy = vi.spyOn(accountApi, "subscribePush");
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register: vi.fn() },
      configurable: true,
    });
    window.Notification = { requestPermission: vi.fn().mockResolvedValue("denied") };

    const result = await pushNotifications.subscribe();

    expect(result).toEqual({ ok: false, denied: true });
    expect(subscribePushSpy).not.toHaveBeenCalled();
  });

  it("unsubscribe removes the server-side subscription and the browser subscription", async () => {
    vi.spyOn(accountApi, "unsubscribePush").mockResolvedValue({ ok: true });
    const unsubscribe = vi.fn().mockResolvedValue(true);
    const getSubscription = vi.fn().mockResolvedValue({ endpoint: "https://push.example/x", unsubscribe });
    Object.defineProperty(navigator, "serviceWorker", {
      value: { register: vi.fn().mockResolvedValue({ pushManager: { getSubscription } }) },
      configurable: true,
    });

    const result = await pushNotifications.unsubscribe();

    expect(accountApi.unsubscribePush).toHaveBeenCalledWith("https://push.example/x");
    expect(unsubscribe).toHaveBeenCalled();
    expect(result.ok).toBe(true);
  });
});
