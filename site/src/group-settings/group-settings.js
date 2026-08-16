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
      body: "The current token stops working immediately - every member will need to re-paste the new one into the plugin.",
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
    for (let i = 0; i < members.length; ++i) {
      const member = members[i];
      const memberEdit = document.createElement("edit-member");
      memberEdit.member = member;
      memberEdit.memberNumber = i + 1;

      memberEdits.appendChild(memberEdit);
    }

    if (members.length < 5) {
      const addMember = document.createElement("edit-member");
      addMember.memberNumber = members.length + 1;
      memberEdits.appendChild(addMember);
    }

    const memberSection = this.querySelector(".group-settings__members");
    memberSection.innerHTML = "";
    memberSection.appendChild(memberEdits);
  }
}

customElements.define("group-settings", GroupSettings);
