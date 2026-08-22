import { BaseElement } from "../base-element/base-element";
import { utility } from "../utility";

export class PlayerInteracting extends BaseElement {
  constructor() {
    super();
    this.staleTimeout = 30 * 1000;
  }

  html() {
    return `{{player-interacting.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.fill = this.querySelector(".player-interacting__fill");
    this.name = this.querySelector(".player-interacting__name");
    this.hp = this.querySelector(".player-interacting__hp");
    this.map = document.querySelector("#background-worldmap");
    const playerName = this.getAttribute("player-name");

    this.addMapMarker().then(() => {
      this.subscribe(`interacting:${playerName}`, this.handleInteracting.bind(this));
    });
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    window.clearTimeout(this.hideTimeout);

    if (this.marker) {
      this.map.removeInteractingMarker(this.marker);
    }
  }

  handleInteracting(interacting) {
    if (!interacting || !interacting.last_updated || !interacting.location) {
      this.hide();
      return;
    }

    this.interacting = interacting;
    const timeSinceLastUpdate = utility.timeSinceLastUpdate(interacting.last_updated);
    const timeUntilHide = this.staleTimeout - timeSinceLastUpdate;

    if (timeUntilHide > 1000) {
      window.clearTimeout(this.hideTimeout);
      this.hideTimeout = window.setTimeout(this.hide.bind(this), this.staleTimeout);

      // Only an attackable actor (one with a combat health bar) gets a real ratio to show;
      // anything else (NPC/object interactions like banking) reads as a full neutral bar.
      const isEnemy = interacting.scale > 0;
      this.classList.toggle("player-interacting--neutral", !isEnemy);
      this.fill.style.width = isEnemy
        ? `${Math.max(0, Math.min(1, interacting.ratio / interacting.scale)) * 100}%`
        : "100%";

      this.hp.textContent = isEnemy ? `${interacting.ratio}/${interacting.scale}` : "";

      this.name.innerHTML = interacting.name;
      this.show();
    }
  }

  async addMapMarker() {
    this.marker = this.map.addInteractingMarker(0, 0, "");
  }

  hide() {
    this.classList.remove("player-interacting--visible");
    this.marker.coordinates = { x: -1000000, y: -1000000, plane: 0 };
  }

  show() {
    this.classList.add("player-interacting--visible");
    this.marker.coordinates = this.interacting.location;
    this.marker.label = this.interacting.name;
  }
}

customElements.define("player-interacting", PlayerInteracting);
