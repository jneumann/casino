import Phaser from "phaser";

import { api } from "../api.js";
import { sessionFromAuth, saveSession } from "../session.js";
import {
  COLORS,
  FONT_BODY,
  FONT_DISPLAY,
  GAME_HEIGHT,
  GAME_WIDTH,
  STARTING_BALANCE,
  css,
} from "../theme.js";
import { createTable } from "../ui/table.js";

const CARD_HTML = `
<div class="auth-card">
  <div class="auth-tabs" role="tablist">
    <button type="button" class="auth-tab is-active" data-mode="login" role="tab">Sign in</button>
    <button type="button" class="auth-tab" data-mode="register" role="tab">Create account</button>
  </div>

  <form class="auth-form" novalidate autocomplete="on">
    <label class="auth-field">
      <span>Username</span>
      <input name="username" type="text" autocomplete="username" autocapitalize="none"
             spellcheck="false" maxlength="24" required />
    </label>

    <label class="auth-field">
      <span>Password</span>
      <input name="password" type="password" autocomplete="current-password"
             maxlength="128" required />
    </label>

    <label class="auth-field" data-confirm hidden>
      <span>Confirm password</span>
      <input name="confirm" type="password" autocomplete="new-password" maxlength="128" />
    </label>

    <p class="auth-hint" data-hint></p>
    <p class="auth-alert" role="alert" data-alert hidden></p>

    <button class="auth-submit" type="submit" data-submit>Sign in</button>
  </form>
</div>
`;

// The card is anchored by its top edge, and these bound where that edge may
// sit: never above the wordmark, never so low that the form runs off the table.
const CARD_TOP_MIN = 200;
const CARD_BOTTOM_MARGIN = 26;
const CARD_REGION_CENTER = 460;

const COPY = {
  login: {
    submit: "Sign in",
    busy: "Signing in…",
    hint: "Welcome back to the table.",
    passwordAutocomplete: "current-password",
  },
  register: {
    submit: "Deal me in",
    busy: "Setting up your table…",
    hint: `Usernames are 3–24 characters. Passwords need at least 10. You start with ${STARTING_BALANCE.toLocaleString()} coins.`,
    passwordAutocomplete: "new-password",
  },
};

export default class AuthScene extends Phaser.Scene {
  constructor() {
    super("Auth");
  }

  init(data) {
    this.notice = data?.notice ?? null;
    this.mode = "login";
    this.busy = false;
  }

  create() {
    createTable(this);
    this.createBrand();
    this.createCard();

    if (this.notice) {
      this.showAlert(this.notice);
    }

    this.cameras.main.fadeIn(350, 0, 0, 0);
  }

  createBrand() {
    this.add
      .image(GAME_WIDTH / 2, 132, "glow")
      .setScale(2.6, 1.1)
      .setAlpha(0.5);

    const title = this.add
      .text(GAME_WIDTH / 2, 124, "CASINO ROYALE", {
        fontFamily: FONT_DISPLAY,
        fontSize: "56px",
        color: css(COLORS.gold),
      })
      .setOrigin(0.5)
      .setShadow(0, 4, "rgba(0, 0, 0, 0.6)", 12);

    if (typeof title.setLetterSpacing === "function") {
      title.setLetterSpacing(6);
    }

    this.add
      .text(
        GAME_WIDTH / 2,
        174,
        `Members only · every new player is staked ${STARTING_BALANCE.toLocaleString()} coins`,
        {
          fontFamily: FONT_BODY,
          fontSize: "16px",
          color: css(COLORS.muted),
        },
      )
      .setOrigin(0.5);
  }

