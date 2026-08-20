import { pubsub } from "./pubsub";
import { utility } from "../utility";
import { groupData } from "./group-data";
import { exampleData } from "./example-data";
import { accountStorage } from "./account-storage";

class Api {
  constructor() {
    this.baseUrl = "/api";
    this.createGroupUrl = `${this.baseUrl}/create-group`;
    this.exampleDataEnabled = false;
    this.enabled = false;
  }

  // Permission-gated group actions (member removal, group settings) need to identify the
  // *account* performing them, not just the group - the shared group token proves membership
  // in the group but carries no account identity. Undefined when no account is logged in;
  // the server rejects those requests with 401 rather than silently no-op'ing.
  get accountAuthHeaders() {
    const accountToken = accountStorage.getAccountToken();
    return accountToken ? { "X-Account-Authorization": accountToken } : {};
  }

  get getGroupDataUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-group-data`;
  }

  get deleteMemberUrl() {
    return `${this.baseUrl}/group/${this.groupName}/delete-group-member`;
  }

  get blockMemberUrl() {
    return `${this.baseUrl}/group/${this.groupName}/block-group-member`;
  }

  get unblockMemberUrl() {
    return `${this.baseUrl}/group/${this.groupName}/unblock-group-member`;
  }

  get blockedMembersUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-blocked-members`;
  }

  get canKickMembersUrl() {
    return `${this.baseUrl}/group/${this.groupName}/can-kick-members`;
  }

  get amILoggedInUrl() {
    return `${this.baseUrl}/group/${this.groupName}/am-i-logged-in`;
  }

  get groupPermissionsUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-group-permissions`;
  }

  get myPermissionsUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-my-permissions`;
  }

  get updateGroupPermissionsUrl() {
    return `${this.baseUrl}/group/${this.groupName}/update-group-permissions`;
  }

  get renameGroupUrl() {
    return `${this.baseUrl}/group/${this.groupName}/rename-group`;
  }

  get rerollGroupTokenUrl() {
    return `${this.baseUrl}/group/${this.groupName}/reroll-group-token`;
  }

  get deleteGroupUrl() {
    return `${this.baseUrl}/group/${this.groupName}/delete-group`;
  }

  get gePricesUrl() {
    return `${this.baseUrl}/ge-prices`;
  }

  get skillDataUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-skill-data`;
  }

  get leaderboardUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-leaderboard`;
  }

  get captchaEnabledUrl() {
    return `${this.baseUrl}/captcha-enabled`;
  }

  get portraitUrl() {
    return `${this.baseUrl}/group/${this.groupName}/portrait`;
  }

  get activityEventsUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-activity-events`;
  }

  get lootSummaryUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-loot-summary`;
  }

  get lootSplitUrl() {
    return `${this.baseUrl}/group/${this.groupName}/get-loot-split`;
  }

  setCredentials(groupName, groupToken) {
    this.groupName = groupName;
    this.groupToken = groupToken;
  }

  async restart() {
    const groupName = this.groupName;
    const groupToken = this.groupToken;
    await this.enable(groupName, groupToken);
  }

  async enable(groupName, groupToken) {
    await this.disable();
    this.nextCheck = new Date(0).toISOString();
    this.setCredentials(groupName, groupToken);

    if (!this.enabled) {
      this.enabled = true;
      // getGroupInterval is a Promise so we can make sure this method does not leak
      // any intervals with multiple calls to .enable(). This could be possible because of
      // the wait for the item and quest data loads before we create the interval.
      this.getGroupInterval = pubsub.waitForAllEvents("item-data-loaded", "quest-data-loaded").then(() => {
        return utility.callOnInterval(this.getGroupData.bind(this), 1000);
      });
    }

    await this.getGroupInterval;
  }

  async disable() {
    this.enabled = false;
    this.groupName = undefined;
    this.groupToken = undefined;
    groupData.members = new Map();
    groupData.groupItems = {};
    groupData.filters = [""];
    if (this.getGroupInterval) {
      window.clearInterval(await this.getGroupInterval);
    }
  }

  async getGroupData() {
    const nextCheck = this.nextCheck;

    if (this.exampleDataEnabled) {
      const newGroupData = exampleData.getGroupData();
      groupData.update(newGroupData);
      pubsub.publish("get-group-data", groupData);
    } else {
      const response = await fetch(`${this.getGroupDataUrl}?from_time=${nextCheck}`, {
        headers: {
          Authorization: this.groupToken,
        },
      });
      if (!response.ok) {
        if (response.status === 401) {
          await this.disable();
          window.history.pushState("", "", "/");
          pubsub.publish("get-group-data");
        }
        return;
      }

      const newGroupData = await response.json();
      this.nextCheck = groupData.update(newGroupData).toISOString();
      pubsub.publish("get-group-data", groupData);
    }
  }

  async createGroup(groupName, memberNames, captchaResponse) {
    const response = await fetch(this.createGroupUrl, {
      body: JSON.stringify({ name: groupName, member_names: memberNames, captcha_response: captchaResponse }),
      headers: {
        "Content-Type": "application/json",
      },
      method: "POST",
    });

    return response;
  }

  async removeMember(memberName) {
    const response = await fetch(this.deleteMemberUrl, {
      body: JSON.stringify({ name: memberName }),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "DELETE",
    });

    return response;
  }

  async blockMember(memberName) {
    const response = await fetch(this.blockMemberUrl, {
      body: JSON.stringify({ name: memberName }),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "POST",
    });

    return response;
  }

  async unblockMember(memberName) {
    const response = await fetch(this.unblockMemberUrl, {
      body: JSON.stringify({ name: memberName }),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "POST",
    });

    return response;
  }

  async getBlockedMembers() {
    const response = await fetch(this.blockedMembersUrl, {
      headers: {
        Authorization: this.groupToken,
      },
    });

    return response;
  }

  // The endpoint itself is the permission gate (401/403 when this account can't kick), so the
  // site treats "ok" as "show the remove/block controls" rather than duplicating the permission
  // check client-side.
  async canKickMembers() {
    const response = await fetch(this.canKickMembersUrl, {
      headers: {
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
    });

    return response.ok;
  }

  async getGroupPermissions() {
    const response = await fetch(this.groupPermissionsUrl, {
      headers: {
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
    });

    return response;
  }

  async getMyPermissions() {
    const response = await fetch(this.myPermissionsUrl, {
      headers: {
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
    });

    return response;
  }

  async updateGroupPermissions(accountId, patch) {
    const response = await fetch(this.updateGroupPermissionsUrl, {
      body: JSON.stringify({ account_id: accountId, ...patch }),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "PUT",
    });

    return response;
  }

  async renameGroup(newName) {
    const response = await fetch(this.renameGroupUrl, {
      body: JSON.stringify({ new_name: newName }),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "PUT",
    });

    return response;
  }

  async rerollGroupToken() {
    const response = await fetch(this.rerollGroupTokenUrl, {
      headers: {
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "POST",
    });

    return response;
  }

  async deleteGroup() {
    const response = await fetch(this.deleteGroupUrl, {
      headers: {
        Authorization: this.groupToken,
        ...this.accountAuthHeaders,
      },
      method: "DELETE",
    });

    return response;
  }

  async amILoggedIn() {
    const response = await fetch(this.amILoggedInUrl, {
      headers: { Authorization: this.groupToken },
    });

    return response;
  }

  async getGePrices() {
    const response = await fetch(this.gePricesUrl);
    return response;
  }

  async getSkillData(period) {
    if (this.exampleDataEnabled) {
      const skillData = exampleData.getSkillData(period, groupData);
      return skillData;
    } else {
      const response = await fetch(`${this.skillDataUrl}?period=${period}`, {
        headers: {
          Authorization: this.groupToken,
        },
      });
      return response.json();
    }
  }

  async getLeaderboard(metric, window, boss) {
    const query = new URLSearchParams({ metric, window });
    if (boss) query.set("boss", boss);
    const response = await fetch(`${this.leaderboardUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.groupToken,
      },
    });
    if (!response.ok) {
      return { metric, window, boss: boss || null, available_bosses: [], entries: [] };
    }
    return response.json();
  }

  async getCaptchaEnabled() {
    const response = await fetch(this.captchaEnabledUrl);
    return response.json();
  }

  async getActivityEvents({ memberName, eventType, before, limit } = {}) {
    const query = new URLSearchParams();
    if (memberName) query.set("member_name", memberName);
    if (eventType) query.set("event_type", eventType);
    if (before) query.set("before", before);
    if (limit) query.set("limit", limit);

    const response = await fetch(`${this.activityEventsUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.groupToken,
      },
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  }

  async getLootSummary({ memberName, sort } = {}) {
    const query = new URLSearchParams();
    if (memberName) query.set("member_name", memberName);
    if (sort) query.set("sort", sort);

    const response = await fetch(`${this.lootSummaryUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.groupToken,
      },
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  }

  async getLootSplit({ since, until } = {}) {
    const query = new URLSearchParams();
    if (since) query.set("since", since);
    if (until) query.set("until", until);

    const response = await fetch(`${this.lootSplitUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.groupToken,
      },
    });
    if (!response.ok) {
      return null;
    }
    return response.json();
  }

  async getPortrait(memberName) {
    const response = await fetch(`${this.portraitUrl}/${encodeURIComponent(memberName)}`, {
      headers: {
        Authorization: this.groupToken,
      },
    });
    if (!response.ok) {
      return null;
    }
    return response.arrayBuffer();
  }
}

const api = new Api();

export { api };
