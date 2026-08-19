import { BaseElement } from "../base-element/base-element";

const RARITY_LABELS = {
  common: "Common",
  uncommon: "Uncommon",
  rare: "Rare",
  very_rare: "Very Rare",
};

export class LootLogRow extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{loot-log-row.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    if (this.row?.rarity) this.setAttribute("data-rarity", this.row.rarity);
    if (this.row?.is_unique) this.setAttribute("data-unique", "true");
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  get itemName() {
    return this.row.item_name || `Item #${this.row.item_id}`;
  }

  get rarityLabel() {
    return RARITY_LABELS[this.row.rarity] || null;
  }

  get formattedValue() {
    return `${this.row.total_value.toLocaleString()} gp`;
  }
}

customElements.define("loot-log-row", LootLogRow);
