import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { pushNotifications } from "../data/push-notifications";

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

function initials(username) {
  if (!username) return "?";
  return username.slice(0, 2).toUpperCase();
}

/**
 * Account-level dashboard: profile info, username/password change, and a link out to
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
    this.usernameDisplay = this.querySelector(".account-page__username-display");
    this.since = this.querySelector(".account-page__since");

    this.discordNameValue = this.querySelector(".account-page__discord-name-value");
    this.discordBadge = this.querySelector(".account-page__discord-badge");
    this.discordConnectButton = this.querySelector(".account-page__discord-connect");
    this.discordStatus = this.querySelector(".account-page__discord-status");
    this.eventListener(this.discordConnectButton, "click", this.connectDiscord.bind(this));

    this.usernameInput = this.querySelector(".account-page__username");
    this.usernameInput.validators = [usernameValidator];
    this.usernameSaveButton = this.querySelector(".account-page__username-save");
    this.usernameError = this.querySelector(".account-page__username-error");
    this.usernameStatus = this.querySelector(".account-page__username-status");
    this.eventListener(this.usernameSaveButton, "click", this.saveUsername.bind(this));

    this.newPasswordInput = this.querySelector(".account-page__new-password");
    this.newPasswordInput.validators = [passwordValidator];
    this.passwordSaveButton = this.querySelector(".account-page__password-save");
    this.passwordError = this.querySelector(".account-page__password-error");
    this.passwordStatus = this.querySelector(".account-page__password-status");
    this.passwordForcedHint = this.querySelector(".account-page__password-forced-hint");
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
    this.showDiscordLinkedStatusIfPresent();
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
    this.avatar.textContent = initials(account.username);
    this.renderDiscordCard(account);
    this.usernameDisplay.textContent = account.username || "No username set";
    this.since.textContent = `Member since ${new Date(account.created_at).toLocaleDateString(undefined, {
      year: "numeric",
      month: "long",
    })}`;
    if (account.username) {
      this.usernameInput.input.value = account.username;
    }
    this.passwordForcedHint.hidden = !account.must_change_password;
  }

  renderDiscordCard(account) {
    const linked = Boolean(account.discord_name);
    this.discordNameValue.textContent = linked ? account.discord_name : "Not connected";
    this.discordNameValue.classList.toggle("account-page__discord-name-value--unlinked", !linked);
    this.discordBadge.hidden = !linked;
    this.discordConnectButton.hidden = linked;
  }

  showDiscordLinkedStatusIfPresent() {
    if (!sessionStorage.getItem("discordLinked")) return;
    sessionStorage.removeItem("discordLinked");
    this.discordStatus.textContent = "Discord connected.";
    this.discordStatus.classList.add("ok");
  }

  async connectDiscord() {
    this.discordStatus.textContent = "";
    this.discordStatus.classList.remove("ok");
    this.discordConnectButton.disabled = true;
    const response = await accountApi.discordLinkRedirect();
    if (!response.ok) {
      this.discordStatus.textContent = "Couldn't connect Discord — try again.";
      this.discordConnectButton.disabled = false;
    }
    // On success the page navigates away to Discord, so there's nothing left to reset here.
  }

  async saveUsername() {
    this.usernameError.textContent = "";
    this.usernameStatus.textContent = "";
    this.usernameStatus.classList.remove("ok");
    if (!this.usernameInput.valid) return;

    try {
      this.usernameSaveButton.disabled = true;
      const response = await accountApi.updateUsername(this.usernameInput.value);
      if (response.ok) {
        const account = await response.json();
        this.renderProfile(account);
        this.usernameStatus.textContent = "Username updated.";
        this.usernameStatus.classList.add("ok");
      } else if (response.status === 409) {
        this.usernameError.textContent = "That username is already registered.";
      } else if (response.status === 400) {
        this.usernameError.textContent = await response.text();
      } else {
        this.usernameError.textContent = "Couldn't update your username — try again.";
      }
    } catch (error) {
      this.usernameError.textContent = "Couldn't update your username — try again.";
    } finally {
      this.usernameSaveButton.disabled = false;
    }
  }

  async savePassword() {
    this.passwordError.textContent = "";
    this.passwordStatus.textContent = "";
    this.passwordStatus.classList.remove("ok");
    if (!this.newPasswordInput.valid) return;

    try {
      this.passwordSaveButton.disabled = true;
      const response = await accountApi.changePassword(this.newPasswordInput.value);
      if (response.ok) {
        this.newPasswordInput.input.value = "";
        this.passwordStatus.textContent = "Password changed.";
        this.passwordStatus.classList.add("ok");
        this.passwordForcedHint.hidden = true;
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
