import Phaser from "phaser";

import { api } from "../api.js";
import { clearSession, saveSession } from "../session.js";
import {
  COLORS,
  FONT_BODY,
  FONT_DISPLAY,
  FONT_NUMERIC,
  GAME_HEIGHT,
  GAME_WIDTH,
  css,
} from "../theme.js";
import { createButton } from "../ui/button.js";
import { createTable } from "../ui/table.js";

const REFRESH_MS = 30_000;
const PANEL = { width: 620, height: 286, x: GAME_WIDTH / 2, y: 338 };

export default class LobbyScene extends Phaser.Scene {
  constructor() {
    super("Lobby");
  }

  init(data) {
    this.session = data.session;
    this.isNewPlayer = Boolean(data.isNewPlayer);
    this.initialError = data.error ?? null;
    this.displayedBalance = 0;
    this.alive = true;
  }

  create() {
    createTable(this);
    this.createTopBar();
    this.createBankPanel();
    this.createControls();
    this.createFooter();

    this.events.once("shutdown", () => {
      this.alive = false;
    });

    this.cameras.main.fadeIn(350, 0, 0, 0);

    // Show the balance we already have, then confirm it against the server.
    if (typeof this.session.balance === "number") {
      this.animateBalanceTo(this.session.balance);
    }
    this.showLoan(this.session.loan);

    if (this.initialError) {
      this.showError(this.initialError);
    }

    if (this.isNewPlayer) {
      this.showToast("Welcome to the table. Your opening stake is on the house.");
    }

    this.refresh();
    this.time.addEvent({
      delay: REFRESH_MS,
      loop: true,
      callback: () => this.refresh(),
    });
  }

  createTopBar() {
    this.add
      .text(56, 46, "CASINO ROYALE", {
        fontFamily: FONT_DISPLAY,
        fontSize: "26px",
        color: css(COLORS.gold),
      })
      .setOrigin(0, 0.5);

    this.add
      .image(GAME_WIDTH - 66, 46, "chip-gold")
      .setScale(0.42)
      .setAlpha(0.95);

    this.add
      .text(GAME_WIDTH - 96, 38, this.session.username, {
        fontFamily: FONT_BODY,
        fontSize: "17px",
        fontStyle: "600",
        color: css(COLORS.cream),
      })
      .setOrigin(1, 0.5);

    this.add
      .text(GAME_WIDTH - 96, 58, "signed in", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        color: css(COLORS.muted),
      })
      .setOrigin(1, 0.5);

