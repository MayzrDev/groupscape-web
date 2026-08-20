import { BaseElement } from "../base-element/base-element";
import { accountStorage } from "../data/account-storage";
import { accountApi } from "../data/account-api";

export class PendingCharactersIndicator extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{pending-characters-indicator.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.update();
    this.subscribe("route-activated", this.update.bind(this));
  }

  async update() {
    if (!accountStorage.getAccountToken()) {
      this.classList.remove("pending-characters-indicator--visible");
      return;
    }

    try {
      const response = await accountApi.listCharacters();
      if (!response.ok) {
        this.classList.remove("pending-characters-indicator--visible");
        return;
      }
      const characters = await response.json();
      const hasPending = characters.some((character) => character.status === "pending");
      this.classList.toggle("pending-characters-indicator--visible", hasPending);
    } catch (error) {
      this.classList.remove("pending-characters-indicator--visible");
    }
  }
}

customElements.define("pending-characters-indicator", PendingCharactersIndicator);
