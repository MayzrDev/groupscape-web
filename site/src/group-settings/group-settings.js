import { BaseElement } from "../base-element/base-element";
import { appearance } from "../appearance";
import { api } from "../data/api";
import { storage } from "../data/storage";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { validCharacters, validLength } from "../validators";
import { pubsub } from "../data/pubsub";
import { mapTrails, DEFAULT_MAP_TRAIL_SETTINGS } from "../data/map-trails";

// A real invite token always looks like `${groupName}|${uuid}` (see `db::rename_group` /
// `db::reroll_group_token` on the server). When this group was reached by clicking into it
// from the account characters page, `storage.groupToken` instead holds the account's session
// token as a fallback auth credential (see `characters-page.viewGroup`) - the server never
// stores the plaintext invite token, so there's no way to recover the real one for display.
function hasRealToken(group) {
  return !!group.groupToken && group.groupToken.startsWith(`${group.groupName}|`);
}

// Accepts plain gp amounts or `k`/`m` shorthand (e.g. "250k", "1.2m") for the Drops
// minimum-value field. Returns null for anything that doesn't parse as a non-negative number.
function parseGpShorthand(value) {
  const match = /^([\d.]+)\s*(k|m)?$/i.exec(value.trim());
  if (!match) return null;
  let amount = parseFloat(match[1]);
  if (Number.isNaN(amount)) return null;
  const suffix = match[2] ? match[2].toLowerCase() : null;
  if (suffix === "k") amount *= 1_000;
  if (suffix === "m") amount *= 1_000_000;
  return Math.round(amount);
}

// Renders a gp amount as the shortest shorthand that round-trips through parseGpShorthand,
// so re-opening the settings shows back whatever form is easiest to read (e.g. 250000 -> "250k").
function formatGpShorthand(amount) {
  if (amount !== 0 && amount % 1_000_000 === 0) return `${amount / 1_000_000}m`;
  if (amount !== 0 && amount % 1_000 === 0) return `${amount / 1_000}k`;
  return String(amount);
}

export class GroupSettings extends BaseElement {
  constructor() {
    super();
    this.canKick = false;
  }

