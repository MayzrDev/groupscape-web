import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";
import { storage } from "../data/storage";

const POLL_INTERVAL_MS = 5000;

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function statsSummary(character) {
  if (character.combat_level == null && character.total_level == null) {
    return "No stats reported yet.";
  }
  const parts = [];
  if (character.combat_level != null) parts.push(`Combat level ${character.combat_level}`);
  if (character.total_level != null) parts.push(`${character.total_level.toLocaleString()} total level`);
  return parts.join(" · ");
}

const requiredFieldValidator = (value) => {
  if (value.length === 0) {
    return "This field is required.";
  }
};

/**
 * Shown right after login/signup instead of dumping the user on the map with no group and no
 * linked character. Two steps, driven entirely off `GET /api/account/characters`:
 *  1. link a character (show the account's API key, wait for the plugin to report one, then
 *     confirm it) — reuses the same portrait/confirm-card shape as `characters-page`.
 *  2. join or create a group for the first confirmed-but-ungrouped character.
 * Polls on an interval the whole time it's mounted so both steps advance on their own once the
 * plugin reports something, without the user needing to refresh. Either step can be skipped —
 * this is a fast path to a good default, not a gate.
 */
export class OnboardingPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{onboarding-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    const params = new URLSearchParams(window.location.search);
    this.addingAnother = params.get("add") === "1";

    this.status = this.querySelector(".onboarding-page__status");
    this.content = this.querySelector(".onboarding-page__content");
    this.linkEmptySection = this.querySelector(".onboarding-page__link-empty");
    this.linkPendingSection = this.querySelector(".onboarding-page__link-pending");
    this.groupSection = this.querySelector(".onboarding-page__group-step");

    this.apikeyBox = this.querySelector(".onboarding-page__apikey-box");
    this.apikeyValue = this.querySelector(".onboarding-page__apikey-value");
    this.apikeyCopy = this.querySelector(".onboarding-page__apikey-copy");
    this.revealHint = this.querySelector(".onboarding-page__reveal-hint");
    this.keyError = this.querySelector(".onboarding-page__key-error");

    this.pendingPortraitSlot = this.querySelector(".onboarding-page__pending-portrait-slot");
    this.pendingRsn = this.querySelector(".onboarding-page__pending-rsn");
    this.pendingStats = this.querySelector(".onboarding-page__pending-stats");
    this.confirmButton = this.querySelector(".onboarding-page__confirm-btn");
    this.removeButton = this.querySelector(".onboarding-page__remove-btn");
    this.pendingError = this.querySelector(".onboarding-page__pending-error");

    this.groupCharacterName = this.querySelector(".onboarding-page__group-character-name");
    this.choiceRow = this.querySelector(".onboarding-page__choice-row");
    this.joinPanel = this.querySelector(".onboarding-page__join-panel");
    this.joinToken = this.querySelector(".onboarding-page__join-token");
    this.joinToken.validators = [requiredFieldValidator];
    this.joinSubmit = this.querySelector(".onboarding-page__join-submit");
    this.joinError = this.querySelector(".onboarding-page__join-error");

    this.skipLink = this.querySelector(".onboarding-page__skip-link");

    this.eventListener(this.apikeyCopy, "click", this.copyApiKey.bind(this));
    this.eventListener(this.confirmButton, "click", () => this.confirmCharacter(this.pendingCharacterId));
    this.eventListener(this.removeButton, "click", () =>
      this.promptRemoveCharacter(this.pendingCharacterId, this.pendingRsnValue)
    );
    this.eventListener(this.choiceRow, "click", this.handleChoiceClick.bind(this));
    this.eventListener(this.joinSubmit, "click", this.joinGroup.bind(this));
    this.eventListener(this.skipLink, "click", () => window.history.pushState("", "", "/account"));

    this.currentStepKey = null;
    this.currentCharacterId = null;

