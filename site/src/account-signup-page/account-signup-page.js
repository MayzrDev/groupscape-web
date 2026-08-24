import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";

export class AccountSignupPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{account-signup-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    const usernameValidator = (value) => {
      if (value.length === 0) {
        return "This field is required.";
      }
      if (!/^[A-Za-z0-9 _-]+$/.test(value) || value.length > 16 || value.trim().length === 0) {
        return "Enter a valid username.";
      }
    };
    const passwordValidator = (value) => {
      if (value.length < 8 || value.length > 256) {
        return "Password must be between 8 and 256 characters.";
      }
    };

    const passwordConfirmValidator = (value) => {
      if (value !== this.password.value) {
        return "Passwords don't match.";
      }
    };

    this.username = this.querySelector(".account-signup__email");
    this.username.validators = [usernameValidator];
    this.password = this.querySelector(".account-signup__password");
    this.password.validators = [passwordValidator];
    this.passwordConfirm = this.querySelector(".account-signup__password-confirm");
    this.passwordConfirm.validators = [passwordConfirmValidator];
    this.signupButton = this.querySelector(".account-signup__button");
    this.discordButton = this.querySelector(".account-signup__discord");
    this.error = this.querySelector(".account-signup__error");

    this.eventListener(this.signupButton, "click", this.signup.bind(this));
    this.eventListener(this.discordButton, "click", this.discordSignup.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  async signup() {
    if (!this.username.valid || !this.password.valid || !this.passwordConfirm.valid) return;
    try {
      this.error.textContent = "";
      this.signupButton.disabled = true;
      const response = await accountApi.register(this.username.value, this.password.value);
      if (response.ok) {
        window.history.pushState("", "", "/welcome");
      } else if (response.status === 409) {
        this.error.textContent = "That username is already registered.";
      } else if (response.status === 400) {
        this.error.textContent = await response.text();
      } else {
        this.error.textContent = "Couldn't create your account — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't create your account — try again.";
    } finally {
      this.signupButton.disabled = false;
    }
  }

  discordSignup() {
    window.location.href = accountApi.discordRedirectUrl;
  }
}

customElements.define("account-signup-page", AccountSignupPage);
