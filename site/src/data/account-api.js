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

  get emailUrl() {
    return `${this.baseUrl}/email`;
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

  async register(email, password) {
    const response = await fetch(this.registerUrl, {
      body: JSON.stringify({ email, password }),
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

  async login(email, password) {
    const response = await fetch(this.loginUrl, {
      body: JSON.stringify({ email, password }),
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
    const accountToken = accountStorage.getAccountToken();
    if (!accountToken) {
      return { ok: false, status: 401 };
    }
    const response = await fetch(this.meUrl, {
      headers: { Authorization: accountToken },
    });
    return response;
  }

  async listCharacters() {
    const accountToken = accountStorage.getAccountToken();
    if (!accountToken) {
      return { ok: false, status: 401 };
    }
    const response = await fetch(this.charactersUrl, {
      headers: { Authorization: accountToken },
    });
    return response;
  }

  async linkCharacter(accountHash, rsn) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.linkCharacterUrl, {
      body: JSON.stringify({ account_hash: accountHash, rsn }),
      headers: {
        "Content-Type": "application/json",
        Authorization: accountToken,
      },
      method: "POST",
    });
    return response;
  }

  async linkCharacterToGroup(characterId, groupName, groupToken) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.linkCharacterToGroupUrl, {
      body: JSON.stringify({ character_id: Number(characterId), group_name: groupName, group_token: groupToken }),
      headers: {
        "Content-Type": "application/json",
        Authorization: accountToken,
      },
      method: "POST",
    });
    return response;
  }

  async leaveGroup(characterId) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.leaveGroupUrl(characterId), {
      headers: { Authorization: accountToken },
      method: "POST",
    });
    return response;
  }

  async unlinkCharacter(characterId) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.characterUrl(characterId), {
      headers: { Authorization: accountToken },
      method: "DELETE",
    });
    return response;
  }

  async confirmCharacter(characterId) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.confirmCharacterUrl(characterId), {
      headers: { Authorization: accountToken },
      method: "POST",
    });
    return response;
  }

  async removePendingCharacter(characterId) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.pendingCharacterUrl(characterId), {
      headers: { Authorization: accountToken },
      method: "DELETE",
    });
    return response;
  }

  async getCharacterPortrait(characterId) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.characterPortraitUrl(characterId), {
      headers: { Authorization: accountToken },
    });
    if (!response.ok) {
      return null;
    }
    return response.arrayBuffer();
  }

  async regenerateApiKey() {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.apiKeyUrl, {
      headers: { Authorization: accountToken },
      method: "POST",
    });
    return response;
  }

  async updateEmail(email) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.emailUrl, {
      body: JSON.stringify({ email }),
      headers: {
        "Content-Type": "application/json",
        Authorization: accountToken,
      },
      method: "PUT",
    });
    return response;
  }

  async changePassword(currentPassword, newPassword) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.passwordUrl, {
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
      headers: {
        "Content-Type": "application/json",
        Authorization: accountToken,
      },
      method: "PUT",
    });
    return response;
  }

  async vapidPublicKey() {
    const response = await fetch(this.vapidPublicKeyUrl);
    return response;
  }

  async subscribePush(subscription) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.pushSubscribeUrl, {
      body: JSON.stringify(subscription.toJSON()),
      headers: {
        "Content-Type": "application/json",
        Authorization: accountToken,
      },
      method: "POST",
    });
    return response;
  }

  async unsubscribePush(endpoint) {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.pushSubscribeUrl, {
      body: JSON.stringify({ endpoint }),
      headers: {
        "Content-Type": "application/json",
        Authorization: accountToken,
      },
      method: "DELETE",
    });
    return response;
  }

  async deleteAccount() {
    const accountToken = accountStorage.getAccountToken();
    const response = await fetch(this.deleteAccountUrl, {
      headers: { Authorization: accountToken },
      method: "DELETE",
    });
    if (response.ok) {
      accountStorage.clearAccountToken();
    }
    return response;
  }
}

const accountApi = new AccountApi();

export { accountApi };