    this.checkSession();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.stopPolling();
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (!response.ok) {
      window.history.pushState("", "", "/account/login");
      return;
    }
    this.status.textContent = "";
    this.content.hidden = false;
    this.refresh();
    this.pollHandle = setInterval(() => this.refresh(), POLL_INTERVAL_MS);
  }

  stopPolling() {
    if (this.pollHandle) {
      clearInterval(this.pollHandle);
      this.pollHandle = null;
    }
  }

  async refresh() {
    const response = await accountApi.listCharacters();
    if (!response.ok) return;
    const characters = await response.json();
    this.renderStep(characters);
  }

  renderStep(characters) {
    const pending = characters.find((character) => character.status === "pending");
    const ungrouped = characters.find((character) => character.status !== "pending" && character.group_id == null);

    if (pending) {
      this.addingAnother = false;
      this.showStep("link-pending", pending.id);
      this.renderPendingCharacter(pending);
    } else if (characters.length === 0) {
      this.showStep("link-empty", null);
    } else if (ungrouped) {
      this.addingAnother = false;
      this.showStep("group", ungrouped.id);
      this.groupCharacterName.textContent = ungrouped.display_rsn;
    } else if (this.addingAnother) {
      // Existing characters are all confirmed and grouped, but the user explicitly asked to
      // link another one (from character-select's "+ Link another character") — show the link
      // step instead of bouncing back to /characters like the no-flag case below does. Stays
      // true until the new character actually shows up above (pending/ungrouped), so this step
      // doesn't flicker back to /characters on the next poll while waiting on the plugin.
      this.showStep("link-empty", null);
    } else {
      // Every character is confirmed and in a group — nothing left to onboard.
      window.history.pushState("", "", "/characters");
    }
  }

  showStep(stepKey, characterId) {
    if (this.currentStepKey === stepKey && this.currentCharacterId === characterId) return;
    this.currentStepKey = stepKey;
    this.currentCharacterId = characterId;

    this.linkEmptySection.hidden = stepKey !== "link-empty";
    this.linkPendingSection.hidden = stepKey !== "link-pending";
    this.groupSection.hidden = stepKey !== "group";

    if (stepKey === "link-empty") {
      this.renderLinkEmpty();
    }
    if (stepKey === "group") {
      this.choiceRow.hidden = false;
      this.joinPanel.hidden = true;
      this.joinError.textContent = "";
    }
  }

  renderLinkEmpty() {
    const freshApiKey = sessionStorage.getItem("freshApiKey");
    if (freshApiKey) {
      this.apikeyBox.hidden = false;
      this.apikeyValue.textContent = freshApiKey;
      this.revealHint.hidden = false;
    } else {
      this.apikeyBox.hidden = true;
      this.revealHint.hidden = true;
      this.fetchApiKey();
    }
  }

  async copyApiKey() {
    await navigator.clipboard.writeText(this.apikeyValue.textContent);
  }

  async fetchApiKey() {
    this.keyError.textContent = "";
    try {
      const response = await accountApi.regenerateApiKey();
      if (response.ok) {
        const { api_key } = await response.json();
        sessionStorage.setItem("freshApiKey", api_key);
        this.renderLinkEmpty();
      } else {
        this.keyError.textContent = "Couldn't generate your API key — try again.";
      }
    } catch (error) {
      this.keyError.textContent = "Couldn't generate your API key — try again.";
    }
  }

  renderPendingCharacter(character) {
    // Only rebuild the portrait (and its WebGL scene) when the character actually changes -
    // recreating it on every poll tick would reload/flicker it even when nothing changed.
    if (this.pendingCharacterId !== character.id) {
      this.pendingPortraitSlot.innerHTML = `<player-portrait account-character-id="${character.id}" mode="bust" class="onboarding-page__pending-portrait"></player-portrait>`;
    }
    this.pendingRsn.textContent = character.display_rsn;
    this.pendingStats.textContent = statsSummary(character);
    this.pendingRsnValue = character.display_rsn;
    this.pendingCharacterId = character.id;
    this.pendingError.textContent = "";
  }

  async confirmCharacter(characterId) {
    this.pendingError.textContent = "";
    try {
      const response = await accountApi.confirmCharacter(characterId);
      if (response.ok) {
        this.refresh();
      } else {
        this.pendingError.textContent = "Couldn't confirm that character — try again.";
      }
    } catch (error) {
      this.pendingError.textContent = "Couldn't confirm that character — try again.";
    }
  }

  promptRemoveCharacter(characterId, rsn) {
    confirmDialogManager.confirm({
      headline: `Remove ${escapeHtml(rsn)}?`,
      body: "This character will be permanently blocked from linking to your account again.",
      yesCallback: () => this.removePendingCharacter(characterId),
      noCallback: () => {},
    });
  }

  async removePendingCharacter(characterId) {
    this.pendingError.textContent = "";
    try {
      const response = await accountApi.removePendingCharacter(characterId);
      if (response.ok) {
        this.refresh();
      } else {
        this.pendingError.textContent = "Couldn't remove that character — try again.";
      }
    } catch (error) {
      this.pendingError.textContent = "Couldn't remove that character — try again.";
    }
  }

  handleChoiceClick(event) {
    const card = event.target.closest(".onboarding-page__choice-card");
    if (!card) return;
    if (card.dataset.choice === "create") {
      window.history.pushState("", "", "/create-group");
      return;
    }
    this.choiceRow.hidden = true;
    this.joinPanel.hidden = false;
  }

  async joinGroup() {
    this.joinError.textContent = "";
    if (!this.joinToken.valid) return;
    const groupToken = this.joinToken.value;
    const separatorIndex = groupToken.indexOf("|");
    const groupName = separatorIndex === -1 ? "" : groupToken.slice(0, separatorIndex);
    if (!groupName) {
      this.joinError.textContent = "That doesn't look like a valid group token.";
      return;
    }
    try {
      this.joinSubmit.disabled = true;
      const response = await accountApi.linkCharacterToGroup(this.currentCharacterId, groupName, groupToken);
      if (response.ok) {
        storage.storeGroup(groupName, groupToken);
        window.history.pushState("", "", "/group");
      } else if (response.status === 401) {
        this.joinError.textContent = "Group not found, or the token is wrong.";
      } else if (response.status === 409) {
        this.joinError.textContent = "This character already belongs to a group.";
      } else {
        this.joinError.textContent = "Couldn't join that group — try again.";
      }
    } catch (error) {
      this.joinError.textContent = "Couldn't join that group — try again.";
    } finally {
      this.joinSubmit.disabled = false;
    }
  }
}

customElements.define("onboarding-page", OnboardingPage);
