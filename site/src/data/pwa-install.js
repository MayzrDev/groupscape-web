// "Not now" is a per-session dismissal (sessionStorage) rather than permanent, so a user who
// closes the banner without deciding still gets asked again on their next visit; an actual
// install (via `appinstalled`, or already running standalone) suppresses it for good since
// `isStandalone` becomes true on every future load.
const DISMISS_KEY = "installBannerDismissed";

class PwaInstall {
  constructor() {
    this.deferredPrompt = null;
  }

  get isStandalone() {
    return (
      (typeof window.matchMedia === "function" && window.matchMedia("(display-mode: standalone)").matches) ||
      window.navigator.standalone === true
    );
  }

  get isIos() {
    return /iphone|ipad|ipod/i.test(window.navigator.userAgent) && !window.MSStream;
  }

  get dismissed() {
    return sessionStorage.getItem(DISMISS_KEY) === "1";
  }

  dismiss() {
    sessionStorage.setItem(DISMISS_KEY, "1");
  }

  shouldShowIosPrompt() {
    return this.isIos && !this.isStandalone && !this.dismissed;
  }

  captureBeforeInstallPrompt(handler) {
    const listener = (event) => {
      event.preventDefault();
      this.deferredPrompt = event;
      handler();
    };
    window.addEventListener("beforeinstallprompt", listener);
    return () => window.removeEventListener("beforeinstallprompt", listener);
  }

  onInstalled(handler) {
    window.addEventListener("appinstalled", handler);
    return () => window.removeEventListener("appinstalled", handler);
  }

  async promptInstall() {
    if (!this.deferredPrompt) {
      return { outcome: "unavailable" };
    }
    this.deferredPrompt.prompt();
    const { outcome } = await this.deferredPrompt.userChoice;
    this.deferredPrompt = null;
    return { outcome };
  }
}

const pwaInstall = new PwaInstall();

export { pwaInstall };
