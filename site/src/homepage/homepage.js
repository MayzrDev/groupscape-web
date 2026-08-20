import { BaseElement } from "../base-element/base-element";
import { accountStorage } from "../data/account-storage";

export class Homepage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{homepage.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    if (accountStorage.getAccountToken()) {
      // Deferred: every other redirect-on-mount in this app naturally waits on an `await
      // fetch(...)` first, which happens to push it past the router's synchronous initial
      // route-registration pass. This one has no such gap - pushState()ing here directly,
      // during that pass, re-enters `router.handleLocationChange` before routes further down
      // index.html (like this one's own target) have registered themselves, and blows the
      // call stack.
      setTimeout(() => window.history.pushState("", "", "/characters"), 0);
    }
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }
}

customElements.define("homepage-page", Homepage);