  /* eslint-disable no-unused-vars */
  html() {
    const group = storage.getGroup();
    const showToken = hasRealToken(group);
    const selectedPanelDockSide = appearance.getLayout();
    const style = appearance.getTheme();
    const mapTrailSettings = mapTrails.settings;
    const mapTrailAgeMinutes = Math.round(mapTrailSettings.maxAgeMs / 60000);
    return `{{group-settings.html}}`;
  }
  /* eslint-enable no-unused-vars */

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.bindElements();
    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    this.subscribe("blocked-members-changed", this.loadBlockedMembers.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  // Re-run after any re-render (rename/reroll swap the DOM via this.render()) so the
  // freshly rendered elements get listeners again - the pubsub subscription above stays
  // bound for the component's lifetime and doesn't need re-registering.
  bindElements() {
    const panelDockSide = this.querySelector(".group-settings__panels");
    const appearanceStyle = this.querySelector(".group-settings__style");
    this.eventListener(panelDockSide, "change", this.handlePanelDockSideChange.bind(this));
    this.eventListener(appearanceStyle, "change", this.handleStyleChange.bind(this));

    this.mapTrailLength = this.querySelector(".group-settings__map-trail-length");
    this.mapTrailAge = this.querySelector(".group-settings__map-trail-age");
    this.mapTrailJump = this.querySelector(".group-settings__map-trail-jump");
    this.eventListener(this.mapTrailLength, "change", this.handleMapTrailSettingsChange.bind(this));
    this.eventListener(this.mapTrailAge, "change", this.handleMapTrailSettingsChange.bind(this));
    this.eventListener(this.mapTrailJump, "change", this.handleMapTrailSettingsChange.bind(this));
    const mapTrailReset = this.querySelector(".group-settings__map-trail-reset");
    this.eventListener(mapTrailReset, "click", this.resetMapTrailSettings.bind(this));

    this.nameInput = this.querySelector(".group-settings__name-input");
    this.nameInput.validators = [
      (value) => (!validCharacters(value) ? "Group name has some unsupported special characters." : null),
      (value) => (!validLength(value) ? "Group name must be between 1 and 16 characters." : null),
    ];
    this.nameError = this.querySelector(".group-settings__error");
    const renameButton = this.querySelector(".group-settings__rename-button");
    this.eventListener(renameButton, "click", this.renameGroup.bind(this));

    const tokenHide = this.querySelector(".setup__credential-hide");
    if (tokenHide) this.eventListener(tokenHide, "click", () => tokenHide.remove());
    const copyTokenButton = this.querySelector(".group-settings__copy-token-button");
    if (copyTokenButton) this.eventListener(copyTokenButton, "click", this.copyToken.bind(this));
    const rerollButton = this.querySelector(".group-settings__reroll-button");
    this.eventListener(rerollButton, "click", this.confirmRerollToken.bind(this));

    const deleteButton = this.querySelector(".group-settings__delete-button");
    this.eventListener(deleteButton, "click", this.confirmDeleteGroup.bind(this));

    const blockedToggle = this.querySelector(".group-settings__blocked-toggle");
    this.eventListener(blockedToggle, "click", this.toggleBlockedList.bind(this));

    const discordHowtoToggle = this.querySelector(".group-settings__discord-howto-toggle");
    this.eventListener(discordHowtoToggle, "click", this.toggleDiscordHowto.bind(this));
    this.discordUrlInput = this.querySelector(".group-settings__discord-url-input");
    const discordSaveButton = this.querySelector(".group-settings__discord-save-button");
    this.eventListener(discordSaveButton, "click", this.saveDiscordSettings.bind(this));
    for (const checkbox of this.querySelectorAll(".group-settings__discord-notify-checkbox")) {
      this.eventListener(checkbox, "change", this.saveDiscordSettings.bind(this));
    }
    this.discordDropsMinValueInput = this.querySelector(".group-settings__discord-drops-min-value");
    this.discordDropsParsed = this.querySelector(".group-settings__discord-drops-parsed");
    this.eventListener(this.discordDropsMinValueInput, "input", this.updateDiscordDropsParsedPreview.bind(this));
    this.eventListener(this.discordDropsMinValueInput, "change", this.saveDiscordSettings.bind(this));
    this.discordDropsUniqueOnlyCheckbox = this.querySelector(".group-settings__discord-drops-unique-checkbox");
    this.eventListener(
      this.discordDropsUniqueOnlyCheckbox,
      "change",
      this.handleDiscordDropsUniqueOnlyChange.bind(this)
    );

    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    this.loadBlockedMembers();
    this.loadPermissions();
    this.loadCanKickMembers();
    this.loadDiscordSettings();
  }

  // Cached rather than checked per-member: it's one account's permission for this group, not
  // something that varies member-to-member, and `handleUpdatedMembers` re-runs frequently as
  // telemetry streams in.
  async loadCanKickMembers() {
    const [canKickMembers, myPermissionsResponse] = await Promise.all([api.canKickMembers(), api.getMyPermissions()]);
    this.canKickMembers = canKickMembers;
    this.myMemberName = myPermissionsResponse.ok ? (await myPermissionsResponse.json()).member_name : null;
    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }
  }

  hideNameError() {
    this.nameError.innerHTML = "";
  }

  showNameError(message) {
    this.nameError.innerHTML = message;
  }

