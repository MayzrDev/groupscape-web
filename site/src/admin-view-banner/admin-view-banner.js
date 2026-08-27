import { BaseElement } from "../base-element/base-element";
import { adminViewSession } from "../data/admin-view-session";

// Only ever rendered inside the `.authed-section` wrapper, alongside `app-initializer` - so it
// lives and dies with the `/group/*` dashboard. Purely a chrome element for the admin looking at
// the screen; it isn't part of any group data and members viewing their own dashboard never see
// it, since `adminViewSession` is only ever set on the admin's own browser.
export class AdminViewBanner extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-view-banner.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.update();
  }

  update() {
    const session = adminViewSession.get();
    this.classList.toggle("admin-view-banner--visible", Boolean(session));
    if (!session) return;
    this.querySelector(".admin-view-banner__text").textContent = `Viewing "${session.groupName}" as admin — read only`;
    this.eventListener(this.querySelector(".admin-view-banner__exit"), "click", this.exit.bind(this));
  }

  exit() {
    const session = adminViewSession.get();
    adminViewSession.clear();
    window.history.pushState("", "", session ? `/admin/group-detail?groupId=${session.groupId}` : "/admin/groups");
  }
}

customElements.define("admin-view-banner", AdminViewBanner);
