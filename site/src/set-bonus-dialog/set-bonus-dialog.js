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

  async renderSets() {
    const sets = await detectActiveSets(this.equippedItemIds);
    this.list.innerHTML = sets
      .map((set) => {
        const statusClass = set.active
          ? "set-bonus-dialog__set--active"
          : set.partial
          ? "set-bonus-dialog__set--partial"
          : "";
        const status = set.active ? "Active" : set.partial ? "Partial" : "Not worn";
        const missing =
          set.partial || (!set.active && set.missingItemIds.length === set.pieceCount)
            ? `<p class="set-bonus-dialog__missing">Missing ${set.missingItemIds.length} of ${set.pieceCount} pieces</p>`
            : "";

        return `
          <div class="set-bonus-dialog__set rsborder-tiny ${statusClass}">
            <div class="set-bonus-dialog__set-head">
              <span class="set-bonus-dialog__set-name">${set.name}</span>
              <span class="set-bonus-dialog__set-status">${status}</span>
            </div>
            <p class="set-bonus-dialog__effect">${set.effect}</p>
            ${missing}
          </div>
        `;
      })
      .join("");
  }
}
customElements.define("set-bonus-dialog", SetBonusDialog);
