import { adminViewSession } from "./admin-view-session";

class Storage {
  // Claiming a real member session and an admin's read-only view of someone else's group are
  // mutually exclusive - without this, a lingering `adminViewSession` (sessionStorage survives
  // reloads/navigation within the tab) would keep outranking these fresh credentials in
  // `app-initializer`'s startup check, so storing your own group here has to end any admin view
  // in progress.
  storeGroup(groupName, groupToken) {
    adminViewSession.clear();
    localStorage.setItem("groupName", groupName);
    localStorage.setItem("groupToken", groupToken);
  }

  getGroup() {
    return {
      groupName: localStorage.getItem("groupName"),
      groupToken: localStorage.getItem("groupToken"),
    };
  }

  clearGroup() {
    localStorage.removeItem("groupName");
    localStorage.removeItem("groupToken");
  }
}

const storage = new Storage();

export { storage };
