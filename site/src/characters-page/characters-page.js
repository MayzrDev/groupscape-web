import { BaseElement } from "../base-element/base-element";
import { accountApi } from "../data/account-api";
import { accountStorage } from "../data/account-storage";
import { storage } from "../data/storage";
import { confirmDialogManager } from "../confirm-dialog/confirm-dialog-manager";

const ADD_POLL_INTERVAL_MS = 5000;

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

/**
 * Unified account-level characters & groups page (canonical route `/account/characters`;
 * `/characters` redirects here — see `index.html`). Merges what used to be two separate pages:
 * the old `characters-page` (confirm/unlink characters, join/leave a group) and `character-select`
 * (pick which group's dashboard to view). A character can only be in one group at a time
 * (`character_group_links.character_id` is a primary key server-side — see `db.rs`), so "join"
 * always targets a specific character, never the account as a whole.
 *
 * Layout is a roster sidebar (pending characters, then confirmed, then "+ Add a character") next
 * to a detail panel. Nothing is selected by default — the detail panel shows a summary (account
 * stats + every grouped character as an enterable/leaveable row) until a sidebar row is clicked.
 * Selection is pure client-side state (`this.selection`), not reflected in the URL.
 *
 * Reached directly or via the account dashboard's "Linked characters" card (`/account`), so it
 * does its own session check rather than relying on a route wrapper.
 */
export class CharactersPage extends BaseElement {
  constructor() {
    super();
    this.characters = [];
    this.selection = { type: "summary" };
  }

