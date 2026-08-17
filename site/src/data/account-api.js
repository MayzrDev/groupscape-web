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

  async register(email, password) {
    const response = await fetch(this.registerUrl, {
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
