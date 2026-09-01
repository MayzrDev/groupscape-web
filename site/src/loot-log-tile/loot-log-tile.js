import { BaseElement } from "../base-element/base-element";
import { Item } from "../data/item";

const RARITY_LABELS = {
  common: "Common",
  uncommon: "Uncommon",
  rare: "Rare",
  very_rare: "Very Rare",
};

const MAX_KILLER_BADGES = 3;

export class LootLogTile extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{loot-log-tile.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    if (this.row.rarity) this.setAttribute("data-rarity", this.row.rarity);
    if (this.row.is_unique) this.setAttribute("data-unique", "true");
    this.enableTooltip();
    this.tooltipText = this.buildTooltip();
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  // Falls back to the full OSRS item table (present for every item id) rather than the raw id -
  // `row.item_name` is only ever set when the boss/item is in the curated drop-rate table, so
  // relying on it alone is what produced "Item #123" for anything outside that table.
  get itemName() {
    return this.row.item_name || Item.itemName(this.row.item_id);
  }

  get rarityLabel() {
    return RARITY_LABELS[this.row.rarity] || null;
  }

  buildTooltip() {
    const unitValue = this.row.unit_value ?? 0;
    const lines = [
      `${this.itemName}${this.row.is_unique ? ` <span class="loot-log-tile__tt-unique">Unique</span>` : ""}`,
      `${unitValue.toLocaleString()} gp &times; ${this.row.quantity.toLocaleString()} = <b>${this.row.total_value.toLocaleString()} gp</b>`,
    ];
    if (this.rarityLabel) {
      lines.push(`<span class="loot-log-tile__tt-rarity">${this.rarityLabel}</span>`);
    }
    if (this.row.killerBreakdown && this.row.killerBreakdown.length > 1) {
      lines.push(
        `<span class="loot-log-tile__tt-breakdown">${this.row.killerBreakdown
          .map(({ memberName, quantity }) => `${memberName}: ${quantity.toLocaleString()}`)
          .join("<br />")}</span>`
      );
    }
    return lines.join("<br />");
  }

  killerBadgesHtml() {
    const killers = this.row.killers;
    const shown = killers.slice(0, MAX_KILLER_BADGES);
    const overflow = killers.length - shown.length;
    const badges = shown
      .map(
        (memberName) =>
          `<span class="loot-log-tile__killer"><player-icon player-name="${memberName}"></player-icon></span>`
      )
      .join("");
    const more = overflow > 0 ? `<span class="loot-log-tile__killer-more">+${overflow}</span>` : "";
    return `${more}${badges}`;
  }
}
customElements.define("loot-log-tile", LootLogTile);
