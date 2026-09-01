import { pubsub } from "./pubsub";
import { utility } from "../utility";
import { groupData } from "./group-data";
import { accountStorage } from "./account-storage";
import { adminViewSession } from "./admin-view-session";

class Api {
  constructor() {
    this.baseUrl = "/api";
    this.createGroupUrl = `${this.baseUrl}/create-group`;
    this.enabled = false;
    this.adminView = false;
  }

  // Permission-gated group actions (member removal, group settings) need to identify the
  // *account* performing them, not just the group - the shared group token proves membership
  // in the group but carries no account identity. Undefined when no account is logged in;
  // the server rejects those requests with 401 rather than silently no-op'ing. Not applicable
  // in admin-view mode, where there's no account behind the request at all.
  get accountAuthHeaders() {
    if (this.adminView) return {};
    const accountToken = accountStorage.getAccountToken();
    return accountToken ? { "X-Account-Authorization": accountToken } : {};
  }

  // Every group-scoped request goes to a group-token-authed `/api/group/{name}` route normally,
  // or, in admin-view mode, to the admin-bearer-authed `/api/admin/group-view/{id}` route
  // instead - which only mounts read handlers, so mutating calls fail closed with a 404 rather
  // than silently no-op'ing. This is the single seam that makes every dashboard page reusable
  // read-only for an admin without touching the pages themselves.
  get groupScopeUrl() {
    if (this.adminView) {
      return `${this.baseUrl}/admin/group-view/${this.adminViewGroupId}`;
    }
    return `${this.baseUrl}/group/${this.groupName}`;
  }

  get authHeader() {
    return this.adminView ? `Bearer ${this.adminToken}` : this.groupToken;
  }

  get getGroupDataUrl() {
    return `${this.groupScopeUrl}/get-group-data`;
  }

  get deleteMemberUrl() {
    return `${this.groupScopeUrl}/delete-group-member`;
  }

  get blockMemberUrl() {
    return `${this.groupScopeUrl}/block-group-member`;
  }

  get unblockMemberUrl() {
    return `${this.groupScopeUrl}/unblock-group-member`;
  }

  get blockedMembersUrl() {
    return `${this.groupScopeUrl}/get-blocked-members`;
  }

  get canKickMembersUrl() {
    return `${this.groupScopeUrl}/can-kick-members`;
  }

  get amILoggedInUrl() {
    return `${this.groupScopeUrl}/am-i-logged-in`;
  }

  get groupPermissionsUrl() {
    return `${this.groupScopeUrl}/get-group-permissions`;
  }

  get myPermissionsUrl() {
    return `${this.groupScopeUrl}/get-my-permissions`;
  }

  get updateGroupPermissionsUrl() {
    return `${this.groupScopeUrl}/update-group-permissions`;
  }

  get updateMemberColorUrl() {
    return `${this.groupScopeUrl}/update-member-color`;
  }

  get discordSettingsUrl() {
    return `${this.groupScopeUrl}/get-discord-settings`;
  }

  get updateDiscordSettingsUrl() {
    return `${this.groupScopeUrl}/update-discord-settings`;
  }

  get renameGroupUrl() {
    return `${this.groupScopeUrl}/rename-group`;
  }

  get rerollGroupTokenUrl() {
    return `${this.groupScopeUrl}/reroll-group-token`;
  }

  get deleteGroupUrl() {
    return `${this.groupScopeUrl}/delete-group`;
  }

  get gePricesUrl() {
    return `${this.baseUrl}/ge-prices`;
  }

  get skillDataUrl() {
    return `${this.groupScopeUrl}/get-skill-data`;
  }

  get leaderboardUrl() {
    return `${this.groupScopeUrl}/get-leaderboard`;
  }

  get metricDataUrl() {
    return `${this.groupScopeUrl}/get-metric-data`;
  }

  get captchaEnabledUrl() {
    return `${this.baseUrl}/captcha-enabled`;
  }

  get portraitUrl() {
    return `${this.groupScopeUrl}/portrait`;
  }

  get activityEventsUrl() {
    return `${this.groupScopeUrl}/get-activity-events`;
  }

  get lootSummaryUrl() {
    return `${this.groupScopeUrl}/get-loot-summary`;
  }

  get itemBonusesUrl() {
    return `${this.groupScopeUrl}/get-item-bonuses`;
  }

