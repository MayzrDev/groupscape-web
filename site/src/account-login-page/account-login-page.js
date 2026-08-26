import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";

const DISCORD_ERROR_MESSAGES = {
  discord_state_mismatch: "Discord login expired before it finished. Please try again.",
  discord_failed: "Discord login failed. Please try again.",
  discord_already_linked: "That Discord account is already linked to a different GroupScape account.",
  account_disabled: "This account has been disabled.",
};

export class AccountLoginPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{account-login-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    const fieldRequiredValidator = (value) => {
      if (value.length === 0) {
        return "This field is required.";
      }
    };

    this.username = this.querySelector(".account-login__username");
    this.username.validators = [fieldRequiredValidator];
    this.password = this.querySelector(".account-login__password");
    this.password.validators = [fieldRequiredValidator];
    this.loginButton = this.querySelector(".account-login__button");
    this.discordButton = this.querySelector(".account-login__discord");
    this.error = this.querySelector(".account-login__error");

    this.eventListener(this.loginButton, "click", this.login.bind(this));
    this.eventListener(this.discordButton, "click", this.discordLogin.bind(this));

    this.showDiscordErrorIfPresent();
  }

  showDiscordErrorIfPresent() {
    const errorCode = sessionStorage.getItem("discordError");
    if (!errorCode) return;
    sessionStorage.removeItem("discordError");
    this.error.textContent = DISCORD_ERROR_MESSAGES[errorCode] || "Discord login failed. Please try again.";
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  async login() {
    if (!this.username.valid || !this.password.valid) return;
    try {
      this.error.textContent = "";
      this.loginButton.disabled = true;
      const response = await accountApi.login(this.username.value, this.password.value);
      if (response.ok) {
        window.history.pushState("", "", "/welcome");
      } else if (response.status === 403) {
        this.error.textContent = "This account has been disabled.";
      } else {
        this.error.textContent = "Username or password is incorrect.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't log in — try again.";
    } finally {
      this.loginButton.disabled = false;
    }
  }

  discordLogin() {
    window.location.href = accountApi.discordRedirectUrl;
  }
}

customElements.define("account-login-page", AccountLoginPage);
