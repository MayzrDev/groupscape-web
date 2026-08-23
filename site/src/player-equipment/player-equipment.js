import { BaseElement } from "../base-element/base-element";
import { fetchItemBonuses } from "../data/item-bonuses";
import { combatSkillLevelsFromMember, openDpsCalculator } from "../data/dps-calc-link";
import { targetBonusesFor } from "../data/target-bonuses";

const EquipmentSlot = {
  Head: 0,
  Back: 1,
  Neck: 2,
  Weapon: 3,
  Torso: 4,
  Shield: 5,
  Legs: 7,
  Gloves: 9,
  Boots: 10,
  Ring: 12,
  Ammo: 13,
};

const ZERO_TOTALS = {
  attackStab: 0,
  attackSlash: 0,
  attackCrush: 0,
  attackMagic: 0,
  attackRanged: 0,
  defenceStab: 0,
  defenceSlash: 0,
  defenceCrush: 0,
  defenceMagic: 0,
  defenceRanged: 0,
  meleeStrength: 0,
  rangedStrength: 0,
  magicDamage: 0,
  prayer: 0,
};

function formatBonus(value) {
  return value >= 0 ? `+${value}` : String(value);
}

function formatMagicDamage(tenths) {
  const percent = tenths / 10;
  const rounded = Number.isInteger(percent) ? percent : percent.toFixed(1);
  return `${rounded >= 0 ? "+" : ""}${rounded}%`;
}

function formatPercent(percent) {
  const rounded = Math.round(percent * 100) / 100;
  return `${rounded >= 0 ? "+" : ""}${rounded}%`;
}

export class PlayerEquipment extends BaseElement {
  constructor() {
    super();
    this.emptySlotImages = {
      [EquipmentSlot.Head]: "156-0.png",
      [EquipmentSlot.Back]: "157-0.png",
      [EquipmentSlot.Neck]: "158-0.png",
      [EquipmentSlot.Weapon]: "159-0.png",
      [EquipmentSlot.Torso]: "161-0.png",
      [EquipmentSlot.Shield]: "162-0.png",
      [EquipmentSlot.Legs]: "163-0.png",
      [EquipmentSlot.Gloves]: "164-0.png",
      [EquipmentSlot.Boots]: "165-0.png",
      [EquipmentSlot.Ring]: "160-0.png",
      [EquipmentSlot.Ammo]: "166-0.png",
    };
    this.equippedBySlot = {};
    this.member = null;
    // Bumped on every equipment update so a stats fetch that resolves after a newer update has
    // already started is discarded instead of clobbering fresher numbers on screen.
    this.statsRequestId = 0;
  }

