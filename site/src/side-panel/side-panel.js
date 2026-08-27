import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { storage } from "../data/storage";

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function initials(rsn) {
  return rsn.slice(0, 2).toUpperCase();
}

/**
 * Renders one `player-panel` per group member. When the group has no real members yet (a
 * brand-new group, or an account whose characters were detected/confirmed but never linked to
 * *this* group - see `characters-page`), shows an empty state that lets a logged-in user link
 * one of their confirmed characters right here instead of leaving a blank sidebar.
 *
 * Linking uses the group name/token already saved locally (`storage.getGroup()`, set at
 * creation/join time) so nothing needs to be typed - see `link-character-mockup` variant D.
 * Only characters with no existing group link are offered; a character already in a different
 * group has to be unlinked first from `/account/characters` (one-group-per-character, enforced
 * server-side).
 */
export class SidePanel extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{side-panel.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.sidePanels = this.querySelector(".side-panel__panels");
    this.empty = this.querySelector(".side-panel__empty");
    this.emptyTitle = this.querySelector(".side-panel__empty-title");
    this.emptyText = this.querySelector(".side-panel__empty-text");
    this.emptyLogin = this.querySelector(".side-panel__empty-login");
    this.emptyAction = this.querySelector(".side-panel__empty-action");
    this.emptyError = this.querySelector(".side-panel__empty-error");
    this.picker = this.querySelector(".side-panel__picker");
    this.pickerList = this.querySelector(".side-panel__picker-list");
    this.mobileToggle = this.querySelector(".side-panel__mobile-toggle");
    this.backdrop = this.querySelector(".side-panel__backdrop");

    this.eventListener(this.emptyAction, "click", this.togglePicker.bind(this));
    this.eventListener(this.pickerList, "click", this.handlePickerClick.bind(this));
    this.eventListener(this.mobileToggle, "click", this.toggleMobilePanel.bind(this));
    this.eventListener(this.backdrop, "click", this.closeMobilePanel.bind(this));

    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));
    this.subscribe("route-activated", this.closeMobilePanel.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleUpdatedMembers(members) {
    const realMembers = members.filter((member) => member.name !== "@SHARED");

    let playerPanels = "";
    for (const member of realMembers) {
      playerPanels += `<player-panel class="rsborder rsbackground" player-name="${member.name}"></player-panel>`;
    }
    this.sidePanels.innerHTML = playerPanels;

    this.empty.hidden = realMembers.length > 0;
    if (realMembers.length === 0) {
      this.refreshEmptyState();
    }
  }

  async refreshEmptyState() {
    this.emptyError.textContent = "";
    this.picker.hidden = true;
    this.emptyAction.hidden = true;
    this.emptyLogin.hidden = true;
    this.emptyText.classList.remove("side-panel__empty-text--linked");

    const session = await accountApi.me();
    if (!session.ok) {
      this.emptyTitle.textContent = "No characters linked";
      this.emptyText.textContent = "Log in to your GroupScape account to link a detected character to this group.";
      this.emptyLogin.hidden = false;
      return;
    }

    try {
      const response = await accountApi.listCharacters();
      if (!response.ok) {
        this.emptyTitle.textContent = "No characters linked";
        this.emptyText.textContent = "Couldn't load your characters — try again.";
        return;
      }
      const characters = await response.json();
      const { groupName } = storage.getGroup();
      const normalizedGroupName = (groupName ?? "").toLowerCase();
      const confirmed = characters.filter((character) => character.status === "confirmed");
      const linkedHere = confirmed.filter(
        (character) => character.group_name && character.group_name.toLowerCase() === normalizedGroupName
      );
      this.linkableCharacters = confirmed.filter((character) => character.group_id == null);

      if (linkedHere.length > 0) {
        // Already linked (this button click or a previous page load) - just waiting on the
        // RuneLite plugin's first telemetry update for this group, not something to re-link.
        this.emptyTitle.textContent = "Waiting for game data";
        this.emptyText.textContent = `${linkedHere
          .map((character) => character.display_rsn)
          .join(", ")} linked. Their panel will appear once the RuneLite plugin reports data.`;
        this.emptyText.classList.add("side-panel__empty-text--linked");
        return;
      }

      this.emptyTitle.textContent = "No characters linked";
      this.emptyText.textContent = "Link one of your RuneLite-detected characters to see their panel here.";
      this.emptyAction.hidden = false;
      this.emptyAction.textContent = "Link a character";
    } catch (error) {
      this.emptyTitle.textContent = "No characters linked";
      this.emptyText.textContent = "Couldn't load your characters — try again.";
    }
  }

  toggleMobilePanel() {
    const open = this.classList.toggle("side-panel--open");
    this.mobileToggle.setAttribute("aria-expanded", String(open));
    this.mobileToggle.setAttribute("aria-label", open ? "Hide player stats" : "Show player stats");
  }

  closeMobilePanel() {
    this.classList.remove("side-panel--open");
    this.mobileToggle.setAttribute("aria-expanded", "false");
    this.mobileToggle.setAttribute("aria-label", "Show player stats");
  }

  togglePicker() {
    const showing = !this.picker.hidden;
    this.picker.hidden = showing;
    this.emptyAction.textContent = showing ? "Link a character" : "Choose a character…";
    if (!showing) {
      this.renderPicker();
    }
  }

  renderPicker() {
    if (!this.linkableCharacters || this.linkableCharacters.length === 0) {
      this.pickerList.innerHTML = `
        <p class="side-panel__picker-foot">
          No confirmed characters yet? <men-link link-href="/account/characters">Confirm one first</men-link>
        </p>
      `;
      return;
    }

    this.pickerList.innerHTML = this.linkableCharacters
      .map(
        (character) => `
          <button class="side-panel__picker-row" data-character-id="${character.id}">
            <span class="side-panel__picker-avatar">${escapeHtml(initials(character.display_rsn))}</span>
            <span>${escapeHtml(character.display_rsn)}</span>
          </button>
        `
      )
      .join("");
  }

  handlePickerClick(event) {
    const row = event.target.closest(".side-panel__picker-row");
    if (!row) return;
    this.linkCharacter(row.dataset.characterId);
  }

  async linkCharacter(characterId) {
    this.emptyError.textContent = "";
    const character = this.linkableCharacters.find((c) => String(c.id) === String(characterId));
    const { groupName, groupToken } = storage.getGroup();

    try {
      const response = await accountApi.linkCharacterToGroup(characterId, groupName, groupToken);
      if (response.ok) {
        this.refreshEmptyState();
      } else if (response.status === 409) {
        this.emptyError.textContent = `${
          character?.display_rsn ?? "That character"
        } is already linked to another group — unlink it first from your characters page.`;
      } else {
        this.emptyError.textContent = "Couldn't link that character — try again.";
      }
    } catch (error) {
      this.emptyError.textContent = "Couldn't link that character — try again.";
    }
  }
}
customElements.define("side-panel", SidePanel);
