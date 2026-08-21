import { BaseElement } from "../base-element/base-element";
import { appearance } from "../appearance";
import { api } from "../data/api";
import { storage } from "../data/storage";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { validCharacters, validLength } from "../validators";
import { pubsub } from "../data/pubsub";

export class GroupSettings extends BaseElement {
  constructor() {
    super();
    this.canKick = false;
  }

  /* eslint-disable no-unused-vars */
  html() {
    const group = storage.getGroup();
    const selectedPanelDockSide = appearance.getLayout();
    const style = appearance.getTheme();
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

    this.nameInput = this.querySelector(".group-settings__name-input");
    this.nameInput.validators = [
      (value) => (!validCharacters(value) ? "Group name has some unsupported special characters." : null),
      (value) => (!validLength(value) ? "Group name must be between 1 and 16 characters." : null),
    ];
    this.nameError = this.querySelector(".group-settings__error");
    const renameButton = this.querySelector(".group-settings__rename-button");
    this.eventListener(renameButton, "click", this.renameGroup.bind(this));

    const tokenHide = this.querySelector(".setup__credential-hide");
    this.eventListener(tokenHide, "click", () => tokenHide.remove());
    const copyTokenButton = this.querySelector(".group-settings__copy-token-button");
    this.eventListener(copyTokenButton, "click", this.copyToken.bind(this));
    const rerollButton = this.querySelector(".group-settings__reroll-button");
    this.eventListener(rerollButton, "click", this.confirmRerollToken.bind(this));

    const deleteButton = this.querySelector(".group-settings__delete-button");
    this.eventListener(deleteButton, "click", this.confirmDeleteGroup.bind(this));

    const blockedToggle = this.querySelector(".group-settings__blocked-toggle");
    this.eventListener(blockedToggle, "click", this.toggleBlockedList.bind(this));

    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }

    this.loadBlockedMembers();
    this.loadPermissions();
    this.loadCanKickMembers();
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
