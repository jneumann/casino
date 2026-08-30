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

const PANEL = { width: 780, height: 420, x: GAME_WIDTH / 2, y: 338 };

export default class BankScene extends Phaser.Scene {
  constructor() {
    super("Bank");
  }

  init(data) {
    this.session = data.session;
    this.displayedBalance = this.session.balance ?? 0;
    this.loan = this.session.loan ?? null;
    this.offers = [];
    this.offerButtons = [];
    this.busy = false;
    this.alive = true;
  }

  create() {
    createTable(this);
    this.createTopBar();
    this.createPanel();
    this.createControls();

    this.events.once("shutdown", () => {
      this.alive = false;
    });

    this.cameras.main.fadeIn(320, 0, 0, 0);
    this.setBalance(this.session.balance ?? 0, { animate: false });
    this.loadBank();
  }

  createTopBar() {
    createButton(this, 148, 48, "Back to lobby", {
      width: 190,
      height: 42,
      variant: "ghost",
      fontSize: 15,
      onClick: () => this.leave(),
    });

    const title = this.add
      .text(GAME_WIDTH / 2, 48, "THE BANK", {
        fontFamily: FONT_DISPLAY,
        fontSize: "34px",
        color: css(COLORS.gold),
      })
      .setOrigin(0.5);

    if (typeof title.setLetterSpacing === "function") {
      title.setLetterSpacing(8);
    }

    this.add.image(GAME_WIDTH - 62, 48, "chip-gold").setScale(0.4);

    this.balanceText = this.add
      .text(GAME_WIDTH - 92, 48, "0", {
        fontFamily: FONT_NUMERIC,
        fontSize: "26px",
        fontStyle: "700",
        color: css(COLORS.cream),
      })
      .setOrigin(1, 0.5);

    const rule = this.add.graphics();
    rule.lineStyle(1, COLORS.goldDeep, 0.28);
    rule.lineBetween(56, 84, GAME_WIDTH - 56, 84);
  }

