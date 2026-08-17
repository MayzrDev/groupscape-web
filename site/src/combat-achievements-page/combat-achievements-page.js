import { BaseElement } from "../base-element/base-element";
import { pubsub } from "../data/pubsub";

export class CombatAchievementsPage extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{combat-achievements-page.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.render();

    this.membersContainer = this.querySelector(".combat-achievements-page__members");
    this.subscribe("members-updated", this.handleUpdatedMembers.bind(this));

    const [mostRecentMembers] = pubsub.getMostRecent("members-updated") || [];
    if (mostRecentMembers) {
      this.handleUpdatedMembers(mostRecentMembers);
    }
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  handleUpdatedMembers(members) {
    members = members.filter((member) => member.name !== "@SHARED");

    this.membersContainer.innerHTML = "";
    for (const member of members) {
      const memberRow = document.createElement("combat-achievement-member");
      memberRow.member = member;
      this.membersContainer.appendChild(memberRow);
    }
  }
}

customElements.define("combat-achievements-page", CombatAchievementsPage);
