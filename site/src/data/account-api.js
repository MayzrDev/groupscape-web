import { accountStorage } from "./account-storage";

class AccountApi {
  constructor() {
    this.baseUrl = "/api/account";
  }

  get meUrl() {
    return `${this.baseUrl}/me`;
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
}

const accountApi = new AccountApi();

export { accountApi };
