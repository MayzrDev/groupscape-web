import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";

/**
 * Opened by the plugin's "Link this character" button with `?accountHash=&rsn=` — the browser's
 * already-authenticated session is what proves which account it links to, so this page just
 * confirms and makes the one authenticated call.
 */
export class LinkPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{link-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();

    const params = new URLSearchParams(window.location.search);
    this.accountHash = params.get("accountHash");
    this.rsn = params.get("rsn");

    this.render();
    this.status = this.querySelector(".link-page__status");
    this.actions = this.querySelector(".link-page__actions");
    this.error = this.querySelector(".link-page__error");

    if (!this.accountHash || !this.rsn) {
      this.status.textContent =
        'Missing character info. Open this page from the "Link this character" button in the GroupScape RuneLite plugin\'s sidebar panel.';
      return;
    }

    this.checkSession();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (response.ok) {
      this.showConfirm();
    } else {
      this.status.innerHTML =
        'You need to be logged into a GroupScape account. Log in, then click "Link this character" in the plugin again.';
    }
  }

  showConfirm() {
    this.status.textContent = `Link ${this.rsn} to your GroupScape account?`;
    this.actions.innerHTML = `<button class="link-page__confirm men-button">Link character</button>`;
    this.confirmButton = this.querySelector(".link-page__confirm");
    this.eventListener(this.confirmButton, "click", this.confirm.bind(this));
  }

  async confirm() {
    this.confirmButton.disabled = true;
    this.confirmButton.textContent = "Linking…";
    this.error.textContent = "";

    try {
      const response = await accountApi.linkCharacter(this.accountHash, this.rsn);
      if (response.ok) {
        this.status.textContent = `Linked ${this.rsn} to your account.`;
        this.actions.innerHTML = "";
      } else if (response.status === 409) {
        this.error.textContent = "That character is already linked to a different account.";
      } else if (response.status === 403) {
        this.error.textContent = "You've already linked the maximum number of characters.";
      } else {
        this.error.textContent = "Couldn't link that character — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't link that character — try again.";
    } finally {
      if (this.confirmButton) {
        this.confirmButton.disabled = false;
        this.confirmButton.textContent = "Link character";
      }
    }
  }
}

customElements.define("link-page", LinkPage);