  async renameGroup() {
    this.hideNameError();
    if (!this.nameInput.valid) return;
    const newName = this.nameInput.value;
    const currentGroup = storage.getGroup();

    if (newName === currentGroup.groupName) {
      this.showNameError("New name is the same as the old name");
      return;
    }

    try {
      loadingScreenManager.showLoadingScreen();
      const result = await api.renameGroup(newName);
      if (result.ok) {
        const credentials = await result.json();
        storage.storeGroup(credentials.name, credentials.token);
        api.setCredentials(credentials.name, credentials.token);
        await api.restart();
        this.render();
        this.bindElements();
      } else {
        const message = await result.text();
        this.showNameError(`Failed to rename group ${message}`);
      }
    } catch (error) {
      this.showNameError(`Failed to rename group ${error}`);
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }

  copyToken() {
    const group = storage.getGroup();
    if (!hasRealToken(group)) return;
    navigator.clipboard.writeText(group.groupToken);
  }

  confirmRerollToken() {
    confirmDialogManager.confirm({
      headline: "Reroll group token?",
      body: "The current token stops working immediately - anyone who hasn't joined yet will need the new one.",
      yesCallback: this.rerollToken.bind(this),
      noCallback: () => {},
    });
  }

  async rerollToken() {
    try {
      loadingScreenManager.showLoadingScreen();
      const result = await api.rerollGroupToken();
      if (result.ok) {
        const credentials = await result.json();
        storage.storeGroup(credentials.name, credentials.token);
        api.setCredentials(credentials.name, credentials.token);
        await api.restart();
        this.render();
        this.bindElements();
      }
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }

  confirmDeleteGroup() {
    confirmDialogManager.confirm({
      headline: "Delete this group?",
      body: "All members and tracked history will be permanently lost. This can't be undone.",
      yesCallback: this.deleteGroup.bind(this),
      noCallback: () => {},
    });
  }

  async deleteGroup() {
    try {
      loadingScreenManager.showLoadingScreen();
      const result = await api.deleteGroup();
      if (result.ok) {
        api.disable();
        storage.clearGroup();
        window.history.pushState("", "", "/");
      }
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }

  handleStyleChange() {
    const style = this.querySelector(`input[name="appearance-style"]:checked`).value;
    appearance.setTheme(style);
  }

  handlePanelDockSideChange() {
    const side = this.querySelector(`input[name="panel-dock-side"]:checked`).value;

    if (side === "right") {
      appearance.setLayout("row-reverse");
    } else {
      appearance.setLayout("row");
    }
  }

  // Clamp to the input's own min/max rather than trusting the typed value - number
  // inputs still let the field go empty or out of range before blur/change fires.
  clampToInput(input) {
    const min = Number(input.min);
    const max = Number(input.max);
    const value = Number(input.value);
    const clamped = Math.min(max, Math.max(min, isNaN(value) ? min : value));
    input.value = clamped;
    return clamped;
  }

  handleMapTrailSettingsChange() {
    const maxPoints = this.clampToInput(this.mapTrailLength);
    const ageMinutes = this.clampToInput(this.mapTrailAge);
    const jumpThresholdTiles = this.clampToInput(this.mapTrailJump);
    mapTrails.setSettings({ maxPoints, maxAgeMs: ageMinutes * 60 * 1000, jumpThresholdTiles });
  }

  resetMapTrailSettings() {
    mapTrails.setSettings(DEFAULT_MAP_TRAIL_SETTINGS);
    this.mapTrailLength.value = DEFAULT_MAP_TRAIL_SETTINGS.maxPoints;
    this.mapTrailAge.value = Math.round(DEFAULT_MAP_TRAIL_SETTINGS.maxAgeMs / 60000);
    this.mapTrailJump.value = DEFAULT_MAP_TRAIL_SETTINGS.jumpThresholdTiles;
  }

  handleUpdatedMembers(members) {
    members = members.filter((member) => member.name !== "@SHARED");
    let memberEdits = document.createDocumentFragment();
    for (const member of members) {
      const memberEdit = document.createElement("edit-member");
      memberEdit.member = member;
      memberEdit.canKick = Boolean(this.canKickMembers);
      memberEdit.isSelf = member.name === this.myMemberName;
      memberEdits.appendChild(memberEdit);
    }

    const memberSection = this.querySelector(".group-settings__members");
    memberSection.innerHTML = "";
    memberSection.appendChild(memberEdits);

    const openSlots = 5 - members.length;
    const openSlotsText = this.querySelector(".group-settings__open-slots");
    openSlotsText.textContent =
      openSlots > 0
        ? `${openSlots} open slot${openSlots === 1 ? "" : "s"} — anyone with your group token can join`
        : "";
  }

  toggleBlockedList() {
    this.querySelector(".group-settings__blocked-list").classList.toggle("group-settings__blocked-list--open");
    this.querySelector(".group-settings__blocked-arrow").classList.toggle("group-settings__blocked-arrow--open");
  }

  async loadBlockedMembers() {
    const response = await api.getBlockedMembers();
    const blockedMembers = response.ok ? await response.json() : [];

    const countLabel = this.querySelector(".group-settings__blocked-count");
    countLabel.textContent = blockedMembers.length > 0 ? `${blockedMembers.length} blocked` : "None blocked";

    const list = this.querySelector(".group-settings__blocked-list");
    list.innerHTML = "";
    for (const blockedMember of blockedMembers) {
      const row = document.createElement("div");
      row.className = "group-settings__blocked-row";

      const name = document.createElement("span");
      name.className = "group-settings__blocked-name";
      name.textContent = blockedMember.member_name;

      const unblockButton = document.createElement("button");
      unblockButton.className = "men-button small";
      unblockButton.textContent = "Unblock";
      unblockButton.addEventListener("click", () => this.unblockMember(blockedMember.member_name));

      row.appendChild(name);
      row.appendChild(unblockButton);
      list.appendChild(row);
    }
  }

  // The permissions endpoint itself is the section's admin gate (server-side, requires
  // ManagePermissions or ManageSettings) - a 401/403 here means this account has neither, so the
  // whole section stays hidden rather than showing controls that would just 403 on save. Which
  // controls each row actually renders (permission toggles vs. colour picker) is then decided
  // per this account's own flags from `get-my-permissions`, since the two are gated separately.
  async loadPermissions() {
    const section = this.querySelector(".group-settings__permissions-section");
    const [response, myPermissionsResponse] = await Promise.all([api.getGroupPermissions(), api.getMyPermissions()]);
    if (!response.ok) {
      section.style.display = "none";
      return;
    }
    section.style.display = "";

    const myPermissions = myPermissionsResponse.ok ? await myPermissionsResponse.json() : {};
    const showPermissions = !!myPermissions.manage_permissions;
    const showColor = !!myPermissions.manage_settings;

    const permissions = await response.json();
    const container = this.querySelector(".group-settings__permissions");
    container.innerHTML = "";

    for (const permission of permissions) {
      const permissionMember = document.createElement("permission-member");
      permissionMember.showPermissions = showPermissions;
      permissionMember.showColor = showColor;
      permissionMember.permission = permission;
      container.appendChild(permissionMember);
    }
  }

  toggleDiscordHowto() {
    this.querySelector(".group-settings__discord-howto-body").classList.toggle(
      "group-settings__discord-howto-body--open"
    );
    this.querySelector(".group-settings__discord-howto-arrow").classList.toggle(
      "group-settings__discord-howto-arrow--open"
    );
  }

  // Mirrors loadPermissions' gating pattern: the GET is itself the ManageDiscord check, so a
  // 401/403 just hides the whole section rather than showing controls that would 403 on save.
  async loadDiscordSettings() {
    const section = this.querySelector(".group-settings__discord-section");
    const response = await api.getDiscordSettings();
    if (!response.ok) {
      section.style.display = "none";
      return;
    }
    section.style.display = "";

    const settings = await response.json();
    this.lastSavedDiscordSettings = settings;
    this.applyDiscordSettingsToForm(settings);
    this.setDiscordConnected(!!settings.webhook_url);
  }

  applyDiscordSettingsToForm(settings) {
    this.discordUrlInput.input.value = settings.webhook_url || "";
    for (const checkbox of this.querySelectorAll(".group-settings__discord-notify-checkbox")) {
      checkbox.checked = !!settings[checkbox.closest(".group-settings__discord-notify-row").dataset.key];
    }
    this.discordDropsMinValueInput.value = formatGpShorthand(settings.drops_min_value ?? 250_000);
    this.updateDiscordDropsParsedPreview();
    this.discordDropsUniqueOnlyCheckbox.checked = !!settings.drops_unique_only;
    this.updateDiscordDropsUniqueOnlyLock();
  }

  // The min-value field is meaningless once unique-only is on (drop_lines ignores it server-side),
  // so it's disabled here to match rather than left editable but silently unused.
  updateDiscordDropsUniqueOnlyLock() {
    const uniqueOnly = this.discordDropsUniqueOnlyCheckbox.checked;
    this.discordDropsMinValueInput.disabled = uniqueOnly;
    this.querySelector(".group-settings__discord-drops-unique-note").style.display = uniqueOnly ? "" : "none";
  }

  handleDiscordDropsUniqueOnlyChange() {
    this.updateDiscordDropsUniqueOnlyLock();
    this.saveDiscordSettings();
  }

  updateDiscordDropsParsedPreview() {
    const parsed = parseGpShorthand(this.discordDropsMinValueInput.value);
    this.discordDropsParsed.textContent =
      parsed === null ? "Unrecognized amount — try 250k or 1.2m" : `= ${parsed.toLocaleString()} gp`;
  }

  // A webhook only ever gets persisted after `update-discord-settings` tests it live against
  // Discord (see `discord::test_webhook`), so a saved, non-empty URL can be trusted as connected
  // without re-testing it just for opening the settings page.
  setDiscordConnected(connected) {
    const badge = this.querySelector(".group-settings__discord-conn-badge");
    badge.classList.toggle("group-settings__discord-conn-badge--connected", connected);
    badge.querySelector(".group-settings__discord-conn-text").textContent = connected ? "Connected" : "Not connected";

    for (const row of this.querySelectorAll(".group-settings__discord-notify-row")) {
      row.classList.toggle("group-settings__discord-notify-row--locked", !connected);
    }
    for (const checkbox of this.querySelectorAll(".group-settings__discord-notify-checkbox")) {
      checkbox.disabled = !connected;
    }
    this.discordDropsUniqueOnlyCheckbox.disabled = !connected;
    this.discordDropsMinValueInput.disabled = !connected || this.discordDropsUniqueOnlyCheckbox.checked;
    this.querySelector(".group-settings__discord-lock-note").style.display = connected ? "none" : "";
  }

  showDiscordStatus(text, kind) {
    const status = this.querySelector(".group-settings__discord-status");
    status.textContent = text;
    status.className = `group-settings__discord-status group-settings__discord-status--${kind}`;
  }

  // Always submits the full settings object (matching DiscordWebhookSettings' deny_unknown_fields
  // + full-replace update endpoint) - fired both by the Save button and by any notify checkbox
  // change, since there's no separate patch endpoint for toggles alone.
  async saveDiscordSettings() {
    const webhookUrl = this.discordUrlInput.value;
    const settings = { webhook_url: webhookUrl || null };
    for (const checkbox of this.querySelectorAll(".group-settings__discord-notify-checkbox")) {
      settings[checkbox.closest(".group-settings__discord-notify-row").dataset.key] = checkbox.checked;
    }
    const parsedMinValue = parseGpShorthand(this.discordDropsMinValueInput.value);
    settings.drops_min_value = parsedMinValue ?? this.lastSavedDiscordSettings?.drops_min_value ?? 250_000;
    settings.drops_unique_only = this.discordDropsUniqueOnlyCheckbox.checked;

    try {
      loadingScreenManager.showLoadingScreen();
      const response = await api.updateDiscordSettings(settings);
      if (response.ok) {
        const saved = await response.json();
        const urlChanged = saved.webhook_url !== (this.lastSavedDiscordSettings?.webhook_url ?? null);
        this.lastSavedDiscordSettings = saved;
        this.setDiscordConnected(!!saved.webhook_url);
        this.showDiscordStatus(
          saved.webhook_url
            ? urlChanged
              ? "Saved — test message sent to Discord."
              : "Saved."
            : "Saved — webhook cleared, notifications are off.",
          "ok"
        );
      } else {
        // The server rejected the whole update (bad URL, or Discord no longer recognizes it) -
        // nothing was persisted, so the notify checkboxes are reset to the last known-good state
        // rather than left showing an edit that never actually saved. The URL field is left as
        // typed so it can be corrected.
        const message = await response.text();
        if (this.lastSavedDiscordSettings) {
          this.applyDiscordSettingsToForm({ ...this.lastSavedDiscordSettings, webhook_url: webhookUrl });
        }
        this.showDiscordStatus(message, "err");
      }
    } catch (error) {
      this.showDiscordStatus(`Failed to save: ${error}`, "err");
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }

  async unblockMember(memberName) {
    try {
      loadingScreenManager.showLoadingScreen();
      await api.unblockMember(memberName);
      await this.loadBlockedMembers();
    } finally {
      loadingScreenManager.hideLoadingScreen();
    }
  }
}

customElements.define("group-settings", GroupSettings);
