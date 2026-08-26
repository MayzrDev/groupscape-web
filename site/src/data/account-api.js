import { accountStorage } from "./account-storage";

class AccountApi {
  constructor() {
    this.baseUrl = "/api/account";
  }

  get meUrl() {
    return `${this.baseUrl}/me`;
  }

  get charactersUrl() {
    return `${this.baseUrl}/characters`;
  }

  get linkCharacterUrl() {
    return `${this.baseUrl}/characters/link`;
  }

  get linkCharacterToGroupUrl() {
    return `${this.baseUrl}/characters/link-group`;
  }

  leaveGroupUrl(characterId) {
    return `${this.baseUrl}/characters/${characterId}/leave-group`;
  }

  characterUrl(characterId) {
    return `${this.baseUrl}/characters/${characterId}`;
  }

  confirmCharacterUrl(characterId) {
    return `${this.baseUrl}/characters/${characterId}/confirm`;
  }

  pendingCharacterUrl(characterId) {
    return `${this.baseUrl}/characters/${characterId}/pending`;
  }

  characterPortraitUrl(characterId) {
    return `${this.baseUrl}/characters/${characterId}/portrait`;
  }

  get apiKeyUrl() {
    return `${this.baseUrl}/api-key`;
  }

  get registerUrl() {
    return `${this.baseUrl}/register`;
  }

  get loginUrl() {
    return `${this.baseUrl}/login`;
  }

  get discordRedirectUrl() {
    return `${this.baseUrl}/discord/redirect`;
  }

  get discordLinkRedirectUrl() {
    return `${this.baseUrl}/discord/link/redirect`;
  }

  /**
   * Discord's callback lands the browser back on `/#token=...` (or `#error=...` /
   * `#discord_linked=1`) as a URL fragment - see `handleDiscordCallback` for why it's a fragment
   * rather than a query string. Reads whichever of those three outcomes is present and always
   * clears the fragment, so this is safe to call unconditionally on every page load.
   */
  handleDiscordCallback() {
    const params = new URLSearchParams(window.location.hash.slice(1));
    const token = params.get("token");
    const error = params.get("error");
    const linked = params.get("discord_linked");
    if (!token && !error && !linked) return { status: "none" };

    if (token) {
      accountStorage.storeAccountToken(token);
      const apiKey = params.get("api_key");
      if (apiKey) {
        sessionStorage.setItem("freshApiKey", apiKey);
      }
      window.history.replaceState("", "", "/welcome");
      return { status: "logged_in" };
    }

    if (linked) {
      window.history.replaceState("", "", "/account");
      return { status: "linked" };
    }

    window.history.replaceState("", "", "/account/login");
    return { status: "error", error };
  }

  get usernameUrl() {
    return `${this.baseUrl}/username`;
  }

  get passwordUrl() {
    return `${this.baseUrl}/password`;
  }

  get deleteAccountUrl() {
    return this.baseUrl;
  }

  get vapidPublicKeyUrl() {
    return `${this.baseUrl}/push/vapid-public-key`;
  }

  get pushSubscribeUrl() {
    return `${this.baseUrl}/push/subscribe`;
  }

  /**
   * Every authed-account fetch routes through here so the forced-password-change gate is
   * handled in exactly one place: the backend returns 403 (`MustChangePasswordError`) for any
   * authed account endpoint other than `/me` and `/password` while `must_change_password` is
   * set, and this redirects to the account page (whose password card is always visible) instead
   * of making every caller special-case that status.
   */
  async authedFetch(url, options = {}) {
    const accountToken = accountStorage.getAccountToken();
    if (!accountToken) {
      return { ok: false, status: 401 };
    }
    const response = await fetch(url, {
      ...options,
      headers: { ...(options.headers ?? {}), Authorization: accountToken },
    });
    if (response.status === 403 && url !== this.passwordUrl && url !== this.meUrl) {
      window.history.pushState("", "", "/account");
    }
    return response;
  }

