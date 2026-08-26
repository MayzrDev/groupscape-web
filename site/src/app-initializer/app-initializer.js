import { BaseElement } from "../base-element/base-element";
import { Item } from "../data/item";
import { Quest } from "../data/quest";
import { api } from "../data/api";
import { storage } from "../data/storage";
import { pubsub } from "../data/pubsub";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";
import { AchievementDiary } from "../data/diaries";
import { toastSource } from "../data/toast-source";
import { combatAchievement } from "../data/combat-achievement";

export class AppInitializer extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{app-initializer.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.initializeApp();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.cleanup();
  }

  cleanup() {
    api.disable();
    toastSource.disable();
    // Unpublish everything to prevent any data leaking over into another session
    pubsub.unpublishAll();
    loadingScreenManager.hideLoadingScreen();
  }

  async initializeApp() {
    this.cleanup();
    loadingScreenManager.showLoadingScreen();
    await Promise.all([
      Item.loadItems(),
      Item.loadGePrices(),
      Quest.loadQuests(),
      AchievementDiary.loadDiaries(),
      combatAchievement.initCatalog(),
    ]);
    const group = storage.getGroup();

    // Make sure this component is still connected after loading the above. We don't want to start
    // making requests for group data if the user navigated away before the preload completed.
    if (this.isConnected) {
      // "@EXAMPLE" is a real, seeded, read-only demo group (see server's `seed` binary) -
      // loaded through the normal API path like any other group, not a client-side mock.
      await this.loadGroup(group);

      loadingScreenManager.hideLoadingScreen();
    }
  }

  async loadGroup(group) {
    const firstDataEvent = pubsub.waitUntilNextEvent("get-group-data", false);
    await api.enable(group.groupName, group.groupToken);
    await firstDataEvent;
    toastSource.enable();
  }
}

customElements.define("app-initializer", AppInitializer);