  html() {
    return `{{characters-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.status = this.querySelector(".characters-page__status");
    this.error = this.querySelector(".characters-page__error");
    this.layout = this.querySelector(".characters-page__layout");
    this.sidebar = this.querySelector(".characters-page__sidebar");
    this.detail = this.querySelector(".characters-page__detail");
    this.joinDialog = this.querySelector(".characters-page__join-dialog");
    this.joinDialogSub = this.querySelector(".characters-page__join-dialog-sub");
    this.joinDialogToken = this.querySelector(".characters-page__join-dialog-token");
    this.joinDialogError = this.querySelector(".characters-page__join-dialog-error");
    this.joinDialogSubmit = this.querySelector(".characters-page__join-dialog-submit");
    this.joinDialogCancel = this.querySelector(".characters-page__join-dialog-cancel");

    this.eventListener(this.sidebar, "click", this.handleSidebarClick.bind(this));
    this.eventListener(this.detail, "click", this.handleDetailClick.bind(this));
    this.eventListener(this.joinDialogSubmit, "click", this.submitJoinGroup.bind(this));
    this.eventListener(this.joinDialogCancel, "click", this.hideJoinDialog.bind(this));

    this.checkSession();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.stopAddPolling();
  }

  async checkSession() {
    this.status.textContent = "Checking your session…";
    const response = await accountApi.me();
    if (response.ok) {
      this.fetchCharacters();
    } else {
      this.status.innerHTML =
        'You need to be logged into a GroupScape account. <men-link link-href="/account/login">Log in</men-link>';
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
      this.characters = await response.json();
      this.status.textContent = "";
      this.error.textContent = "";
      this.layout.hidden = false;

      // While the "add" panel is up and polling, a newly-reported character jumps straight to
      // its own detail (mirrors onboarding's auto-advance) instead of leaving the user staring
      // at the same static instructions after the plugin has already done its part.
      const newlyPending = this.pending[0];
      if (this.selection.type === "add" && newlyPending) {
        this.stopAddPolling();
        this.selectCharacter(String(newlyPending.id));
        return;
      }

      // Selecting a character that no longer exists (unlinked/removed/confirmed-and-gone) falls
      // back to the summary rather than rendering a detail panel for nothing.
      if (this.selection.type === "character" && !this.findCharacter(this.selection.id)) {
        this.selection = { type: "summary" };
      }

      this.renderSidebar();
      this.renderDetail();
    } catch (error) {
      this.status.textContent = "";
      this.error.textContent = "Couldn't load your characters — try again.";
    }
  }

  findCharacter(characterId) {
    return this.characters.find((character) => String(character.id) === String(characterId));
  }

  get pending() {
    return this.characters.filter((character) => character.status === "pending");
  }

  get confirmed() {
    return this.characters.filter((character) => character.status !== "pending");
  }

  get grouped() {
    return this.confirmed.filter((character) => character.group_name);
  }

  selectSummary() {
    this.stopAddPolling();
    this.selection = { type: "summary" };
    this.renderDetail();
    this.updateActiveSidebarItem();
  }

  selectCharacter(characterId) {
    this.stopAddPolling();
    this.selection = { type: "character", id: characterId };
    this.renderDetail();
    this.updateActiveSidebarItem();
  }

  selectAdd() {
    this.selection = { type: "add" };
    this.renderDetail();
    this.updateActiveSidebarItem();
    this.startAddPolling();
  }

  startAddPolling() {
    this.stopAddPolling();
    this.addPollHandle = setInterval(() => this.fetchCharacters(), ADD_POLL_INTERVAL_MS);
  }

  stopAddPolling() {
    if (this.addPollHandle) {
      clearInterval(this.addPollHandle);
      this.addPollHandle = null;
    }
  }

  updateActiveSidebarItem() {
    this.sidebar.querySelectorAll("[data-select]").forEach((element) => {
      const isActive =
        (this.selection.type === "add" && element.dataset.select === "add") ||
        (this.selection.type === "character" &&
          element.dataset.select === "character" &&
          element.dataset.characterId === String(this.selection.id));
      element.classList.toggle("characters-page__sidebar-item--active", isActive);
    });
  }

  renderSidebar() {
    const pending = this.pending;
    const confirmed = this.confirmed;

    this.sidebar.innerHTML = `
      ${
        pending.length > 0
          ? `
        <div class="characters-page__sidebar-section">
          <div class="characters-page__sidebar-label">Pending (${pending.length})</div>
          ${pending.map((character) => this.sidebarRow(character)).join("")}
        </div>
      `
          : ""
      }
      <div class="characters-page__sidebar-section">
        <div class="characters-page__sidebar-label">Characters (${confirmed.length})</div>
        ${
          confirmed.length > 0
            ? confirmed.map((character) => this.sidebarRow(character)).join("")
            : `<p class="characters-page__sidebar-empty">No confirmed characters yet.</p>`
        }
      </div>
      <button class="characters-page__sidebar-add" data-select="add" type="button">
        <span class="characters-page__sidebar-add-plus">+</span> Add a character
      </button>
    `;
    this.updateActiveSidebarItem();
  }

  sidebarRow(character) {
    const isPending = character.status === "pending";
    return `
      <button
        class="characters-page__sidebar-item${isPending ? " characters-page__sidebar-item--pending" : ""}"
        data-select="character"
        data-character-id="${character.id}"
        type="button"
      >
        <player-portrait
          account-character-id="${character.id}"
          mode="bust"
          class="characters-page__sidebar-portrait"
        ></player-portrait>
        <span class="characters-page__sidebar-name">${escapeHtml(character.display_rsn)}</span>
        ${isPending ? `<span class="pill pill--pending">new</span>` : ""}
      </button>
    `;
  }

  renderDetail() {
    if (this.selection.type === "add") {
      this.detail.innerHTML = this.addDetail();
      this.apikeyBox = this.detail.querySelector(".characters-page__apikey-box");
      this.apikeyValue = this.detail.querySelector(".characters-page__apikey-value");
      this.apikeyRevealHint = this.detail.querySelector(".characters-page__apikey-reveal-hint");
      this.apikeyError = this.detail.querySelector(".characters-page__key-error");
      this.renderApiKeyBox();
      return;
    }

    if (this.selection.type === "character") {
      const character = this.findCharacter(this.selection.id);
      if (!character) {
        this.selection = { type: "summary" };
      } else {
        this.detail.innerHTML =
          character.status === "pending" ? this.pendingDetail(character) : this.confirmedDetail(character);
        return;
      }
    }

    this.detail.innerHTML = this.summaryDetail();
  }

  summaryDetail() {
    const confirmed = this.confirmed;
    const pending = this.pending;
    const grouped = this.grouped;

    if (this.characters.length === 0) {
      return `
        <div class="characters-page__empty-state">
          <h2>Link your first character</h2>
          <p>GroupScape reports characters through the RuneLite plugin — no code to type in.</p>
          <button class="men-button" data-action="add">Add a character</button>
        </div>
      `;
    }

    return `
      <div class="characters-page__summary-stats">
        <div class="characters-page__stat"><strong>${confirmed.length}</strong> character${
      confirmed.length === 1 ? "" : "s"
    }</div>
        ${
          pending.length > 0
            ? `<div class="characters-page__stat characters-page__stat--pending"><strong>${pending.length}</strong> pending</div>`
            : ""
        }
        <div class="characters-page__stat"><strong>${grouped.length}</strong> in a group</div>
      </div>
      <h2 class="characters-page__section-title">Your Groups</h2>
      ${
        grouped.length === 0
          ? `<p class="characters-page__summary-empty">None of your characters are in a group yet. Select a character to join one with an invite token.</p>`
          : grouped.map((character) => this.groupRow(character)).join("")
      }
    `;
  }

  groupRow(character) {
    return `
      <div class="characters-page__group-row">
        <player-portrait
          account-character-id="${character.id}"
          mode="bust"
          class="characters-page__group-row-portrait"
        ></player-portrait>
        <div class="characters-page__group-row-meta">
          <span class="characters-page__group-row-name">${escapeHtml(character.display_rsn)}</span>
          <span class="characters-page__group-row-group">${escapeHtml(character.group_name)}</span>
        </div>
        <button
          class="men-button small"
          data-action="enter-group"
          data-group-name="${escapeHtml(character.group_name)}"
          type="button"
        >Enter</button>
        <button
          class="characters-page__ghost-action"
          data-action="leave-group"
          data-character-id="${character.id}"
          data-rsn="${escapeHtml(character.display_rsn)}"
          data-group-name="${escapeHtml(character.group_name)}"
          type="button"
        >Leave</button>
      </div>
    `;
  }

  pendingDetail(character) {
    return `
      <div class="characters-page__detail-head">
        <player-portrait
          account-character-id="${character.id}"
          mode="bust"
          class="characters-page__detail-portrait"
        ></player-portrait>
        <div class="characters-page__detail-head-meta">
          <h2>${escapeHtml(character.display_rsn)}</h2>
          <p class="characters-page__detail-sub">
            Reported by the RuneLite plugin. Confirm it before it can join a group.
          </p>
          <div class="characters-page__detail-actions">
            <button class="men-button" data-action="confirm" data-character-id="${character.id}" type="button">
              Confirm character
            </button>
            <button
              class="characters-page__ghost-action characters-page__ghost-action--danger"
              data-action="dismiss"
              data-character-id="${character.id}"
              data-rsn="${escapeHtml(character.display_rsn)}"
              type="button"
            >Remove</button>
          </div>
        </div>
      </div>
    `;
  }

  confirmedDetail(character) {
    const boundAt = new Date(character.bound_at).toLocaleDateString();
    return `
      <div class="characters-page__detail-head">
        <player-portrait
          account-character-id="${character.id}"
          mode="bust"
          class="characters-page__detail-portrait"
        ></player-portrait>
        <div class="characters-page__detail-head-meta">
          <h2>${escapeHtml(character.display_rsn)}</h2>
          <p class="characters-page__detail-sub">Linked ${boundAt}</p>
          <div class="characters-page__detail-actions">
            <button
              class="characters-page__ghost-action characters-page__ghost-action--danger"
              data-action="unlink"
              data-character-id="${character.id}"
              data-rsn="${escapeHtml(character.display_rsn)}"
              type="button"
            >Unlink character</button>
          </div>
        </div>
      </div>
      <div class="characters-page__detail-section">
        <h3>Group</h3>
        ${
          character.group_name
            ? `
          <div class="characters-page__group-row">
            <div class="characters-page__group-row-meta">
              <span class="characters-page__group-row-group">${escapeHtml(character.group_name)}</span>
            </div>
            <button
              class="men-button small"
              data-action="enter-group"
              data-group-name="${escapeHtml(character.group_name)}"
              type="button"
            >Enter</button>
            <button
              class="characters-page__ghost-action"
              data-action="leave-group"
              data-character-id="${character.id}"
              data-rsn="${escapeHtml(character.display_rsn)}"
              data-group-name="${escapeHtml(character.group_name)}"
              type="button"
            >Leave</button>
          </div>
        `
            : `
          <p class="characters-page__summary-empty">Not in a group yet.</p>
          <button
            class="men-button small"
            data-action="join-group"
            data-character-id="${character.id}"
            data-rsn="${escapeHtml(character.display_rsn)}"
            type="button"
          >Join with a token</button>
        `
        }
      </div>
    `;
  }

  addDetail() {
    return `
      <div class="characters-page__detail-head">
        <h2>Add a character</h2>
      </div>
      <p class="characters-page__detail-sub">
        Paste this key into the RuneLite plugin so it can report your character.
      </p>
      <div class="characters-page__apikey-box" hidden>
        <code class="characters-page__apikey-value"></code>
        <button class="men-button small" data-action="copy-key" type="button">Copy</button>
      </div>
      <p class="characters-page__apikey-reveal-hint" hidden>
        This invalidates any key you generated before — you'll need to re-paste it into the plugin.
      </p>
      <ol class="characters-page__instructions-list">
        <li>Install the GroupScape plugin from the RuneLite Plugin Hub.</li>
        <li>Open its settings and paste your key above.</li>
        <li>Log into RuneScape — this page updates automatically.</li>
      </ol>
      <div class="characters-page__waiting">
        <span class="characters-page__spinner"></span> Waiting for your character to check in&hellip;
      </div>
      <div class="characters-page__key-error validation-error"></div>
    `;
  }

  renderApiKeyBox() {
    const freshApiKey = sessionStorage.getItem("freshApiKey");
    if (freshApiKey) {
      this.apikeyBox.hidden = false;
      this.apikeyValue.textContent = freshApiKey;
      this.apikeyRevealHint.hidden = false;
    } else {
      this.apikeyBox.hidden = true;
      this.apikeyRevealHint.hidden = true;
      this.fetchApiKey();
    }
  }

  async copyApiKey() {
    await navigator.clipboard.writeText(this.apikeyValue.textContent);
  }

  async fetchApiKey() {
    this.apikeyError.textContent = "";
    try {
      const response = await accountApi.regenerateApiKey();
      if (response.ok) {
        const { api_key } = await response.json();
        sessionStorage.setItem("freshApiKey", api_key);
        this.renderApiKeyBox();
      } else {
        this.apikeyError.textContent = "Couldn't generate your API key — try again.";
      }
    } catch (error) {
      this.apikeyError.textContent = "Couldn't generate your API key — try again.";
    }
  }

  handleSidebarClick(event) {
    const target = event.target.closest("[data-select]");
    if (!target) return;

    if (target.dataset.select === "add") {
      this.selectAdd();
    } else if (target.dataset.select === "character") {
      this.selectCharacter(target.dataset.characterId);
    }
  }

  handleDetailClick(event) {
    const target = event.target.closest("[data-action]");
    if (!target) return;
    const { action, characterId, rsn, groupName } = target.dataset;

    switch (action) {
      case "add":
        this.selectAdd();
        break;
      case "copy-key":
        this.copyApiKey();
        break;
      case "confirm":
        this.confirmCharacter(characterId);
        break;
      case "dismiss":
        confirmDialogManager.confirm({
          headline: `Remove ${rsn}?`,
          body: "This character will be permanently blocked from linking to your account again.",
          yesCallback: () => this.removePendingCharacter(characterId),
          noCallback: () => {},
        });
        break;
      case "unlink":
        confirmDialogManager.confirm({
          headline: "Unlink character?",
          body: `${rsn} will no longer be linked to this account. You can re-link it later from RuneLite.`,
          yesCallback: () => this.unlinkCharacter(characterId),
          noCallback: () => {},
        });
        break;
      case "join-group":
        this.showJoinDialog(characterId, rsn);
        break;
      case "leave-group":
        confirmDialogManager.confirm({
          headline: `Leave ${groupName}?`,
          body: `${rsn} will no longer be a member. You can join a different group afterward.`,
          yesCallback: () => this.leaveGroup(characterId),
          noCallback: () => {},
        });
        break;
      case "enter-group":
        this.viewGroup(groupName);
        break;
    }
  }

  viewGroup(groupName) {
    storage.storeGroup(groupName, accountStorage.getAccountToken());
    window.history.pushState("", "", "/group");
  }

  showJoinDialog(characterId, rsn) {
    this.joinDialogCharacterId = characterId;
    this.joinDialogSub.textContent = `${rsn} will join once you enter its group's token.`;
    this.joinDialogToken.input.value = "";
    this.joinDialogError.textContent = "";
    this.joinDialog.classList.add("dialog__visible");
  }

  hideJoinDialog() {
    this.joinDialog.classList.remove("dialog__visible");
  }

  async submitJoinGroup() {
    const groupToken = this.joinDialogToken.value;
    const separatorIndex = groupToken.indexOf("|");
    const groupName = separatorIndex === -1 ? "" : groupToken.slice(0, separatorIndex);
    if (!groupName) {
      this.joinDialogError.textContent = "Enter the group token.";
      return;
    }

    this.joinDialogError.textContent = "";
    try {
      const response = await accountApi.linkCharacterToGroup(this.joinDialogCharacterId, groupName, groupToken);
      if (response.ok) {
        this.hideJoinDialog();
        this.fetchCharacters();
      } else {
        this.joinDialogError.textContent = "Couldn't find that group — check the token.";
      }
    } catch (error) {
      this.joinDialogError.textContent = "Couldn't find that group — check the token.";
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

  async confirmCharacter(characterId) {
    this.error.textContent = "";
    try {
      const response = await accountApi.confirmCharacter(characterId);
      if (response.ok) {
        this.selection = { type: "character", id: characterId };
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
