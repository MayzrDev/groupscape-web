import { BaseElement } from "../base-element/base-element";
import { adminStorage } from "../data/admin-storage";

export class AdminNavigation extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-navigation.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.subscribe("route-activated", this.handleRouteActivated.bind(this));
    const logoutButton = this.querySelector(".admin-navigation__logout");
    this.eventListener(logoutButton, "click", this.logout.bind(this));
    this.querySelector(".admin-navigation__version").textContent = `v${__APP_VERSION__}`;
  }

  handleRouteActivated(route) {
    const routeComponent = route.getAttribute("route-component");
    const tabs = Array.from(this.querySelectorAll("button[route-component]"));
    for (const tab of tabs) {
      tab.classList.toggle("active", tab.getAttribute("route-component") === routeComponent);
    }
  }

  logout() {
    adminStorage.clearAdminToken();
    window.history.pushState("", "", "/admin/login");
  }
}

customElements.define("admin-navigation", AdminNavigation);
