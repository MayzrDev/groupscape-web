import { BaseElement } from "../base-element/base-element";
import { detectActiveSets } from "../data/set-bonus";

/** Small popup listing curated item-set effects (Void, Barrows, God Wars, ...) and whether the
 * player currently has each active/partially equipped - created and appended on demand from
 * `player-equipment.js`'s "Set Bonus" button, matching `player-panel.js`'s
 * `collection-log`/`combat-achievements` pattern (a standalone dialog element owning its own
 * data, appended to `document.body` rather than mounted inline). Structurally mirrors
 * `diary-dialog` (same `.dialog`/`.dialog__container rsborder rsbackground` shell, close button,
 * background-click-to-close), just with a caller-supplied item id list instead of a pubsub
 * subscription, since the equipped-item set is already known synchronously by the caller. */
export class SetBonusDialog extends BaseElement {
  constructor() {
    super();
    // Set by the caller before appending to the DOM (see player-equipment.js).
    this.equippedItemIds = [];
    this.othersOpen = false;
  }

  html() {
    return `{{set-bonus-dialog.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    this.render();
    this.background = this.querySelector(".dialog__visible");
    this.list = this.querySelector(".set-bonus-dialog__list");

    this.eventListener(this.querySelector(".dialog__close"), "click", this.close.bind(this));
    this.eventListener(this.background, "click", this.closeIfBackgroundClick.bind(this));

    this.renderSets();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  closeIfBackgroundClick(evt) {
    if (evt.target === this.background) {
      this.close();
    }
  }

  close() {
    this.remove();
  }

  renderSetRow(set) {
    const wornPieces = set.pieceCount - set.missingItemIds.length;
    const status = set.active ? "Active" : `${wornPieces} / ${set.pieceCount} worn`;
    const missing = set.partial
      ? `<p class="set-bonus-dialog__missing">Missing ${set.missingItemIds.length} of ${set.pieceCount} pieces</p>`
      : "";

    return `
      <div class="set-bonus-dialog__set rsborder-tiny ${
        set.active ? "set-bonus-dialog__set--active" : "set-bonus-dialog__set--partial"
      }">
        <div class="set-bonus-dialog__set-head">
          <a class="set-bonus-dialog__set-name" href="${set.wikiUrl}" target="_blank" rel="noopener">${set.name}</a>
          <span class="set-bonus-dialog__set-status">${status}</span>
        </div>
        <p class="set-bonus-dialog__effect">${set.effect}</p>
        ${missing}
      </div>
    `;
  }

  renderOtherRow(set) {
    return `
      <div class="set-bonus-dialog__other">
        <a class="set-bonus-dialog__other-name" href="${set.wikiUrl}" target="_blank" rel="noopener"
          >${set.name}<span class="set-bonus-dialog__other-pieces">${set.pieceCount} piece${
      set.pieceCount === 1 ? "" : "s"
    }</span></a>
        <p class="set-bonus-dialog__other-effect">${set.effect}</p>
      </div>
    `;
  }

  async renderSets() {
    const sets = await detectActiveSets(this.equippedItemIds);
    const active = sets.filter((set) => set.active);
    const partial = sets.filter((set) => set.partial).sort((a, b) => a.missingItemIds.length - b.missingItemIds.length);
    const others = sets.filter((set) => !set.active && !set.partial).sort((a, b) => a.pieceCount - b.pieceCount);

    if (active.length === 0 && partial.length === 0) {
      this.othersOpen = true;
    }

    const activeSection = active.length
      ? `
        <section>
          <p class="set-bonus-dialog__tier-label">Active Sets</p>
          <div class="set-bonus-dialog__tier-list">${active.map((set) => this.renderSetRow(set)).join("")}</div>
        </section>
      `
      : "";

    const partialSection = partial.length
      ? `
        <section>
          <p class="set-bonus-dialog__tier-label">Partial Sets</p>
          <div class="set-bonus-dialog__tier-list">${partial.map((set) => this.renderSetRow(set)).join("")}</div>
        </section>
      `
      : "";

    const othersSection = others.length
      ? `
        <section class="set-bonus-dialog__others" data-open="${this.othersOpen}">
          <button class="set-bonus-dialog__others-toggle" aria-expanded="${
            this.othersOpen
          }" aria-controls="set-bonus-others-panel">
            <span>Other Sets (${others.length})</span>
            <span class="set-bonus-dialog__others-chevron" aria-hidden="true"></span>
          </button>
          <div class="set-bonus-dialog__others-panel" id="set-bonus-others-panel">
            <div class="set-bonus-dialog__others-panel-inner">
              <div class="set-bonus-dialog__others-list">${others.map((set) => this.renderOtherRow(set)).join("")}</div>
            </div>
          </div>
        </section>
      `
      : "";

    this.list.innerHTML = activeSection + partialSection + othersSection;

    const toggle = this.querySelector(".set-bonus-dialog__others-toggle");
    if (toggle) {
      this.eventListener(toggle, "click", this.toggleOthers.bind(this));
    }
  }

  toggleOthers() {
    this.othersOpen = !this.othersOpen;
    const section = this.querySelector(".set-bonus-dialog__others");
    const toggle = this.querySelector(".set-bonus-dialog__others-toggle");
    section.setAttribute("data-open", String(this.othersOpen));
    toggle.setAttribute("aria-expanded", String(this.othersOpen));
  }
}
customElements.define("set-bonus-dialog", SetBonusDialog);
