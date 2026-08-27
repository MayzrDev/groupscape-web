// Lets a global admin open the real (live, full-featured) group dashboard as a read-only
// observer, reusing every `/group/*` page unmodified. Kept in sessionStorage rather than
// localStorage so a view doesn't linger past the tab that started it, and kept entirely
// separate from the normal member `storage.getGroup()` credentials so the two flows can never
// bleed into each other.
class AdminViewSession {
  start(groupId, groupName, adminToken) {
    sessionStorage.setItem("adminViewGroupId", groupId);
    sessionStorage.setItem("adminViewGroupName", groupName);
    sessionStorage.setItem("adminViewToken", adminToken);
  }

  get() {
    const groupId = sessionStorage.getItem("adminViewGroupId");
    const groupName = sessionStorage.getItem("adminViewGroupName");
    const adminToken = sessionStorage.getItem("adminViewToken");
    if (!groupId || !adminToken) return null;
    return { groupId, groupName, adminToken };
  }

  clear() {
    sessionStorage.removeItem("adminViewGroupId");
    sessionStorage.removeItem("adminViewGroupName");
    sessionStorage.removeItem("adminViewToken");
  }
}

const adminViewSession = new AdminViewSession();

export { adminViewSession };
