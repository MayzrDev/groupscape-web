import { BaseElement } from "../base-element/base-element";
import { accountStorage } from "../data/account-storage";

// Wrappers whose page furniture already includes its own logout control.
const WRAPPERS_WITH_OWN_LOGOUT = [".authed-section", ".admin-section", ".admin-login-section"];

export class AccountLogoutIndicator extends BaseElement {
  constructor() {
    super();
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

  handleRouteActivated() {
    // The router publishes "route-activated" before it mounts/unmounts the wrapper
    // content (see router.js), so the wrapper's own logout button isn't in the DOM
    // yet at this point. Defer the recheck until that mount/unmount has happened.
    queueMicrotask(() => this.update());
  }

  update() {
    const hasOwnLogout = WRAPPERS_WITH_OWN_LOGOUT.some((selector) => document.querySelector(selector)?.active);
    const visible = Boolean(accountStorage.getAccountToken()) && !hasOwnLogout;
    this.classList.toggle("account-logout-indicator--visible", visible);
  }
}

customElements.define("account-logout-indicator", AccountLogoutIndicator);
