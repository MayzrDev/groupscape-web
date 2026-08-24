import { BaseElement } from "../base-element/base-element";
import { accountStorage } from "../data/account-storage";

const HIDDEN_WRAPPERS = new Set([".authed-section", ".admin-section", ".admin-login-section"]);

export class AccountLogoutIndicator extends BaseElement {
  constructor() {
    super();
    this.currentWrapper = null;
  }

  html() {
    return `{{account-logout-indicator.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.update();
    this.subscribe("route-activated", this.handleRouteActivated.bind(this));
  }

  handleRouteActivated(route) {
    this.currentWrapper = route.getAttribute("route-wrapper");
    this.update();
  }

  update() {
    const visible = Boolean(accountStorage.getAccountToken()) && !HIDDEN_WRAPPERS.has(this.currentWrapper);
    this.classList.toggle("account-logout-indicator--visible", visible);
  }
}

customElements.define("account-logout-indicator", AccountLogoutIndicator);
