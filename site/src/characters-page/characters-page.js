import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function initials(rsn) {
  return rsn.slice(0, 2).toUpperCase();
}

/**
 * Lists every character linked to the signed-in account, split into two groups by `status`:
 * pending characters (auto-created server-side from plugin telemetry) need explicit
 * confirmation before they can join a group, shown above as richer "confirm cards" with a live
 * 3D portrait; confirmed characters render as the existing plain grid below, unchanged.
 *
 * Also has an inline "add a character" tile that expands the plugin-linking instructions in
 * place — no separate page or modal needed for a flow this short. Reached directly or via the
 * account dashboard's "Linked characters" card (`/account`), so it does its own session check
 * rather than relying on a route wrapper, matching `link-page`.
 *
 * Confirmed tiles show initials in place of a portrait — the live 3D character render is
 * reserved for the pending confirm cards where a positive ID before confirming matters most.
 *
 * Each confirmed tile has an unlink control (confirm dialog, then
 * `DELETE /api/account/characters/:id`) — RSN itself isn't editable here since it's derived
 * from plugin telemetry, not user-settable. Pending cards instead have Confirm
 * (`POST /api/account/characters/:id/confirm`) and Remove (confirm dialog, then
 * `DELETE /api/account/characters/:id/pending` — permanently blocks the character from
 * re-linking, unlike unlink).
 *
 * Confirmed tiles also show group status (`group_name`, only populated on this listing - see
 * `Character::group_id` doc in `models.rs`) with a Join/Leave action: Join opens
 * `.characters-page__join-dialog` to collect a group name + token for
 * `POST /api/account/characters/link-group`; Leave goes through the same confirm dialog as
 * unlink, then `POST /api/account/characters/:id/leave-group` (switching groups requires
 * leaving first - the backend rejects a direct switch).
 */
export class CharactersPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{characters-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.status = this.querySelector(".characters-page__status");
    this.pendingSection = this.querySelector(".characters-page__pending-section");
    this.pendingGrid = this.querySelector(".characters-page__pending-grid");
    this.grid = this.querySelector(".characters-page__grid");
    this.addTile = this.querySelector(".characters-page__add-tile");
    this.instructions = this.querySelector(".characters-page__instructions");
    this.instructionsClose = this.querySelector(".characters-page__instructions-close");
    this.error = this.querySelector(".characters-page__error");
    this.joinDialog = this.querySelector(".characters-page__join-dialog");
    this.joinDialogSub = this.querySelector(".characters-page__join-dialog-sub");
    this.joinDialogName = this.querySelector(".characters-page__join-dialog-name");
    this.joinDialogToken = this.querySelector(".characters-page__join-dialog-token");
    this.joinDialogError = this.querySelector(".characters-page__join-dialog-error");
    this.joinDialogSubmit = this.querySelector(".characters-page__join-dialog-submit");
    this.joinDialogCancel = this.querySelector(".characters-page__join-dialog-cancel");

    this.eventListener(this.addTile, "click", this.toggleInstructions.bind(this));
    this.eventListener(this.instructionsClose, "click", this.toggleInstructions.bind(this));
    this.eventListener(this.grid, "click", this.handleGridClick.bind(this));
    this.eventListener(this.pendingGrid, "click", this.handlePendingGridClick.bind(this));
    this.eventListener(this.joinDialogSubmit, "click", this.submitJoinGroup.bind(this));
    this.eventListener(this.joinDialogCancel, "click", this.hideJoinDialog.bind(this));

    this.checkSession();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  toggleInstructions() {
    const showing = !this.instructions.hidden;
    this.instructions.hidden = showing;
    this.addTile.setAttribute("aria-expanded", String(!showing));
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (response.ok) {
      this.fetchCharacters();
    } else {
      this.status.innerHTML =
        'You need to be logged into a GroupScape account. <men-link link-href="/account/login">Log in</men-link>';
      this.addTile.hidden = true;
    }
  }

  async fetchCharacters() {
    try {
      const response = await accountApi.listCharacters();
      if (!response.ok) {
        this.status.textContent = "";
        this.error.textContent = "Couldn't load your characters — try again.";
        return;
      }
      const characters = await response.json();
      this.renderCharacters(characters);
    } catch (error) {
      this.status.textContent = "";
      this.error.textContent = "Couldn't load your characters — try again.";
    }
  }

  renderCharacters(characters) {
    this.error.textContent = "";

    const pending = characters.filter((character) => character.status === "pending");
    const confirmed = characters.filter((character) => character.status !== "pending");

    this.status.textContent =
      confirmed.length === 0
        ? "You haven't linked any characters yet."
        : `${confirmed.length} character${confirmed.length === 1 ? "" : "s"} linked to your account.`;

    this.renderPendingCharacters(pending);

    this.grid.innerHTML = confirmed
      .map((character) => {
        const boundAt = new Date(character.bound_at).toLocaleDateString();
        const groupAction = character.group_name
          ? `
            <span class="characters-page__tile-group-pill">${escapeHtml(character.group_name)}</span>
            <button class="characters-page__tile-leave" data-character-id="${character.id}" data-rsn="${escapeHtml(
              character.display_rsn
            )}">Leave group</button>
          `
          : `
            <span class="characters-page__tile-group-pill characters-page__tile-group-pill--none">Not in a group</span>
            <button class="characters-page__tile-join" data-character-id="${character.id}" data-rsn="${escapeHtml(
              character.display_rsn
            )}">Join group</button>
          `;
        return `
          <div class="characters-page__tile">
            <button
              class="characters-page__tile-unlink"
              data-character-id="${character.id}"
              data-rsn="${escapeHtml(character.display_rsn)}"
              aria-label="Unlink ${escapeHtml(character.display_rsn)}"
              title="Unlink"
            >&times;</button>
            <div class="characters-page__tile-badge">${escapeHtml(initials(character.display_rsn))}</div>
            <span class="characters-page__tile-rsn">${escapeHtml(character.display_rsn)}</span>
            <span class="characters-page__tile-meta">Linked ${boundAt}</span>
            ${groupAction}
          </div>
        `;
      })
      .join("");
  }

