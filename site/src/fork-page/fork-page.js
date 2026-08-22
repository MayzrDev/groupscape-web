import { BaseElement } from "../base-element/base-element";

export class ForkPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{fork-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }
}

customElements.define("fork-page", ForkPage);
