import { BaseElement } from "../base-element/base-element";
import { storage } from "../data/storage";

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

  get hasLogin() {
    const group = storage.getGroup();
    return group && group.groupName && group.groupToken && group.groupName !== "@EXAMPLE";
  }
}

customElements.define("homepage", Homepage);
