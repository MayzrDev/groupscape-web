import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { pushNotifications } from "../data/push-notifications";

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

const fieldRequiredValidator = (value) => {
  if (value.length === 0) {
    return "This field is required.";
  }
};

function initials(email) {
  if (!email) return "?";
  return email.slice(0, 2).toUpperCase();
}

/**
 * Account-level dashboard: profile info, email/password change, and a link out to
 * `/account/characters`. Reached directly (no nav entry point yet) so it does its own session
 * check rather than relying on a route wrapper, matching `characters-page`.
 */
export class AccountPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{account-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.status = this.querySelector(".account-page__status");
    this.content = this.querySelector(".account-page__content");
    this.avatar = this.querySelector(".account-page__avatar");
    this.emailDisplay = this.querySelector(".account-page__email-display");
    this.since = this.querySelector(".account-page__since");

    this.emailInput = this.querySelector(".account-page__email");
    this.emailInput.validators = [emailValidator];
    this.emailSaveButton = this.querySelector(".account-page__email-save");
    this.emailError = this.querySelector(".account-page__email-error");
    this.emailStatus = this.querySelector(".account-page__email-status");
    this.eventListener(this.emailSaveButton, "click", this.saveEmail.bind(this));

    this.currentPasswordInput = this.querySelector(".account-page__current-password");
    this.currentPasswordInput.validators = [fieldRequiredValidator];
    this.newPasswordInput = this.querySelector(".account-page__new-password");
    this.newPasswordInput.validators = [passwordValidator];
    this.passwordSaveButton = this.querySelector(".account-page__password-save");
    this.passwordError = this.querySelector(".account-page__password-error");
    this.passwordStatus = this.querySelector(".account-page__password-status");
    this.eventListener(this.passwordSaveButton, "click", this.savePassword.bind(this));

    this.apiKeyReveal = this.querySelector(".account-page__api-key-reveal");
    this.apiKeyValue = this.querySelector(".account-page__api-key-value");
    this.apiKeyRegenerateButton = this.querySelector(".account-page__api-key-regenerate");
    this.apiKeyError = this.querySelector(".account-page__api-key-error");
    this.apiKeyStatus = this.querySelector(".account-page__api-key-status");
    this.eventListener(this.apiKeyRegenerateButton, "click", this.confirmRegenerateApiKey.bind(this));

    this.deleteButton = this.querySelector(".account-page__delete-button");
    this.eventListener(this.deleteButton, "click", this.confirmDeleteAccount.bind(this));

    this.notificationsToggle = this.querySelector(".account-page__notifications-toggle");
    this.notificationsStatus = this.querySelector(".account-page__notifications-status");
    this.eventListener(this.notificationsToggle, "click", this.toggleNotifications.bind(this));

