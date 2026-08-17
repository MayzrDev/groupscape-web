/* global hcaptcha */
import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { storage } from "../data/storage";
import { validCharacters, validLength } from "../validators";
import { loadingScreenManager } from "../loading-screen/loading-screen-manager";

export class CreateGroup extends BaseElement {
  constructor() {
    super();
  }

  html() {
    return `{{create-group.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    loadingScreenManager.showLoadingScreen();
    this.initCaptcha().then(() => {
      loadingScreenManager.hideLoadingScreen();
      if (!this.isConnected) return;

      this.render();
      this.groupName = this.querySelector(".create-group__name");
      this.groupName.validators = [
        (value) => {
          return !validCharacters(value) ? "Group name has some unsupported special characters." : null;
        },
        (value) => {
          return !validLength(value) ? "Group name must be between 1 and 16 characters." : null;
        },
      ];
      this.serverError = this.querySelector(".create-group__server-error");
      this.querySelector(".create-group__submit").style.display = "block";

      this.eventListener(this.querySelector(".create-group__submit"), "click", this.createGroup.bind(this));

      if (this.captchaEnabled) {
        this.captchaWidgetID = hcaptcha.render("create-group__step-captcha", {
          sitekey: this.sitekey,
          theme: "dark",
        });
      }
    });
  }

  disconnectedCallback() {
    super.disconnectedCallback();

    // NOTE: For some reason hcaptcha only works on the first widget so we have to just
    // destroy it after we leave and import again later.
    if (this.captchaEnabled) {
      document.getElementById("hcaptcha").remove();
      window.hcaptcha = undefined;
    }
  }

  async createGroup() {
    this.serverError.innerHTML = "";
    if (!this.groupName.valid) {
      return;
    }

    let captchaResponse = "";
    if (this.captchaEnabled) {
      captchaResponse = hcaptcha.getResponse(this.captchaWidgetID);

      if (!captchaResponse) {
        this.serverError.innerHTML = "Complete the captcha";
        return;
      }
    }

    const groupName = this.groupName.value;
    const memberNames = [];

    const submitBtn = this.querySelector(".create-group__submit");
    try {
      submitBtn.disabled = true;
      const result = await api.createGroup(groupName, memberNames, captchaResponse);
      if (!result.ok) {
        const message = await result.text();
        this.serverError.innerHTML = `Error creating group: ${message}`;
      } else {
        const createdGroup = await result.json();

        storage.storeGroup(createdGroup.name, createdGroup.token);
        window.history.pushState("", "", "/setup-instructions");
      }
    } catch (err) {
      this.serverError.innerHTML = `Error creating group: ${err}`;
    } finally {
      submitBtn.disabled = false;
    }
  }

  async initCaptcha() {
    const captchaEnabled = await api.getCaptchaEnabled();
    this.captchaEnabled = captchaEnabled.enabled;
    this.sitekey = captchaEnabled.sitekey;

    if (this.captchaEnabled) {
      await this.waitForCaptchaScript();
    }
  }

  waitForCaptchaScript() {
    return new Promise((resolve) => {
      if (document.getElementById("hcaptcha")) resolve();
      window.menCaptchaLoaded = () => resolve();
      const script = document.createElement("script");
      script.id = "hcaptcha";
      script.src = "https://js.hcaptcha.com/1/api.js?render=explicit&onload=menCaptchaLoaded";
      document.body.appendChild(script);
    });
  }
}

customElements.define("create-group", CreateGroup);
