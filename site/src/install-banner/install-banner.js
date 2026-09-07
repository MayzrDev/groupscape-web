import { BaseElement } from "../base-element/base-element";
import { pwaInstall } from "../data/pwa-install";

// Dismissible bottom banner offering to install the PWA. Android/desktop gets a real
// `beforeinstallprompt`-backed Install button; iOS Safari never fires that event, so it gets a
// manual "Add to Home Screen" instruction instead (checked on connect, since there's no event to
// wait for there).
export class InstallBanner extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{install-banner.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.eventListener(this.querySelector(".install-banner__close"), "click", this.dismiss.bind(this));
    this.eventListener(this.querySelector(".install-banner__not-now"), "click", this.dismiss.bind(this));
    this.eventListener(this.querySelector(".install-banner__install"), "click", this.install.bind(this));

    this.eventUnbinders.add(pwaInstall.captureBeforeInstallPrompt(() => this.show("android")));
    this.eventUnbinders.add(pwaInstall.onInstalled(this.hide.bind(this)));

    if (pwaInstall.shouldShowIosPrompt()) {
      this.show("ios");
    }
  }

  show(variant) {
    if (pwaInstall.dismissed || pwaInstall.isStandalone) return;
    this.classList.toggle("install-banner--ios", variant === "ios");
    this.classList.add("install-banner--visible");
  }

  hide() {
    this.classList.remove("install-banner--visible");
  }

  dismiss() {
    pwaInstall.dismiss();
    this.hide();
  }

  async install() {
    await pwaInstall.promptInstall();
    this.hide();
  }
}

customElements.define("install-banner", InstallBanner);