    this.checkSession();
    this.refreshNotificationsState();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (!response.ok) {
      window.history.pushState("", "", "/account/login");
      return;
    }
    const account = await response.json();
    this.status.textContent = "";
    this.content.hidden = false;
    this.renderProfile(account);
    this.showFreshApiKeyIfPresent();
  }

  showFreshApiKeyIfPresent() {
    const freshApiKey = sessionStorage.getItem("freshApiKey");
    if (!freshApiKey) return;
    sessionStorage.removeItem("freshApiKey");
    this.renderApiKey(freshApiKey);
  }

  renderApiKey(apiKey) {
    this.apiKeyValue.textContent = apiKey;
    this.apiKeyReveal.hidden = false;
  }

  renderProfile(account) {
    this.account = account;
    this.avatar.textContent = initials(account.email);
    this.emailDisplay.textContent = account.email || "No email set";
    this.since.textContent = `Member since ${new Date(account.created_at).toLocaleDateString(undefined, {
      year: "numeric",
      month: "long",
    })}`;
    if (account.email) {
      this.emailInput.input.value = account.email;
    }
  }

  async saveEmail() {
    this.emailError.textContent = "";
    this.emailStatus.textContent = "";
    this.emailStatus.classList.remove("ok");
    if (!this.emailInput.valid) return;

    try {
      this.emailSaveButton.disabled = true;
      const response = await accountApi.updateEmail(this.emailInput.value);
      if (response.ok) {
        const account = await response.json();
        this.renderProfile(account);
        this.emailStatus.textContent = "Email updated.";
        this.emailStatus.classList.add("ok");
      } else if (response.status === 409) {
        this.emailError.textContent = "That email is already registered.";
      } else if (response.status === 400) {
        this.emailError.textContent = await response.text();
      } else {
        this.emailError.textContent = "Couldn't update your email — try again.";
      }
    } catch (error) {
      this.emailError.textContent = "Couldn't update your email — try again.";
    } finally {
      this.emailSaveButton.disabled = false;
    }
  }

  async savePassword() {
    this.passwordError.textContent = "";
    this.passwordStatus.textContent = "";
    this.passwordStatus.classList.remove("ok");
    if (!this.currentPasswordInput.valid || !this.newPasswordInput.valid) return;

    try {
      this.passwordSaveButton.disabled = true;
      const response = await accountApi.changePassword(this.currentPasswordInput.value, this.newPasswordInput.value);
      if (response.ok) {
        this.currentPasswordInput.input.value = "";
        this.newPasswordInput.input.value = "";
        this.passwordStatus.textContent = "Password changed.";
        this.passwordStatus.classList.add("ok");
      } else if (response.status === 401) {
        this.passwordError.textContent = "Current password is incorrect.";
      } else if (response.status === 400) {
        this.passwordError.textContent = await response.text();
      } else {
        this.passwordError.textContent = "Couldn't change your password — try again.";
      }
    } catch (error) {
      this.passwordError.textContent = "Couldn't change your password — try again.";
    } finally {
      this.passwordSaveButton.disabled = false;
    }
  }

  async refreshNotificationsState() {
    const state = await pushNotifications.state();
    this.renderNotificationsState(state);
  }

  renderNotificationsState(state) {
    if (state === "unsupported") {
      this.notificationsToggle.setAttribute("aria-checked", "false");
      this.notificationsToggle.disabled = true;
      this.notificationsStatus.textContent = "Not supported in this browser.";
      this.notificationsStatus.className = "account-page__notifications-status status-msg";
      return;
    }
    if (state === "denied") {
      this.notificationsToggle.setAttribute("aria-checked", "false");
      this.notificationsToggle.disabled = true;
      this.notificationsStatus.textContent = "Notifications are blocked in your browser settings.";
      this.notificationsStatus.className = "account-page__notifications-status status-msg warn";
      return;
    }
    const subscribed = state === "subscribed";
    this.notificationsToggle.disabled = false;
    this.notificationsToggle.setAttribute("aria-checked", subscribed ? "true" : "false");
    this.notificationsStatus.textContent = subscribed ? "You'll get alerts on this device." : "";
    this.notificationsStatus.className = subscribed
      ? "account-page__notifications-status status-msg ok"
      : "account-page__notifications-status status-msg";
  }

  async toggleNotifications() {
    this.notificationsToggle.disabled = true;
    const subscribed = this.notificationsToggle.getAttribute("aria-checked") === "true";
    const result = subscribed ? await pushNotifications.unsubscribe() : await pushNotifications.subscribe();
    if (!result.ok && result.denied) {
      this.renderNotificationsState("denied");
      return;
    }
    if (!result.ok) {
      this.notificationsToggle.disabled = false;
      this.notificationsStatus.textContent = "Couldn't update notifications — try again.";
      this.notificationsStatus.className = "account-page__notifications-status status-msg warn";
      return;
    }
    await this.refreshNotificationsState();
  }

  confirmRegenerateApiKey() {
    confirmDialogManager.confirm({
      headline: "Regenerate API key?",
      body: "Your old key will stop working immediately.",
      yesCallback: this.regenerateApiKey.bind(this),
      noCallback: () => {},
    });
  }

  async regenerateApiKey() {
    this.apiKeyError.textContent = "";
    this.apiKeyStatus.textContent = "";
    this.apiKeyStatus.classList.remove("ok");
    try {
      this.apiKeyRegenerateButton.disabled = true;
      const response = await accountApi.regenerateApiKey();
      if (response.ok) {
        const { api_key } = await response.json();
        this.renderApiKey(api_key);
        this.apiKeyStatus.textContent = "New key generated.";
        this.apiKeyStatus.classList.add("ok");
      } else {
        this.apiKeyError.textContent = "Couldn't regenerate your API key — try again.";
      }
    } catch (error) {
      this.apiKeyError.textContent = "Couldn't regenerate your API key — try again.";
    } finally {
      this.apiKeyRegenerateButton.disabled = false;
    }
  }

  confirmDeleteAccount() {
    confirmDialogManager.confirm({
      headline: "Delete account?",
      body: "This permanently removes your account, linked characters, and sessions. This can't be undone.",
      yesCallback: this.deleteAccount.bind(this),
      noCallback: () => {},
    });
  }

  async deleteAccount() {
    const response = await accountApi.deleteAccount();
    if (response.ok) {
      window.history.pushState("", "", "/account/login");
    }
  }
}

customElements.define("account-page", AccountPage);
