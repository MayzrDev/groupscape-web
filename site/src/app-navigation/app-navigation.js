import { BaseElement } from "../base-element/base-element";
import { storage } from "../data/storage";

export class AppNavigation extends BaseElement {
  constructor() {
    super();
  }

  /* eslint-disable no-unused-vars */
  html() {
    const group = storage.getGroup();
    const coinIconUrl =
      "data:image/svg+xml;base64," +
      btoa(
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#ffd700" stroke="#8a6d00" stroke-width="1.5"/><text x="12" y="16.5" font-size="12" font-family="Arial, sans-serif" font-weight="bold" text-anchor="middle" fill="#8a6d00">gp</text></svg>'
      );
    return `{{app-navigation.html}}`;
  }
  /* eslint-enable no-unused-vars */

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.subscribe("route-activated", this.handleRouteActivated.bind(this));
  }

  handleRouteActivated(route) {
    const routeComponent = route.getAttribute("route-component");

    const buttons = Array.from(this.querySelectorAll("button"));
    for (const button of buttons) {
      const c = button.getAttribute("route-component");
      if (routeComponent === c) {
        button.classList.add("active");
      } else {
        button.classList.remove("active");
      }
    }
  }
}
customElements.define("app-navigation", AppNavigation);
