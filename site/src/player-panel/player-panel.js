import { BaseElement } from "../base-element/base-element";
import { pubsub } from "../data/pubsub";

export class PlayerPanel extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{player-panel.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    if (!this.contentArea) {
      this.renderPanel();
    }
  }

  renderPanel() {
    this.render();
    this.contentArea = this.querySelector(".player-panel__content");
    this.eventListener(this.querySelector(".player-panel__minibar"), "click", this.handleMiniBarClick.bind(this));
    this.eventListener(
      this.querySelector(".player-panel__collection-log"),
      "click",
      this.handleCollectionLogClick.bind(this)
    );
    this.eventListener(
      this.querySelector(".player-panel__combat-achievements"),
      "click",
      this.handleCombatAchievementsClick.bind(this)
    );

    this.followButton = this.querySelector(".player-panel__follow");
    this.eventListener(this.followButton, "click", this.handleFollowClick.bind(this));
    this.subscribe("player-followed", this.handleFollowedChanged.bind(this));
  }

  handleFollowClick() {
    const worldMap = document.querySelector("#background-worldmap");
    if (!worldMap) return;

    if (worldMap.followingPlayer.name === this.playerName) {
      worldMap.stopFollowingPlayer();
      pubsub.publish("player-followed", null);
    } else {
      worldMap.followPlayer(this.playerName);
      pubsub.publish("player-followed", worldMap.followingPlayer.name);
      if (window.location.pathname !== "/group/map") {
        window.history.pushState("", "", "/group/map");
      }
    }
  }

  handleFollowedChanged(followedName) {
    this.followButton.classList.toggle("player-panel__follow--active", followedName === this.playerName);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleCollectionLogClick() {
    const collectionLogEl = document.createElement("collection-log");
    collectionLogEl.setAttribute("player-name", this.playerName);
    document.body.appendChild(collectionLogEl);
  }

  handleCombatAchievementsClick() {
    const combatAchievementsEl = document.createElement("combat-achievements");
    combatAchievementsEl.setAttribute("player-name", this.playerName);
    document.body.appendChild(combatAchievementsEl);
  }

  handleMiniBarClick(event) {
    const component = event.target.getAttribute("data-component");
    if (component && this.activeComponent !== component) {
      const showPortrait = component === "player-equipment";
      this.contentArea.innerHTML = `<${component} player-name="${this.playerName}" show-portrait="${showPortrait}"></${component}>`;

      if (this.activeComponent) {
        this.querySelector(`button[data-component="${this.activeComponent}"]`).classList.remove(
          "player-panel__tab-active"
        );
      }
      this.querySelector(`button[data-component="${component}"]`).classList.add("player-panel__tab-active");
      this.activeComponent = component;
      this.classList.add("expanded");
      // The equipment tab's bonus-stats layout needs more room than the fixed side-panel width
      // gives every other tab - see side-panel.css's `:has()` rule widening the whole side-panel
      // while this class is present, rather than every tab sharing one wider width.
      this.classList.toggle("player-panel--equipment-active", component === "player-equipment");
    } else if (this.activeComponent && this.activeComponent === component) {
      this.contentArea.innerHTML = "";
      this.querySelector(`button[data-component="${this.activeComponent}"]`).classList.remove(
        "player-panel__tab-active"
      );
      this.activeComponent = null;
      this.classList.remove("expanded");
      this.classList.remove("player-panel--equipment-active");
    }
  }
}
customElements.define("player-panel", PlayerPanel);
