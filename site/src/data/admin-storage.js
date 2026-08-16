class AdminStorage {
  storeAdminToken(adminToken) {
    localStorage.setItem("adminToken", adminToken);
  }

  getAdminToken() {
    return localStorage.getItem("adminToken");
  }

  clearAdminToken() {
    localStorage.removeItem("adminToken");
  }
}

const adminStorage = new AdminStorage();

export { adminStorage };