  createPanel() {
    this.add.image(PANEL.x, PANEL.y, "glow").setScale(3.8, 2.2).setAlpha(0.38);

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

    this.add
      .text(PANEL.x, PANEL.y - 176, "H O U S E   M A R K E R S", {
        fontFamily: FONT_BODY,
        fontSize: "14px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.termsText = this.add
      .text(
        PANEL.x,
        PANEL.y - 132,
        "The cashier will write one marker at a time. Interest is charged when you borrow.",
        {
          fontFamily: FONT_BODY,
          fontSize: "16px",
          color: css(COLORS.cream),
          align: "center",
          wordWrap: { width: PANEL.width - 80 },
        },
      )
      .setOrigin(0.5);

    this.statusText = this.add
      .text(PANEL.x, PANEL.y + 168, "Checking the books…", {
        fontFamily: FONT_BODY,
        fontSize: "15px",
        color: css(COLORS.muted),
        align: "center",
        wordWrap: { width: PANEL.width - 80 },
      })
      .setOrigin(0.5);
  }

  createControls() {
    this.repayButton = createButton(this, PANEL.x, PANEL.y + 86, "Repay the house", {
      width: 280,
      height: 54,
      fontSize: 18,
      onClick: () => this.repay(),
    });
    this.repayButton.setVisible(false);
  }

  async loadBank() {
    try {
      const bank = await api.bank(this.session.token);
      if (!this.alive) return;

      this.offers = bank.offers ?? [];
      this.applyWallet(bank.balance, bank.loan);
      this.renderOffers();
      this.refreshState();
    } catch (error) {
      if (!this.alive) return;
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }
      this.showStatus(error.message, COLORS.red);
    }
  }

  renderOffers() {
    this.offerButtons.forEach((button) => {
      button.caption?.destroy();
      button.destroy();
    });
    this.offerButtons = [];

    const count = this.offers.length;
    if (count === 0) return;

    const width = 156;
    const gap = 18;
    const total = count * width + (count - 1) * gap;
    const left = PANEL.x - total / 2 + width / 2;

    this.offers.forEach((offer, index) => {
      const x = left + index * (width + gap);
      const button = createButton(
        this,
        x,
        PANEL.y - 18,
        `${offer.principal.toLocaleString()}`,
        {
          width,
          height: 86,
          fontSize: 22,
          onClick: () => this.borrow(offer.principal),
        },
      );

      const caption = this.add
        .text(x, PANEL.y + 44, `repay ${offer.repay.toLocaleString()}`, {
          fontFamily: FONT_BODY,
          fontSize: "13px",
          color: css(COLORS.muted),
        })
        .setOrigin(0.5);

      button.caption = caption;
      this.offerButtons.push(button);
    });
  }

  refreshState() {
    const hasLoan = Boolean(this.loan);
    const canAfford = this.loan ? this.session.balance >= this.loan.owed : false;

    this.offerButtons.forEach((button) => {
      button.setVisible(!hasLoan);
      button.setEnabled(!hasLoan && !this.busy);
      if (button.caption) button.caption.setVisible(!hasLoan);
    });

    this.repayButton.setVisible(hasLoan);
    this.repayButton.setEnabled(hasLoan && canAfford && !this.busy);
    this.repayButton.setLabel(
      this.loan ? `Repay ${this.loan.owed.toLocaleString()}` : "Repay the house",
    );

    if (hasLoan) {
      const rate = Math.round((this.loan.interest / this.loan.principal) * 100);
      this.termsText.setText(
        `You borrowed ${this.loan.principal.toLocaleString()} and owe ${this.loan.owed.toLocaleString()} (${rate}% interest). Clear the marker before taking another.`,
      );
      this.showStatus(
        canAfford
          ? "The cashier will take the full amount in one payment."
          : `You need ${this.loan.owed.toLocaleString()} coins to clear this marker.`,
        canAfford ? COLORS.muted : COLORS.gold,
      );
      return;
    }

    const sample = this.offers.find((offer) => offer.principal === 500) ?? this.offers[0];
    const rate = sample
      ? Math.round((sample.interest / sample.principal) * 100)
      : 20;
    const example = sample
      ? `take ${sample.principal.toLocaleString()}, walk out with ${sample.principal.toLocaleString()}, and owe ${sample.repay.toLocaleString()}`
      : "the house takes a cut up front";
    this.termsText.setText(
      `One marker at a time. Interest is ${rate}%, charged when you borrow — ${example}.`,
    );
    this.showStatus("Choose a marker. The house always collects.", COLORS.muted);
  }

  async borrow(amount) {
    if (this.busy || this.loan) return;

    this.busy = true;
    this.refreshState();
    this.showStatus("The cashier is writing the marker…", COLORS.muted);

    try {
      const result = await api.borrow(this.session.token, amount);
      if (!this.alive) return;

      this.applyWallet(result.balance, result.loan);
      this.refreshState();
      this.showStatus(
        `Marker written for ${result.loan.principal.toLocaleString()} coins. You owe ${result.loan.owed.toLocaleString()}.`,
        COLORS.success,
      );
    } catch (error) {
      if (!this.alive) return;
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }
      this.showStatus(error.message, COLORS.red);
    } finally {
      this.busy = false;
      if (this.alive) this.refreshState();
    }
  }

  async repay() {
    if (this.busy || !this.loan) return;

    this.busy = true;
    this.refreshState();
    this.showStatus("Settling the marker…", COLORS.muted);

    try {
      const result = await api.repay(this.session.token);
      if (!this.alive) return;

      this.applyWallet(result.balance, result.loan);
      this.refreshState();
      this.showStatus("The books are clear. The cashier will write another when you ask.", COLORS.success);
    } catch (error) {
      if (!this.alive) return;
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }
      this.showStatus(error.message, COLORS.red);
    } finally {
      this.busy = false;
      if (this.alive) this.refreshState();
    }
  }

  applyWallet(balance, loan) {
    this.loan = loan ?? null;
    this.session = { ...this.session, balance, loan: this.loan };
    saveSession(this.session);
    this.setBalance(balance);
  }

  setBalance(value, { animate = true } = {}) {
    if (!animate) {
      this.displayedBalance = value;
      this.balanceText.setText(value.toLocaleString());
      return;
    }

    this.tweens.addCounter({
      from: this.displayedBalance,
      to: value,
      duration: 700,
      ease: "Cubic.easeOut",
      onUpdate: (tween) => {
        this.balanceText.setText(Math.round(tween.getValue()).toLocaleString());
      },
      onComplete: () => this.balanceText.setText(value.toLocaleString()),
    });

    this.displayedBalance = value;
  }

  showStatus(message, color) {
    this.statusText.setText(message).setColor(css(color));
  }

  leave() {
    if (this.busy) return;

    this.alive = false;
    this.cameras.main.fadeOut(260, 0, 0, 0);
    this.cameras.main.once("camerafadeoutcomplete", () => {
      this.scene.start("Lobby", { session: this.session });
    });
  }

  signOut() {
    clearSession();
    this.alive = false;
    this.scene.start("Auth", { notice: "Your session expired. Please sign in again." });
  }
}
