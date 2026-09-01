import { BaseElement } from "../base-element/base-element";
import { Item } from "../data/item";

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
    for (const row of this.mergedRows()) {
      const tile = document.createElement("loot-log-tile");
      tile.row = row;
      tile.showKillers = this.group.showKillers;
      grid.appendChild(tile);
    }
  }

  // Server sends one row per (member, source, item); items unrecognized by the client's item
  // table (no name/image/value to render) are dropped rather than shown blank, and rows for the
  // same item dropped by different members are folded into one tile with a summed quantity.
  mergedRows() {
    const merged = new Map();
    for (const row of this.group.rows) {
      if (!Item.exists(row.item_id)) continue;
      if (!merged.has(row.item_id)) {
        merged.set(row.item_id, { ...row, quantity: 0, total_value: 0, killers: [], killerBreakdown: [] });
      }
      const entry = merged.get(row.item_id);
      entry.quantity += row.quantity;
      entry.total_value += row.total_value;
      entry.killers.push(row.member_name);
      entry.killerBreakdown.push({ memberName: row.member_name, quantity: row.quantity });
    }
    return [...merged.values()];
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

  // Bosses get a chathead icon hotlinked from the wiki's file-path redirect - no backend
  // scraping, so a boss whose page doesn't use the usual "<Name> chathead.png" filename just
  // falls back to the plain source dot via the <img>'s onerror in loot-log-group.html.
  get bossIconUrl() {
    if (this.group.sourceType !== "kill") return null;
    return `https://oldschool.runescape.wiki/w/Special:FilePath/${encodeURIComponent(
      `${this.group.sourceName} chathead.png`
    )}`;
  }

  get totalValue() {
    return this.group.rows.reduce((sum, row) => sum + row.total_value, 0);
  }
}
customElements.define("loot-log-group", LootLogGroup);
