import { BaseElement } from "../base-element/base-element";
import { networkStatus } from "../data/network-status";

// Pinned above the app chrome for as long as the browser reports offline - a persistent status
// strip rather than a toast, since it needs to keep reminding the user that whatever's on screen
// (member HP, position, etc) may no longer be live.
export class OfflineBanner extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{offline-banner.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.update();
    this.eventUnbinders.add(networkStatus.onChange(this.update.bind(this)));
  }

  update() {
    this.classList.toggle("offline-banner--visible", networkStatus.offline);
  }
}

customElements.define("offline-banner", OfflineBanner);
