import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { accountStorage } from "../data/account-storage";
import { storage } from "../data/storage";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function initials(rsn) {
  return rsn.slice(0, 2).toUpperCase();
}

/**
 * Landing page for a signed-in account with at least one character that's both confirmed and
 * in a group - lets them pick which group's dashboard to view, or leave one to free the
 * character up for a different group. Reached from the homepage when an account token is
 * present; falls back to `/welcome` itself if nothing here is actually ready to view yet
 * (mirrors `onboarding-page`'s own state checks, since this page doesn't handle linking or
 * confirming characters at all - that's `onboarding-page`'s job).
 *
 * "View group" works even on a device that never had the group's token saved locally: it
 * stores the account's own session token in the group-token slot, and the server's group auth
 * middleware accepts that as proof of membership for any group one of this account's confirmed
 * characters is linked to (see `auth_middleware`'s account-session fallback).
 */
export class CharacterSelect extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{character-select.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.status = this.querySelector(".character-select__status");
    this.content = this.querySelector(".character-select__content");
    this.list = this.querySelector(".character-select__list");
    this.error = this.querySelector(".character-select__error");

    this.eventListener(this.list, "click", this.handleListClick.bind(this));

    this.checkSession();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (!response.ok) {
      window.history.pushState("", "", "/account/login");
      return;
    }
    await this.refresh();
  }

  async refresh() {
    const response = await accountApi.listCharacters();
    if (!response.ok) {
      this.status.textContent = "Couldn't load your characters — try refreshing.";
      return;
    }
    const characters = await response.json();
    const ready = characters.filter((character) => character.status !== "pending" && character.group_id != null);

    if (ready.length === 0) {
      window.history.pushState("", "", "/welcome");
      return;
    }

    this.status.textContent = "";
    this.content.hidden = false;
    this.renderCharacters(ready);
  }

  renderCharacters(characters) {
    this.list.innerHTML = characters
      .map(
        (character) => `
          <div class="character-select__card">
            <div class="character-select__badge">${escapeHtml(initials(character.display_rsn))}</div>
            <div class="character-select__meta">
              <span class="character-select__rsn">${escapeHtml(character.display_rsn)}</span>
              <span class="character-select__group">${escapeHtml(character.group_name)}</span>
            </div>
            <button
              class="character-select__view men-button small"
              data-character-id="${character.id}"
              data-group-name="${escapeHtml(character.group_name)}"
            >View group</button>
            <button
              class="character-select__leave"
              data-character-id="${character.id}"
              data-rsn="${escapeHtml(character.display_rsn)}"
              data-group-name="${escapeHtml(character.group_name)}"
            >Leave group</button>
          </div>
        `
      )
      .join("");
  }

  handleListClick(event) {
    const viewButton = event.target.closest(".character-select__view");
    if (viewButton) {
      this.viewGroup(viewButton.dataset.groupName);
      return;
    }

    const leaveButton = event.target.closest(".character-select__leave");
    if (leaveButton) {
      const { characterId, rsn, groupName } = leaveButton.dataset;
      confirmDialogManager.confirm({
        headline: `Leave ${escapeHtml(groupName)}?`,
        body: `${escapeHtml(rsn)} will no longer be a member. You can link it to a different group afterward.`,
        yesCallback: () => this.leaveGroup(characterId),
        noCallback: () => {},
      });
    }
  }

  viewGroup(groupName) {
    storage.storeGroup(groupName, accountStorage.getAccountToken());
    window.history.pushState("", "", "/group");
  }

  async leaveGroup(characterId) {
    this.error.textContent = "";
    try {
      const response = await accountApi.leaveGroup(characterId);
      if (response.ok) {
        this.refresh();
      } else {
        this.error.textContent = "Couldn't leave that group — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't leave that group — try again.";
    }
  }
}

customElements.define("character-select", CharacterSelect);
