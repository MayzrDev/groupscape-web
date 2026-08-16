import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

/**
 * Lists every character linked to the signed-in account. Reached directly (there's no account
 * dashboard yet — see `site: account page`) so it does its own session check rather than relying
 * on a route wrapper, matching `link-page`.
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
    this.list = this.querySelector(".characters-page__list");
    this.empty = this.querySelector(".characters-page__empty");
    this.error = this.querySelector(".characters-page__error");

    this.checkSession();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (response.ok) {
      this.fetchCharacters();
    } else {
      this.status.textContent = "You need to be logged into a GroupScape account to manage your characters.";
      this.empty.innerHTML = '<men-link link-href="/account/login">Log in</men-link>';
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

    if (characters.length === 0) {
      this.status.textContent = "You haven't linked any characters yet.";
      this.list.innerHTML = "";
      this.empty.textContent = "";
      return;
    }

    this.status.textContent = `${characters.length} character${
      characters.length === 1 ? "" : "s"
    } linked to your account.`;
    this.empty.textContent = "";
    this.list.innerHTML = characters
      .map((character) => {
        const boundAt = new Date(character.bound_at).toLocaleDateString();
        return `
          <li class="characters-page__item">
            <span class="characters-page__item-rsn">${escapeHtml(character.display_rsn)}</span>
            <span class="characters-page__item-meta">Linked ${boundAt}</span>
          </li>
        `;
      })
      .join("");
  }
}

customElements.define("characters-page", CharactersPage);