  html() {
    return `{{player-equipment.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
    this.playerName = this.getAttribute("player-name");
    this.slotEls = {
      [EquipmentSlot.Head]: this.querySelector(".equipment-head"),
      [EquipmentSlot.Back]: this.querySelector(".equipment-cape"),
      [EquipmentSlot.Neck]: this.querySelector(".equipment-neck"),
      [EquipmentSlot.Weapon]: this.querySelector(".equipment-weapon"),
      [EquipmentSlot.Torso]: this.querySelector(".equipment-torso"),
      [EquipmentSlot.Shield]: this.querySelector(".equipment-shield"),
      [EquipmentSlot.Legs]: this.querySelector(".equipment-legs"),
      [EquipmentSlot.Gloves]: this.querySelector(".equipment-gloves"),
      [EquipmentSlot.Boots]: this.querySelector(".equipment-boots"),
      [EquipmentSlot.Ring]: this.querySelector(".equipment-ring"),
      [EquipmentSlot.Ammo]: this.querySelector(".equipment-ammo"),
    };
    this.statsEl = this.querySelector(".equipment-stats");
    this.statValueEls = {};
    for (const el of this.querySelectorAll(".equipment-stats__value")) {
      this.statValueEls[el.dataset.stat] = el;
    }

    this.eventListener(this.querySelector(".equipment-set-bonus-button"), "click", this.handleSetBonusClick.bind(this));
    this.eventListener(this.querySelector(".equipment-dps-button"), "click", this.handleDpsClick.bind(this));

    this.subscribe(`equipment:${this.playerName}`, this.handleUpdatedEquipment.bind(this));
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleUpdatedEquipment(equipment, member) {
    this.member = member;
    this.equippedBySlot = {};

    for (let position = 0; position < equipment.length; ++position) {
      const el = this.slotEls[position];
      // NOTE: Not every position has an equipment slot
      if (el === undefined) continue;
      const item = equipment[position];

      if (item.isValid()) {
        this.equippedBySlot[position] = item;
        const itemEl = document.createElement("item-box");
        itemEl.item = item;
        itemEl.setAttribute("player-name", this.playerName);
        itemEl.setAttribute("inventory-type", "equipment");
        el.innerHTML = "";
        el.appendChild(itemEl);
      } else {
        el.innerHTML = `<img loading="lazy" src="/ui/${this.emptySlotImages[position]}" />`;
      }
    }

    this.refreshStats();
  }

  async refreshStats() {
    const requestId = ++this.statsRequestId;
    const equippedItems = Object.values(this.equippedBySlot);

    if (equippedItems.length === 0) {
      this.statsEl.classList.remove("equipment-stats--loading");
      this.renderStats(ZERO_TOTALS, null, { undeadPercent: 0, slayerPercent: 0 });
      return;
    }

    this.statsEl.classList.add("equipment-stats--loading");

    const equippedItemIds = equippedItems.map((item) => item.id);
    const [results, targetBonuses] = await Promise.all([
      Promise.allSettled(equippedItems.map((item) => fetchItemBonuses(item.id))),
      targetBonusesFor(equippedItemIds),
    ]);
    if (requestId !== this.statsRequestId) return; // a newer equipment update superseded this fetch

    const totals = { ...ZERO_TOTALS };
    let weaponSpeedTicks = null;
    const weaponItem = this.equippedBySlot[EquipmentSlot.Weapon];

    for (let i = 0; i < results.length; ++i) {
      const result = results[i];
      if (result.status !== "fulfilled") continue;
      const bonuses = result.value;

      totals.attackStab += bonuses.attack.stab;
      totals.attackSlash += bonuses.attack.slash;
      totals.attackCrush += bonuses.attack.crush;
      totals.attackMagic += bonuses.attack.magic;
      totals.attackRanged += bonuses.attack.ranged;
      totals.defenceStab += bonuses.defence.stab;
      totals.defenceSlash += bonuses.defence.slash;
      totals.defenceCrush += bonuses.defence.crush;
      totals.defenceMagic += bonuses.defence.magic;
      totals.defenceRanged += bonuses.defence.ranged;
      totals.meleeStrength += bonuses.meleeStrength;
      totals.rangedStrength += bonuses.rangedStrength;
      totals.magicDamage += bonuses.magicDamage;
      totals.prayer += bonuses.prayer;

      if (equippedItems[i] === weaponItem && bonuses.attackSpeed != null) {
        weaponSpeedTicks = bonuses.attackSpeed;
      }
    }

    this.statsEl.classList.remove("equipment-stats--loading");
    this.renderStats(totals, weaponSpeedTicks, targetBonuses);
  }

  renderStats(totals, weaponSpeedTicks, targetBonuses) {
    for (const [stat, el] of Object.entries(this.statValueEls)) {
      if (stat === "magicDamage") {
        el.textContent = formatMagicDamage(totals.magicDamage);
      } else if (stat === "undead") {
        el.textContent = formatPercent(targetBonuses.undeadPercent);
      } else if (stat === "slayer") {
        el.textContent = formatPercent(targetBonuses.slayerPercent);
      } else if (stat === "speedBase" || stat === "speedActual") {
        continue; // handled below
      } else if (stat in totals) {
        el.textContent = formatBonus(totals[stat]);
      }
    }

    if (weaponSpeedTicks == null) {
      this.statValueEls.speedBase.textContent = "-";
      this.statValueEls.speedActual.textContent = "-";
    } else {
      this.statValueEls.speedBase.textContent = `${weaponSpeedTicks} ticks`;
      this.statValueEls.speedActual.textContent = `${(weaponSpeedTicks * 0.6).toFixed(1)}s`;
    }
  }

  handleSetBonusClick() {
    const equippedItemIds = Object.values(this.equippedBySlot).map((item) => item.id);
    const dialog = document.createElement("set-bonus-dialog");
    dialog.equippedItemIds = equippedItemIds;
    dialog.setAttribute("player-name", this.playerName);
    document.body.appendChild(dialog);
  }

  handleDpsClick() {
    const skillLevels = combatSkillLevelsFromMember(this.member);
    openDpsCalculator(this.equippedBySlot, skillLevels, this.playerName);
  }
}
customElements.define("player-equipment", PlayerEquipment);
