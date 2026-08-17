import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";
import { pubsub } from "../data/pubsub";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";

export class EditMember extends BaseElement {
  constructor() {
    super();
    this._canKick = false;
  }

  // Set by group-settings once it knows the acting account's effective permissions - defaults
  // to false so a member without kick permission never sees the buttons flash into view before
  // the permission check resolves.
  set canKick(value) {
    this._canKick = value;
    this.updateButtonsVisibility();
  }

  get canKick() {
    return this._canKick;
  }

  html() {
    return `{{edit-member.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.error = this.querySelector(".edit-member__error");
    const removeButton = this.querySelector(".edit-member__remove");
    const blockButton = this.querySelector(".edit-member__block");

    this.eventListener(removeButton, "click", this.confirmRemove.bind(this));
    this.eventListener(blockButton, "click", this.confirmBlock.bind(this));
    this.updateButtonsVisibility();
  }

  updateButtonsVisibility() {
    const buttons = this.querySelector(".edit-member__buttons");
    if (buttons) {
      buttons.style.display = this._canKick ? "" : "none";
    }
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  lastSeenText() {
    if (!this.member.lastUpdated) return "Never seen";
    if (!this.member.inactive) return "Online now";

    const seconds = Math.floor((Date.now() - this.member.lastUpdated.getTime()) / 1000);
    if (seconds < 60) return "Last seen moments ago";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `Last seen ${minutes} minute${minutes === 1 ? "" : "s"} ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `Last seen ${hours} hour${hours === 1 ? "" : "s"} ago`;
    const days = Math.floor(hours / 24);
    return `Last seen ${days} day${days === 1 ? "" : "s"} ago`;
  }

  hideError() {
    this.error.innerHTML = "";
  }

  showError(message) {
    this.error.innerHTML = message;
  }

  confirmRemove() {
    this.hideError();
    confirmDialogManager.confirm({
      headline: `Remove ${this.member.name}?`,
      body: "All player data will be lost. They'll rejoin automatically the next time their plugin sends data with the group token.",
      yesCallback: this.removeMember.bind(this),
      noCallback: () => {},
    });
  }

  async removeMember() {
    try {
      loadingScreenManager.showLoadingScreen();
      const result = await api.removeMember(this.member.name);
      if (result.ok) {
        await api.restart();
        await pubsub.waitUntilNextEvent("get-group-data", false);
      } else {
        const message = await result.text();
        this.showError(`Failed to remove member ${message}`);
      }
    } catch (error) {
      this.showError(`Failed to remove member ${error}`);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }

  confirmBlock() {
    this.hideError();
    confirmDialogManager.confirm({
      headline: `Block ${this.member.name}?`,
      body: "All player data will be lost and they won't be able to rejoin until you unblock them.",
      yesCallback: this.blockMember.bind(this),
      noCallback: () => {},
    });
  }

  async blockMember() {
    try {
      loadingScreenManager.showLoadingScreen();
      const result = await api.blockMember(this.member.name);
      if (result.ok) {
        await api.restart();
        await pubsub.waitUntilNextEvent("get-group-data", false);
        pubsub.publish("blocked-members-changed");
      } else {
        const message = await result.text();
        this.showError(`Failed to block member ${message}`);
      }
    } catch (error) {
      this.showError(`Failed to block member ${error}`);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }
}

customElements.define("edit-member", EditMember);