  createCard() {
    // Anchored top-centre: Phaser offsets a DOM element by its *cached* height
    // times originY, and this card grows as fields and errors appear. An
    // originY of 0 keeps the anchor honest no matter how tall it gets.
    this.card = this.add
      .dom(GAME_WIDTH / 2, CARD_REGION_CENTER)
      .createFromHTML(CARD_HTML)
      .setOrigin(0.5, 0);

    const root = this.card.node;
    this.cardEl = root.querySelector(".auth-card");
    this.els = {
      form: root.querySelector(".auth-form"),
      tabs: [...root.querySelectorAll(".auth-tab")],
      username: root.querySelector('input[name="username"]'),
      password: root.querySelector('input[name="password"]'),
      confirm: root.querySelector('input[name="confirm"]'),
      confirmField: root.querySelector("[data-confirm]"),
      hint: root.querySelector("[data-hint]"),
      alert: root.querySelector("[data-alert]"),
      submit: root.querySelector("[data-submit]"),
    };

    this.els.tabs.forEach((tab) => {
      tab.addEventListener("click", () => this.setMode(tab.dataset.mode));
    });

    this.els.form.addEventListener("submit", (event) => {
      event.preventDefault();
      this.submit();
    });

    this.applyMode();
    this.els.username.focus();
  }

  setMode(mode) {
    if (this.busy || mode === this.mode) return;
    this.mode = mode;
    this.applyMode();
    this.els.username.focus();
  }

  applyMode() {
    const copy = COPY[this.mode];
    const registering = this.mode === "register";

    this.els.tabs.forEach((tab) => {
      tab.classList.toggle("is-active", tab.dataset.mode === this.mode);
    });

    this.els.confirmField.hidden = !registering;
    this.els.confirm.value = "";
    this.els.password.setAttribute("autocomplete", copy.passwordAutocomplete);
    this.els.hint.textContent = copy.hint;
    this.els.submit.textContent = copy.submit;
    this.hideAlert();
  }

  /**
   * The card changes height as fields and messages come and go. Anchor its top
   * just under the wordmark, sliding it up only if it would run off the table.
   */
  positionCard() {
    const height = this.cardEl.offsetHeight;
    const centered = CARD_REGION_CENTER - height / 2;
    const lowest = GAME_HEIGHT - CARD_BOTTOM_MARGIN - height;

    this.card.setY(Math.max(CARD_TOP_MIN, Math.min(centered, lowest)));
  }

  async submit() {
    if (this.busy) return;

    const username = this.els.username.value.trim();
    const password = this.els.password.value;
    const registering = this.mode === "register";

    if (!username || !password) {
      this.showAlert("Enter a username and password.");
      return;
    }

    if (registering && password !== this.els.confirm.value) {
      this.showAlert("Those passwords do not match.");
      return;
    }

    this.setBusy(true);

    try {
      const payload = registering
        ? await api.register(username, password)
        : await api.login(username, password);

      const session = sessionFromAuth(payload);
      saveSession(session);
      this.enterLobby(session, registering);
    } catch (error) {
      this.showAlert(error.message);
      // A taken username is a registration problem; keep them on that tab with
      // the field focused so the fix is one keystroke away.
      if (error.code === "username_taken") {
        this.els.username.select();
      } else {
        this.els.password.select();
      }
      this.setBusy(false);
    }
  }

  enterLobby(session, isNewPlayer) {
    this.busy = true;
    this.tweens.add({ targets: this.card, alpha: 0, duration: 220 });
    this.cameras.main.fadeOut(280, 0, 0, 0);
    this.cameras.main.once("camerafadeoutcomplete", () => {
      this.scene.start("Lobby", { session, isNewPlayer });
    });
  }

  setBusy(busy) {
    this.busy = busy;
    this.els.submit.disabled = busy;
    this.els.submit.textContent = busy ? COPY[this.mode].busy : COPY[this.mode].submit;
    this.els.form.classList.toggle("is-busy", busy);
  }

  showAlert(message) {
    this.els.alert.textContent = message;
    this.els.alert.hidden = false;
    // A specific error is more useful than the general guidance, and showing
    // both makes the card too tall for the table.
    this.els.hint.hidden = true;
    this.positionCard();
  }

  hideAlert() {
    this.els.alert.hidden = true;
    this.els.hint.hidden = false;
    this.positionCard();
  }
}
