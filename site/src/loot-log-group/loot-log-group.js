import { BaseElement } from "../base-element/base-element";

const COUNT_LABELS = {
  kill: "kills",
  chest: "opens",
  clue: "caskets",
};

export class LootLogGroup extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{loot-log-group.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.setAttribute("data-source", this.group.sourceType);
    if (this.group.sourceType === "clue" && this.group.clueTier) {
      this.style.setProperty("--clue-tier-color", `var(--clue-${this.group.clueTier})`);
    }
    this.render();
    const grid = this.querySelector(".loot-log-group__grid");
    for (const row of this.group.rows) {
      const tile = document.createElement("loot-log-tile");
      tile.row = row;
      grid.appendChild(tile);
    }
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  get sourceLabel() {
    const group = this.group;
    if (group.sourceType === "clue") {
      const tier = group.clueTier ? group.clueTier[0].toUpperCase() + group.clueTier.slice(1) : "";
      return `${tier} clue casket`.trim();
    }
    return group.sourceName;
  }

  get countLabel() {
    return COUNT_LABELS[this.group.sourceType] || "events";
  }

  get totalValue() {
    return this.group.rows.reduce((sum, row) => sum + row.total_value, 0);
  }
}
customElements.define("loot-log-group", LootLogGroup);
