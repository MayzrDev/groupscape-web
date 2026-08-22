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

  async listAccounts(search, status, groupId, page, pageSize) {
    const params = new URLSearchParams({ page, page_size: pageSize });
    if (search) params.set("search", search);
    if (status) params.set("status", status);
    if (groupId) params.set("group_id", groupId);
    const response = await fetch(`${this.baseUrl}/accounts?${params}`, { headers: this.authHeaders });
    return response;
  }

  async getAccount(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}`, { headers: this.authHeaders });
    return response;
  }

  async resetAccountPassword(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/reset-password`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async setAccountStatus(accountId, status) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/status`, {
      method: "POST",
      headers: { ...this.authHeaders, "Content-Type": "application/json" },
      body: JSON.stringify({ status }),
    });
    return response;
  }

  async softDeleteAccount(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/soft-delete`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async hardDeleteAccount(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/hard-delete`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async listAccountSessions(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/sessions`, {
      headers: this.authHeaders,
    });
    return response;
  }

  async revokeAccountSession(accountId, sessionId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/sessions/${sessionId}/revoke`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async revokeAllAccountSessions(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/sessions/revoke-all`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async setAccountEmail(accountId, email) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/email`, {
      method: "POST",
      headers: { ...this.authHeaders, "Content-Type": "application/json" },
      body: JSON.stringify({ email }),
    });
    return response;
  }

  async clearAccountLockout(accountId) {
    const response = await fetch(`${this.baseUrl}/accounts/${accountId}/clear-lockout`, {
      method: "POST",
      headers: this.authHeaders,
    });
    return response;
  }

  async search(q) {
    const params = new URLSearchParams({ q });
    const response = await fetch(`${this.baseUrl}/search?${params}`, { headers: this.authHeaders });
    return response;
  }

  async dashboard() {
    const response = await fetch(`${this.baseUrl}/dashboard`, { headers: this.authHeaders });
    return response;
  }
}

const adminApi = new AdminApi();

export { adminApi };