  get activePingsUrl() {
    return `${this.groupScopeUrl}/get-active-pings`;
  }

  get activeRaidMarkersUrl() {
    return `${this.groupScopeUrl}/get-active-raid-markers`;
  }

  setCredentials(groupName, groupToken) {
    this.groupName = groupName;
    this.groupToken = groupToken;
  }

  async restart() {
    const groupName = this.groupName;
    const groupToken = this.groupToken;
    if (this.adminView) {
      await this.enableAdminView(this.adminViewGroupId, groupName, this.adminToken);
    } else {
      await this.enable(groupName, groupToken);
    }
  }

  async enable(groupName, groupToken) {
    await this.disable();
    this.adminView = false;
    this.setCredentials(groupName, groupToken);
    await this.startPolling();
  }

  // Same startup as `enable()`, but authenticated as a global admin viewing this group
  // read-only rather than as a member holding the group's token - see `groupScopeUrl`.
  async enableAdminView(groupId, groupName, adminToken) {
    await this.disable();
    this.adminView = true;
    this.adminViewGroupId = groupId;
    this.groupName = groupName;
    this.adminToken = adminToken;
    await this.startPolling();
  }

  async startPolling() {
    this.nextCheck = new Date(0).toISOString();
    if (!this.enabled) {
      this.enabled = true;
      // getGroupInterval is a Promise so we can make sure this method does not leak
      // any intervals with multiple calls to .enable(). This could be possible because of
      // the wait for the item and quest data loads before we create the interval.
      this.getGroupInterval = pubsub.waitForAllEvents("item-data-loaded", "quest-data-loaded").then(() => {
        return utility.callOnInterval(this.pollTick.bind(this), 1000);
      });
    }

    await this.getGroupInterval;
  }

  async disable() {
    this.enabled = false;
    this.adminView = false;
    this.adminViewGroupId = undefined;
    this.adminToken = undefined;
    this.groupName = undefined;
    this.groupToken = undefined;
    groupData.members = new Map();
    groupData.groupItems = {};
    groupData.filters = [""];
    this.seenPingIds = undefined;
    if (this.getGroupInterval) {
      window.clearInterval(await this.getGroupInterval);
    }
  }

  // The single interval tick driving `startPolling` - keeps active pings on the same ~1s
  // cadence as member positions rather than opening a second interval for them.
  async pollTick() {
    await this.getGroupData();
    await this.getActivePings();
    await this.getActiveRaidMarkers();
  }

  async getActivePings() {
    const response = await fetch(this.activePingsUrl, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) return;

    const pings = await response.json();

    // A ping newly appearing since the last poll gets a toast - same "diff against what we saw
    // last time" idea as toast-source.js's activity events, just sourced from this poll instead
    // since pings never land in the activity event feed (they're ephemeral, not stored).
    if (this.seenPingIds) {
      for (const ping of pings) {
        if (!this.seenPingIds.has(ping.pingId)) {
          pubsub.publish("toast", { type: "ping", ping });
        }
      }
    }
    this.seenPingIds = new Set(pings.map((ping) => ping.pingId));

    pubsub.publish("active-pings", pings);
  }

  // No toast/seen-id diffing here, unlike getActivePings - a raid marker is a persistent state
  // change (up to 8 per player) rather than a one-off event worth calling out, and the plugin
  // itself doesn't chat-message on marker start either.
  async getActiveRaidMarkers() {
    const response = await fetch(this.activeRaidMarkersUrl, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) return;

    const markers = await response.json();
    pubsub.publish("active-raid-markers", markers);
  }

