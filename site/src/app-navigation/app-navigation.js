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
    this.reserveAccountPanelSpace();
    if (document.fonts && document.fonts.ready) {
      document.fonts.ready.then(() => this.reserveAccountPanelSpace());
    }
  }

  // The account panel is absolutely positioned (see app-navigation.css) so its taller,
  // vertically-stacked buttons don't push page content down. That takes it out of flex
  // flow, so the tabs panel needs its rendered width reserved via margin to stay clear of it.
  // Its height is exposed as a CSS var too: being positioned, it paints above static page
  // content regardless of DOM order, so any page content sharing that top-right corner
  // (a toolbar spanning full width, say) needs the var to clear it.
  reserveAccountPanelSpace() {
    const tabsPanel = this.querySelector(".app-navigation__panel--tabs");
    const accountPanel = this.querySelector(".app-navigation__panel--account");
    if (tabsPanel && accountPanel) {
      tabsPanel.style.marginRight = `${accountPanel.offsetWidth + 6}px`;
      document.documentElement.style.setProperty("--account-panel-height", `${accountPanel.offsetHeight}px`);
    }
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
