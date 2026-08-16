import { adminApi } from "./admin-api";
import { adminStorage } from "./admin-storage";

// Confirms a stored admin token is present and still valid, redirecting to
// the admin login page otherwise. Returns true if the caller should proceed.
async function requireAdmin() {
  const token = adminStorage.getAdminToken();
  if (!token) {
    window.history.pushState("", "", "/admin/login");
    return false;
  }

  adminApi.setCredentials(token);
  const response = await adminApi.amILoggedIn();
  if (!response.ok) {
    adminStorage.clearAdminToken();
    window.history.pushState("", "", "/admin/login");
    return false;
  }

  return true;
}

export { requireAdmin };
