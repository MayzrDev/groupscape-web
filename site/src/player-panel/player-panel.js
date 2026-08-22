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
    this.portraitMode = "bust";
    // side-panel (and its player-panel children) lives outside the route outlet and is never
    // remounted on navigation, so switching in/out of /panels has to be observed reactively
    // rather than checked once here. If /panels is already active this synchronously flips
    // portraitMode to "full" before the fallback render below runs.
    this.subscribe("panels-page-active", this.handlePanelsPageActive.bind(this));
    if (!this.contentArea) {
      this.renderPanel();
    }
  }

  handlePanelsPageActive(isPanelsPage) {
    const mode = isPanelsPage ? "full" : "bust";
    if (mode === this.portraitMode) return;
    this.portraitMode = mode;
    this.activeComponent = null;
    this.classList.remove("expanded");
    this.renderPanel();
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
      this.contentArea.innerHTML = `<${component} player-name="${this.playerName}"></${component}>`;

      if (this.activeComponent) {
        this.querySelector(`button[data-component="${this.activeComponent}"]`).classList.remove(
          "player-panel__tab-active"
        );
      }
      this.querySelector(`button[data-component="${component}"]`).classList.add("player-panel__tab-active");
      this.activeComponent = component;
      this.classList.add("expanded");
    } else if (this.activeComponent && this.activeComponent === component) {
      this.contentArea.innerHTML = "";
      this.querySelector(`button[data-component="${this.activeComponent}"]`).classList.remove(
        "player-panel__tab-active"
      );
      this.activeComponent = null;
      this.classList.remove("expanded");
    }
  }
}
customElements.define("player-panel", PlayerPanel);
