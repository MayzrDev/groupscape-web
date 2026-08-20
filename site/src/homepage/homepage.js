import { BaseElement } from "../base-element/base-element";

export class Homepage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{homepage.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }
}

customElements.define("homepage-page", Homepage);
