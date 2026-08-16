import { BaseElement } from "../base-element/base-element";
import { adminStorage } from "../data/admin-storage";

export class AdminIndicator extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-indicator.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.update();
    this.subscribe("route-activated", this.update.bind(this));
  }

  update() {
    this.classList.toggle("admin-indicator--visible", Boolean(adminStorage.getAdminToken()));
  }
}

customElements.define("admin-indicator", AdminIndicator);
