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

    const emailValidator = (value) => {
      if (value.length === 0) {
        return "This field is required.";
      }
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)) {
        return "Enter a valid email address.";
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

    this.email = this.querySelector(".account-signup__email");
    this.email.validators = [emailValidator];
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
    if (!this.email.valid || !this.password.valid || !this.passwordConfirm.valid) return;
    try {
      this.error.textContent = "";
      this.signupButton.disabled = true;
      const response = await accountApi.register(this.email.value, this.password.value);
      if (response.ok) {
        window.history.pushState("", "", "/welcome");
      } else if (response.status === 409) {
        this.error.textContent = "That email is already registered.";
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
