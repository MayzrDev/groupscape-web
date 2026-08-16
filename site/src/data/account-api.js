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
