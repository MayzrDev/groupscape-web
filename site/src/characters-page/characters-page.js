import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function initials(rsn) {
  return rsn.slice(0, 2).toUpperCase();
}

/**
 * Lists every character linked to the signed-in account as a grid of tiles, with an inline
 * "add a character" tile that expands the plugin-linking instructions in place — no separate
 * page or modal needed for a flow this short. Reached directly (there's no account dashboard
 * yet — see `site: account page`) so it does its own session check rather than relying on a
 * route wrapper, matching `link-page`.
 *
 * Character tiles show initials in place of a portrait — the live 3D character render is a
 * separate, unbuilt feature (no plugin capture, no renderer yet), so this page doesn't reserve
 * space for it.
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
            <div class="characters-page__tile-badge">${escapeHtml(initials(character.display_rsn))}</div>
            <span class="characters-page__tile-rsn">${escapeHtml(character.display_rsn)}</span>
            <span class="characters-page__tile-meta">Linked ${boundAt}</span>
          </div>
        `;
      })
      .join("");
  }
}

customElements.define("characters-page", CharactersPage);
