class AdminApi {
  constructor() {
    this.baseUrl = "/api/admin";
  }

  setCredentials(adminToken) {
    this.adminToken = adminToken;
  }

  get authHeaders() {
    return { Authorization: `Bearer ${this.adminToken}` };
  }

  async amILoggedIn() {
    return fetch(`${this.baseUrl}/am-i-logged-in`, { headers: this.authHeaders });
  }

  async listGroups(search, page, pageSize) {
    const params = new URLSearchParams({ page, page_size: pageSize });
    if (search) params.set("search", search);
    const response = await fetch(`${this.baseUrl}/groups?${params}`, { headers: this.authHeaders });
    return response;
  }

  async getGroup(groupId) {
    const response = await fetch(`${this.baseUrl}/groups/${groupId}`, { headers: this.authHeaders });
    return response;
  }

  async suspendGroup(groupId, reason) {
    return this.moderateGroup(groupId, "suspend", reason);
  }

  async banGroup(groupId, reason) {
    return this.moderateGroup(groupId, "ban", reason);
  }

  async unbanGroup(groupId) {
    const response = await fetch(`${this.baseUrl}/groups/${groupId}/unban`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async moderateGroup(groupId, action, reason) {
    const response = await fetch(`${this.baseUrl}/groups/${groupId}/${action}`, {
      method: "POST",
      headers: { ...this.authHeaders, "Content-Type": "application/json" },
      body: JSON.stringify({ reason: reason || null }),
    });
    return response;
  }

  async deleteGroup(groupId) {
    const response = await fetch(`${this.baseUrl}/groups/${groupId}`, {
      method: "DELETE",
      headers: this.authHeaders,
    });
    return response;
  }

  async listFeatureFlags() {
    const response = await fetch(`${this.baseUrl}/feature-flags`, { headers: this.authHeaders });
    return response;
  }

  async setFeatureFlag(flagKey, enabled, description) {
    const response = await fetch(`${this.baseUrl}/feature-flags/${encodeURIComponent(flagKey)}`, {
      method: "PUT",
      headers: { ...this.authHeaders, "Content-Type": "application/json" },
      body: JSON.stringify({ enabled, description: description || null }),
    });
    return response;
  }

  async listAuditLog(page, pageSize) {
    const params = new URLSearchParams({ page, page_size: pageSize });
    const response = await fetch(`${this.baseUrl}/audit-log?${params}`, { headers: this.authHeaders });
    return response;
  }

  async accountsSummary() {
    const response = await fetch(`${this.baseUrl}/accounts/summary`, { headers: this.authHeaders });
    return response;
  }
}

const adminApi = new AdminApi();

export { adminApi };
