import { BaseElement } from "../base-element/base-element";
import { pubsub } from "../data/pubsub";

export class PanelsPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{panels-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    document.body.classList.add("panels-page");
    pubsub.publish("panels-page-active", true);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    document.body.classList.remove("panels-page");
    pubsub.publish("panels-page-active", false);
  }
}

customElements.define("panels-page", PanelsPage);
