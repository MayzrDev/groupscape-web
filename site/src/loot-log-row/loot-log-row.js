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
    if (this.row?.source_type) this.setAttribute("data-source", this.row.source_type);
    if (this.row?.source_type === "clue" && this.row.clue_tier) {
      this.style.setProperty("--clue-tier-color", `var(--clue-${this.row.clue_tier})`);
    }
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  get itemName() {
    return this.row.item_name || `Item #${this.row.item_id}`;
  }

  get sourceLabel() {
    switch (this.row.source_type) {
      case "chest":
        return `opened ${this.row.source_name}`;
      case "clue": {
        const tier = this.row.clue_tier ? this.row.clue_tier[0].toUpperCase() + this.row.clue_tier.slice(1) : "";
        return `${tier} clue casket`.trim();
      }
      default:
        return `from ${this.row.source_name}`;
    }
  }

  get rarityLabel() {
    return RARITY_LABELS[this.row.rarity] || null;
  }

  get dropRateLabel() {
    return this.row.drop_rate || null;
  }

  get formattedValue() {
    return `${this.row.total_value.toLocaleString()} gp`;
  }
}

customElements.define("loot-log-row", LootLogRow);
