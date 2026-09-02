import { BaseElement } from "../base-element/base-element";
import { Item } from "../data/item";

const RARITY_LABELS = {
  common: "Common",
  uncommon: "Uncommon",
  rare: "Rare",
  very_rare: "Very Rare",
};

export class LootLogTile extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{loot-log-tile.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    if (this.item.rarity) this.setAttribute("data-rarity", this.item.rarity);
    if (this.item.is_unique) this.setAttribute("data-unique", "true");
    // `matched` is only ever explicitly `false` when a search is active and this specific item
    // failed an item-level clause (value/quantity/id) while the rest of its entry matched some
    // other way - see authed.rs's `build_matching_loot_log_event`. `true`/`undefined` (no search,
    // or the whole event matched by name/level) both render at full opacity.
    if (this.item.matched === false) this.setAttribute("data-dimmed", "true");
    this.enableTooltip();
    this.tooltipText = this.buildTooltip();
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  // Falls back to the full OSRS item table (present for every item id) rather than the raw id -
  // `item.item_name` is only ever set when the boss/item is in the curated drop-rate table, so
  // relying on it alone is what produced "Item #123" for anything outside that table.
  get itemName() {
    return this.item.item_name || Item.itemName(this.item.item_id);
  }

  get rarityLabel() {
    return RARITY_LABELS[this.item.rarity] || null;
  }

  get wikiLink() {
    return `https://oldschool.runescape.wiki/w/Special:Lookup?type=item&id=${this.item.item_id}`;
  }

  get isUntradeable() {
    return !this.item.unit_value;
  }

  buildTooltip() {
    const lines = [
      `${this.itemName}${this.item.is_unique ? ` <span class="loot-log-tile__tt-unique">Unique</span>` : ""}`,
      this.isUntradeable
        ? `<span class="loot-log-tile__tt-untradeable">Untradeable</span> &times; ${this.item.quantity.toLocaleString()}`
        : `${this.item.unit_value.toLocaleString()} gp &times; ${this.item.quantity.toLocaleString()} = <b>${this.item.total_value.toLocaleString()} gp</b>`,
    ];
    if (this.rarityLabel) {
      lines.push(`<span class="loot-log-tile__tt-rarity">${this.rarityLabel}</span>`);
    }
    return lines.join("<br />");
  }
}
customElements.define("loot-log-tile", LootLogTile);
