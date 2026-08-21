import { BaseElement } from "../base-element/base-element";

export class SetupInstructions extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{setup-instructions.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }
}

customElements.define("setup-instructions", SetupInstructions);