  async getGroupData() {
    const nextCheck = this.nextCheck;

    const response = await fetch(`${this.getGroupDataUrl}?from_time=${nextCheck}`, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      if (response.status === 401) {
        const wasAdminView = this.adminView;
        await this.disable();
        if (wasAdminView) adminViewSession.clear();
        window.history.pushState("", "", wasAdminView ? "/admin/groups" : "/");
        pubsub.publish("get-group-data");
      }
      return;
    }

    const newGroupData = await response.json();
    this.nextCheck = groupData.update(newGroupData).toISOString();
    pubsub.publish("get-group-data", groupData);
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
        Authorization: this.authHeader,
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
        Authorization: this.authHeader,
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
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
      method: "POST",
    });

    return response;
  }

  async getBlockedMembers() {
    const response = await fetch(this.blockedMembersUrl, {
      headers: {
        Authorization: this.authHeader,
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
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
    });

    return response.ok;
  }

  async getDiscordSettings() {
    const response = await fetch(this.discordSettingsUrl, {
      headers: {
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
    });

    return response;
  }

  async updateDiscordSettings(settings) {
    const response = await fetch(this.updateDiscordSettingsUrl, {
      body: JSON.stringify(settings),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
      method: "PUT",
    });

    return response;
  }

  async getGroupPermissions() {
    const response = await fetch(this.groupPermissionsUrl, {
      headers: {
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
    });

    return response;
  }

  async getMyPermissions() {
    const response = await fetch(this.myPermissionsUrl, {
      headers: {
        Authorization: this.authHeader,
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
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
      method: "PUT",
    });

    return response;
  }

  async updateMemberColor(accountId, color) {
    const response = await fetch(this.updateMemberColorUrl, {
      body: JSON.stringify({ account_id: accountId, color }),
      headers: {
        "Content-Type": "application/json",
        Authorization: this.authHeader,
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
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
      method: "PUT",
    });

    return response;
  }

  async rerollGroupToken() {
    const response = await fetch(this.rerollGroupTokenUrl, {
      headers: {
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
      method: "POST",
    });

    return response;
  }

  async deleteGroup() {
    const response = await fetch(this.deleteGroupUrl, {
      headers: {
        Authorization: this.authHeader,
        ...this.accountAuthHeaders,
      },
      method: "DELETE",
    });

    return response;
  }

  async amILoggedIn() {
    const response = await fetch(this.amILoggedInUrl, {
      headers: { Authorization: this.authHeader },
    });

    return response;
  }

  async getGePrices() {
    const response = await fetch(this.gePricesUrl);
    return response;
  }

  async getSkillData(period) {
    const response = await fetch(`${this.skillDataUrl}?period=${period}`, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  }

  async getLeaderboard(metric, window, boss, skill, raidType, raidDifficulty) {
    const query = new URLSearchParams({ metric, window });
    if (boss) query.set("boss", boss);
    if (skill && metric === "xp") query.set("skill", skill);
    if (raidType && metric === "raid_completions") query.set("raid_type", raidType);
    if (raidDifficulty && metric === "raid_completions") query.set("raid_difficulty", raidDifficulty);
    const response = await fetch(`${this.leaderboardUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      return { metric, window, boss: boss || null, available_bosses: [], entries: [] };
    }
    return response.json();
  }

  async getMetricData(metric, period, boss, raidType, raidDifficulty, groupBy) {
    const query = new URLSearchParams({ metric, period });
    if (boss) query.set("boss", boss);
    if (raidType && metric === "raid_completions") query.set("raid_type", raidType);
    if (raidDifficulty && metric === "raid_completions") query.set("raid_difficulty", raidDifficulty);
    if (groupBy && metric === "raid_completions") query.set("group_by", groupBy);
    const response = await fetch(`${this.metricDataUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      return [];
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
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      return [];
    }
    return response.json();
  }

  async getLootSummary({ memberName, sessionId, boss, clueTier, since, until, splitMode } = {}) {
    const query = new URLSearchParams();
    if (memberName) query.set("member_name", memberName);
    if (sessionId) query.set("session_id", sessionId);
    if (boss) query.set("boss", boss);
    if (clueTier) query.set("clue_tier", clueTier);
    if (since) query.set("since", since);
    if (until) query.set("until", until);
    if (splitMode) query.set("split_mode", splitMode);

    const response = await fetch(`${this.lootSummaryUrl}?${query.toString()}`, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      return { rows: [], sources: [] };
    }
    return response.json();
  }

  async getItemBonuses(itemId) {
    const response = await fetch(`${this.itemBonusesUrl}?item_id=${itemId}`, {
      headers: {
        Authorization: this.authHeader,
      },
    });
    if (!response.ok) {
      throw new Error(`get-item-bonuses ${itemId} failed: ${response.status}`);
    }
    return response.json();
  }

  async getPortrait(memberName) {
    const response = await fetch(`${this.portraitUrl}/${encodeURIComponent(memberName)}`, {
      headers: {
        Authorization: this.authHeader,
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
