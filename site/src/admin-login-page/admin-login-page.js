import { BaseElement } from "../base-element/base-element";
import { adminApi } from "../data/admin-api";
import { adminStorage } from "../data/admin-storage";

export class AdminLoginPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{admin-login-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    const fieldRequiredValidator = (value) => {
      if (value.length === 0) {
        return "This field is required.";
      }
    };
    this.token = this.querySelector(".admin-login__token");
    this.token.validators = [fieldRequiredValidator];
    this.loginButton = this.querySelector(".admin-login__button");
    this.error = this.querySelector(".admin-login__error");
    this.eventListener(this.loginButton, "click", this.login.bind(this));
  }

  async login() {
    if (!this.token.valid) return;
    try {
      this.error.innerHTML = "";
      this.loginButton.disabled = true;
      const token = this.token.value;
      adminApi.setCredentials(token);
      const response = await adminApi.amILoggedIn();
      if (response.ok) {
        adminStorage.storeAdminToken(token);
        window.history.pushState("", "", "/admin");
      } else if (response.status === 429) {
        this.error.innerHTML = "Too many attempts. Try again in a few minutes.";
      } else if (response.status === 404) {
        this.error.innerHTML = "The admin panel is not enabled on this server.";
      } else {
        this.error.innerHTML = "Admin token is incorrect";
      }
    } catch (error) {
      this.error.innerHTML = `Unable to login ${error}`;
    } finally {
      this.loginButton.disabled = false;
    }
  }
}

customElements.define("admin-login-page", AdminLoginPage);
