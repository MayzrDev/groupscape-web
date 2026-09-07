import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { pwaInstall } from "../src/data/pwa-install";

describe("pwaInstall", () => {
  let originalUserAgent;
  let originalStandalone;
  let originalMatchMedia;

  beforeEach(() => {
    originalUserAgent = window.navigator.userAgent;
    originalStandalone = window.navigator.standalone;
    originalMatchMedia = window.matchMedia;
    sessionStorage.clear();
  });

  afterEach(() => {
    Object.defineProperty(window.navigator, "userAgent", { value: originalUserAgent, configurable: true });
    Object.defineProperty(window.navigator, "standalone", { value: originalStandalone, configurable: true });
    window.matchMedia = originalMatchMedia;
    pwaInstall.deferredPrompt = null;
    sessionStorage.clear();
  });

  function setUserAgent(userAgent) {
    Object.defineProperty(window.navigator, "userAgent", { value: userAgent, configurable: true });
  }

  function setStandalone(matches) {
    window.matchMedia = vi.fn().mockReturnValue({ matches });
  }

  it("detects iOS from the user agent", () => {
    setUserAgent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)");
    expect(pwaInstall.isIos).toBe(true);

    setUserAgent("Mozilla/5.0 (Linux; Android 14)");
    expect(pwaInstall.isIos).toBe(false);
  });

  it("shows the iOS prompt only on iOS, not standalone, and not dismissed", () => {
    setUserAgent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)");
    setStandalone(false);

    expect(pwaInstall.shouldShowIosPrompt()).toBe(true);

    pwaInstall.dismiss();
    expect(pwaInstall.shouldShowIosPrompt()).toBe(false);
  });

  it("never shows the iOS prompt when already running standalone", () => {
    setUserAgent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)");
    setStandalone(true);

    expect(pwaInstall.shouldShowIosPrompt()).toBe(false);
  });

  it("captures the beforeinstallprompt event, preventing the default mini-infobar", () => {
    const handler = vi.fn();
    const unbind = pwaInstall.captureBeforeInstallPrompt(handler);

    const event = new Event("beforeinstallprompt");
    event.preventDefault = vi.fn();
    window.dispatchEvent(event);

    expect(event.preventDefault).toHaveBeenCalled();
    expect(handler).toHaveBeenCalled();
    expect(pwaInstall.deferredPrompt).toBe(event);

    unbind();
  });

  it("promptInstall prompts the captured event and clears it afterwards", async () => {
    pwaInstall.deferredPrompt = {
      prompt: vi.fn(),
      userChoice: Promise.resolve({ outcome: "accepted" }),
    };

    const result = await pwaInstall.promptInstall();

    expect(result).toEqual({ outcome: "accepted" });
    expect(pwaInstall.deferredPrompt).toBe(null);
  });

  it("promptInstall reports unavailable when there's no captured prompt", async () => {
    pwaInstall.deferredPrompt = null;

    expect(await pwaInstall.promptInstall()).toEqual({ outcome: "unavailable" });
  });
});
