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
 * Lists every character linked to the signed-in account as a grid of tiles, with an inline
 * "add a character" tile that expands the plugin-linking instructions in place — no separate
 * page or modal needed for a flow this short. Reached directly or via the account dashboard's
 * "Linked characters" card (`/account`), so it does its own session check rather than relying
 * on a route wrapper, matching `link-page`.
 *
 * Character tiles show initials in place of a portrait — the live 3D character render is a
 * separate, unbuilt feature (no plugin capture, no renderer yet), so this page doesn't reserve
 * space for it.
 *
 * Each tile has an unlink control (confirm dialog, then `DELETE /api/account/characters/:id`)
 * — RSN itself isn't editable here since it's derived from plugin telemetry, not user-settable.
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
    this.grid = this.querySelector(".characters-page__grid");
    this.addTile = this.querySelector(".characters-page__add-tile");
    this.instructions = this.querySelector(".characters-page__instructions");
    this.instructionsClose = this.querySelector(".characters-page__instructions-close");
    this.error = this.querySelector(".characters-page__error");

    this.eventListener(this.addTile, "click", this.toggleInstructions.bind(this));
    this.eventListener(this.instructionsClose, "click", this.toggleInstructions.bind(this));
    this.eventListener(this.grid, "click", this.handleGridClick.bind(this));

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
    this.status.textContent =
      characters.length === 0
        ? "You haven't linked any characters yet."
        : `${characters.length} character${characters.length === 1 ? "" : "s"} linked to your account.`;

    this.grid.innerHTML = characters
      .map((character) => {
        const boundAt = new Date(character.bound_at).toLocaleDateString();
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
          </div>
        `;
      })
      .join("");
  }

  handleGridClick(event) {
    const unlinkButton = event.target.closest(".characters-page__tile-unlink");
    if (!unlinkButton) return;

    const characterId = unlinkButton.dataset.characterId;
    const rsn = unlinkButton.dataset.rsn;
    confirmDialogManager.confirm({
      headline: "Unlink character?",
      body: `${rsn} will no longer be linked to this account. You can re-link it later from RuneLite.`,
      yesCallback: () => this.unlinkCharacter(characterId),
      noCallback: () => {},
    });
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