  renderPendingCharacters(pending) {
    this.pendingSection.hidden = pending.length === 0;
    this.pendingGrid.innerHTML = pending
      .map((character) => {
        return `
          <div class="characters-page__pending-card">
            <player-portrait
              account-character-id="${character.id}"
              mode="bust"
              class="characters-page__pending-portrait"
            ></player-portrait>
            <span class="characters-page__pending-rsn">${escapeHtml(character.display_rsn)}</span>
            <div class="characters-page__pending-actions">
              <button
                class="characters-page__pending-confirm men-button small"
                data-character-id="${character.id}"
              >Confirm</button>
              <button
                class="characters-page__pending-remove"
                data-character-id="${character.id}"
                data-rsn="${escapeHtml(character.display_rsn)}"
              >Remove</button>
            </div>
          </div>
        `;
      })
      .join("");
  }

  handleGridClick(event) {
    const unlinkButton = event.target.closest(".characters-page__tile-unlink");
    if (unlinkButton) {
      const characterId = unlinkButton.dataset.characterId;
      const rsn = unlinkButton.dataset.rsn;
      confirmDialogManager.confirm({
        headline: "Unlink character?",
        body: `${rsn} will no longer be linked to this account. You can re-link it later from RuneLite.`,
        yesCallback: () => this.unlinkCharacter(characterId),
        noCallback: () => {},
      });
      return;
    }

    const joinButton = event.target.closest(".characters-page__tile-join");
    if (joinButton) {
      this.showJoinDialog(joinButton.dataset.characterId, joinButton.dataset.rsn);
      return;
    }

    const leaveButton = event.target.closest(".characters-page__tile-leave");
    if (leaveButton) {
      const characterId = leaveButton.dataset.characterId;
      const rsn = leaveButton.dataset.rsn;
      confirmDialogManager.confirm({
        headline: "Leave group?",
        body: `${rsn} will leave its current group. You can join a different group afterward.`,
        yesCallback: () => this.leaveGroup(characterId),
        noCallback: () => {},
      });
    }
  }

  showJoinDialog(characterId, rsn) {
    this.joinDialogCharacterId = characterId;
    this.joinDialogSub.textContent = `${rsn} will join once you enter its group's name and token.`;
    this.joinDialogName.input.value = "";
    this.joinDialogToken.input.value = "";
    this.joinDialogError.textContent = "";
    this.joinDialog.classList.add("dialog__visible");
  }

  hideJoinDialog() {
    this.joinDialog.classList.remove("dialog__visible");
  }

  async submitJoinGroup() {
    const groupName = this.joinDialogName.value;
    const groupToken = this.joinDialogToken.value;
    if (!groupName || !groupToken) {
      this.joinDialogError.textContent = "Enter both the group name and token.";
      return;
    }

    this.joinDialogError.textContent = "";
    try {
      const response = await accountApi.linkCharacterToGroup(this.joinDialogCharacterId, groupName, groupToken);
      if (response.ok) {
        this.hideJoinDialog();
        this.fetchCharacters();
      } else {
        this.joinDialogError.textContent = "Couldn't find that group — check the name and token.";
      }
    } catch (error) {
      this.joinDialogError.textContent = "Couldn't find that group — check the name and token.";
    }
  }

  async leaveGroup(characterId) {
    this.error.textContent = "";
    try {
      const response = await accountApi.leaveGroup(characterId);
      if (response.ok) {
        this.fetchCharacters();
      } else {
        this.error.textContent = "Couldn't leave that group — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't leave that group — try again.";
    }
  }

  handlePendingGridClick(event) {
    const confirmButton = event.target.closest(".characters-page__pending-confirm");
    if (confirmButton) {
      this.confirmCharacter(confirmButton.dataset.characterId);
      return;
    }

    const removeButton = event.target.closest(".characters-page__pending-remove");
    if (removeButton) {
      const characterId = removeButton.dataset.characterId;
      const rsn = removeButton.dataset.rsn;
      confirmDialogManager.confirm({
        headline: `Remove ${rsn}?`,
        body: "This character will be permanently blocked from linking to your account again.",
        yesCallback: () => this.removePendingCharacter(characterId),
        noCallback: () => {},
      });
    }
  }

  async confirmCharacter(characterId) {
    this.error.textContent = "";
    try {
      const response = await accountApi.confirmCharacter(characterId);
      if (response.ok) {
        this.fetchCharacters();
      } else {
        this.error.textContent = "Couldn't confirm that character — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't confirm that character — try again.";
    }
  }

  async removePendingCharacter(characterId) {
    this.error.textContent = "";
    try {
      const response = await accountApi.removePendingCharacter(characterId);
      if (response.ok) {
        this.fetchCharacters();
      } else {
        this.error.textContent = "Couldn't remove that character — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't remove that character — try again.";
    }
  }

  async unlinkCharacter(characterId) {
    this.error.textContent = "";
    try {
      const response = await accountApi.unlinkCharacter(characterId);
      if (response.ok) {
        this.fetchCharacters();
      } else {
        this.error.textContent = "Couldn't unlink that character — try again.";
      }
    } catch (error) {
      this.error.textContent = "Couldn't unlink that character — try again.";
    }
  }
}

customElements.define("characters-page", CharactersPage);
