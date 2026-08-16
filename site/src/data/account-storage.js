class AccountStorage {
  storeAccountToken(accountToken) {
    localStorage.setItem("accountToken", accountToken);
  }

  getAccountToken() {
    return localStorage.getItem("accountToken");
  }

  clearAccountToken() {
    localStorage.removeItem("accountToken");
  }
}

const accountStorage = new AccountStorage();

export { accountStorage };