    const rule = this.add.graphics();
    rule.lineStyle(1, COLORS.goldDeep, 0.28);
    rule.lineBetween(56, 84, GAME_WIDTH - 56, 84);
  }

  createBankPanel() {
    this.add
      .image(PANEL.x, PANEL.y, "glow")
      .setScale(3.4, 1.9)
      .setAlpha(0.42);

    const panel = this.add.graphics();
    panel.fillStyle(COLORS.panel, 0.82);
    panel.fillRoundedRect(
      PANEL.x - PANEL.width / 2,
      PANEL.y - PANEL.height / 2,
      PANEL.width,
      PANEL.height,
      22,
    );
    panel.lineStyle(1.5, COLORS.goldDeep, 0.75);
    panel.strokeRoundedRect(
      PANEL.x - PANEL.width / 2,
      PANEL.y - PANEL.height / 2,
      PANEL.width,
      PANEL.height,
      22,
    );

    const label = this.add
      .text(PANEL.x, PANEL.y - 82, "Y O U R   B A N K", {
        fontFamily: FONT_BODY,
        fontSize: "14px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);
    label.setAlpha(0.9);

    this.balanceText = this.add
      .text(PANEL.x, PANEL.y - 8, "0", {
        fontFamily: FONT_NUMERIC,
        fontSize: "76px",
        fontStyle: "700",
        color: css(COLORS.cream),
      })
      .setOrigin(0, 0.5)
      .setShadow(0, 4, "rgba(0, 0, 0, 0.55)", 10);

    this.coinIcon = this.add
      .image(PANEL.x, PANEL.y - 8, "chip-gold")
      .setScale(0.5)
      .setOrigin(0.5);

    this.layoutBalance();

    this.add
      .text(PANEL.x, PANEL.y + 52, "coins", {
        fontFamily: FONT_BODY,
        fontSize: "18px",
        color: css(COLORS.gold),
      })
      .setOrigin(0.5);

    this.loanText = this.add
      .text(PANEL.x, PANEL.y + 82, "", {
        fontFamily: FONT_BODY,
        fontSize: "15px",
        color: css(COLORS.gold),
      })
      .setOrigin(0.5)
      .setVisible(false);

    this.updatedText = this.add
      .text(PANEL.x, PANEL.y + 114, "", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.errorText = this.add
      .text(PANEL.x, PANEL.y + 168, "", {
        fontFamily: FONT_BODY,
        fontSize: "15px",
        color: css(COLORS.red),
        align: "center",
        wordWrap: { width: PANEL.width },
      })
      .setOrigin(0.5)
      .setVisible(false);

    this.createChipStack(PANEL.x - PANEL.width / 2 - 96, PANEL.y + 44);
    this.createChipStack(PANEL.x + PANEL.width / 2 + 96, PANEL.y + 44);
  }

  /**
   * Centres the coin and the digits as one group. The digits change width as
   * the balance grows, so this runs on every update rather than once.
   */
  layoutBalance() {
    const gap = 18;
    const coinWidth = this.coinIcon.displayWidth;
    const total = coinWidth + gap + this.balanceText.width;
    const left = PANEL.x - total / 2;

    this.coinIcon.setX(left + coinWidth / 2);
    this.balanceText.setX(left + coinWidth + gap);
  }

  /** Decorative pile of chips flanking the bank panel. */
  createChipStack(x, y) {
    const keys = ["chip-navy", "chip-red", "chip-green", "chip-gold", "chip-red"];
    const stack = this.add.container(x, y);

    keys.forEach((key, index) => {
      const chip = this.add
        .image(0, -index * 13, key)
        .setScale(0.86)
        .setAlpha(0.9);
      stack.add(chip);
    });

    this.tweens.add({
      targets: stack,
      y: y - 6,
      duration: 2600,
      ease: "Sine.easeInOut",
      yoyo: true,
      repeat: -1,
    });
  }

  createControls() {
    this.playButton = createButton(this, GAME_WIDTH / 2 - 148, 548, "Play slots", {
      width: 260,
      height: 58,
      fontSize: 19,
      onClick: () => this.playSlots(),
    });

    this.bankButton = createButton(this, GAME_WIDTH / 2 + 148, 548, "Visit the bank", {
      width: 260,
      height: 58,
      fontSize: 19,
      variant: "ghost",
      onClick: () => this.visitBank(),
    });

    this.refreshButton = createButton(this, GAME_WIDTH / 2 - 120, 622, "Refresh balance", {
      width: 218,
      height: 46,
      fontSize: 15,
      onClick: () => this.refresh(),
    });

    createButton(this, GAME_WIDTH / 2 + 120, 622, "Sign out", {
      width: 218,
      height: 46,
      fontSize: 15,
      variant: "ghost",
      onClick: () => this.signOut(),
    });
  }

  createFooter() {
    this.add
      .text(
        GAME_WIDTH / 2,
        GAME_HEIGHT - 26,
        "Blackjack and roulette are still being dealt in.",
        {
          fontFamily: FONT_BODY,
          fontSize: "13px",
          color: css(COLORS.muted),
        },
      )
      .setOrigin(0.5)
      .setAlpha(0.7);
  }

  playSlots() {
    this.goTo("Slots");
  }

  visitBank() {
    this.goTo("Bank");
  }

  goTo(scene) {
    this.alive = false;
    this.cameras.main.fadeOut(260, 0, 0, 0);
    this.cameras.main.once("camerafadeoutcomplete", () => {
      this.scene.start(scene, { session: this.session });
    });
  }

  async refresh() {
    if (!this.alive) return;

    this.refreshButton.setEnabled(false).setLabel("Checking…");

    try {
      const wallet = await api.balance(this.session.token);
      if (!this.alive) return;

      this.session = {
        ...this.session,
        balance: wallet.balance,
        username: wallet.username,
        loan: wallet.loan ?? null,
      };
      saveSession(this.session);

      this.animateBalanceTo(wallet.balance);
      this.showLoan(wallet.loan);
      this.updatedText.setText(
        `Updated ${new Date(wallet.as_of).toLocaleTimeString()} · refreshes automatically`,
      );
      this.hideError();
    } catch (error) {
      if (!this.alive) return;

      if (error.isUnauthorized) {
        this.signOut("Your session expired. Please sign in again.");
        return;
      }

      this.showError(error.message);
    } finally {
      if (this.alive) {
        this.refreshButton.setEnabled(true).setLabel("Refresh balance");
      }
    }
  }

  /** Counts the number up rather than snapping, so a change is noticeable. */
  animateBalanceTo(target) {
    const from = this.displayedBalance;
    if (from === target) {
      this.balanceText.setText(target.toLocaleString());
      this.layoutBalance();
      return;
    }

    this.tweens.addCounter({
      from,
      to: target,
      duration: 850,
      ease: "Cubic.easeOut",
      onUpdate: (tween) => {
        this.balanceText.setText(Math.round(tween.getValue()).toLocaleString());
        this.layoutBalance();
      },
      onComplete: () => {
        this.balanceText.setText(target.toLocaleString());
        this.layoutBalance();
      },
    });

    this.showLoan(this.session.loan);

    this.displayedBalance = target;

    this.tweens.add({
      targets: this.coinIcon,
      angle: this.coinIcon.angle + 360,
      duration: 850,
      ease: "Cubic.easeOut",
    });
  }

  showLoan(loan) {
    if (!this.loanText) return;

    if (loan?.owed) {
      this.loanText
        .setText(`You owe the house ${loan.owed.toLocaleString()} coins`)
        .setVisible(true);
      return;
    }

    this.loanText.setVisible(false);
  }

  showToast(message) {
    const toast = this.add
      .text(GAME_WIDTH / 2, 150, message, {
        fontFamily: FONT_BODY,
        fontSize: "16px",
        color: css(COLORS.success),
      })
      .setOrigin(0.5)
      .setAlpha(0);

    this.tweens.add({
      targets: toast,
      alpha: 1,
      y: 142,
      duration: 500,
      ease: "Cubic.easeOut",
      hold: 4200,
      yoyo: true,
      onComplete: () => toast.destroy(),
    });
  }

  showError(message) {
    this.errorText.setText(message).setVisible(true);
  }

  hideError() {
    this.errorText.setVisible(false);
  }

  signOut(notice = null) {
    clearSession();
    this.alive = false;
    this.cameras.main.fadeOut(280, 0, 0, 0);
    this.cameras.main.once("camerafadeoutcomplete", () => {
      this.scene.start("Auth", { notice });
    });
  }
}