  async register(username, password) {
    const response = await fetch(this.registerUrl, {
      body: JSON.stringify({ username, password }),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
    if (response.ok) {
      const authenticated = await response.json();
      accountStorage.storeAccountToken(authenticated.token);
      if (authenticated.api_key) {
        sessionStorage.setItem("freshApiKey", authenticated.api_key);
      }
    }
    return response;
  }

  async login(username, password) {
    const response = await fetch(this.loginUrl, {
      body: JSON.stringify({ username, password }),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
    if (response.ok) {
      const authenticated = await response.json();
      accountStorage.storeAccountToken(authenticated.token);
    }
    return response;
  }

  async me() {
    return this.authedFetch(this.meUrl);
  }

  async listCharacters() {
    return this.authedFetch(this.charactersUrl);
  }

  async linkCharacter(accountHash, rsn) {
    return this.authedFetch(this.linkCharacterUrl, {
      body: JSON.stringify({ account_hash: accountHash, rsn }),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
  }

  async linkCharacterToGroup(characterId, groupName, groupToken) {
    return this.authedFetch(this.linkCharacterToGroupUrl, {
      body: JSON.stringify({ character_id: Number(characterId), group_name: groupName, group_token: groupToken }),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
  }

  async leaveGroup(characterId) {
    return this.authedFetch(this.leaveGroupUrl(characterId), { method: "POST" });
  }

  async unlinkCharacter(characterId) {
    return this.authedFetch(this.characterUrl(characterId), { method: "DELETE" });
  }

  async confirmCharacter(characterId) {
    return this.authedFetch(this.confirmCharacterUrl(characterId), { method: "POST" });
  }

  async removePendingCharacter(characterId) {
    return this.authedFetch(this.pendingCharacterUrl(characterId), { method: "DELETE" });
  }

  async getCharacterPortrait(characterId) {
    const response = await this.authedFetch(this.characterPortraitUrl(characterId));
    if (!response.ok) {
      return null;
    }
    return response.arrayBuffer();
  }

  /**
   * Fetches a signed Discord authorize URL bound to the logged-in account (see
   * `discord_link_redirect` on the server), then navigates the whole page there - Discord's own
   * redirect can't carry an Authorization header, so the account id has to ride along inside the
   * OAuth `state` instead of being read from a header when Discord calls back.
   */
  async discordLinkRedirect() {
    const response = await this.authedFetch(this.discordLinkRedirectUrl);
    if (!response.ok) return response;
    const { redirect_url } = await response.json();
    window.location.href = redirect_url;
    return response;
  }

  async regenerateApiKey() {
    return this.authedFetch(this.apiKeyUrl, { method: "POST" });
  }

  async updateUsername(username) {
    return this.authedFetch(this.usernameUrl, {
      body: JSON.stringify({ username }),
      headers: { "Content-Type": "application/json" },
      method: "PUT",
    });
  }

  async changePassword(newPassword) {
    return this.authedFetch(this.passwordUrl, {
      body: JSON.stringify({ new_password: newPassword }),
      headers: { "Content-Type": "application/json" },
      method: "PUT",
    });
  }

  async vapidPublicKey() {
    const response = await fetch(this.vapidPublicKeyUrl);
    return response;
  }

  async subscribePush(subscription) {
    return this.authedFetch(this.pushSubscribeUrl, {
      body: JSON.stringify(subscription.toJSON()),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
  }

  async unsubscribePush(endpoint) {
    return this.authedFetch(this.pushSubscribeUrl, {
      body: JSON.stringify({ endpoint }),
      headers: { "Content-Type": "application/json" },
      method: "DELETE",
    });
  }

  async deleteAccount() {
    const response = await this.authedFetch(this.deleteAccountUrl, { method: "DELETE" });
    if (response.ok) {
      accountStorage.clearAccountToken();
    }
    return response;
  }
}

const accountApi = new AccountApi();

export { accountApi };
