import Phaser from "phaser";

import { api } from "../api.js";
import { clearSession, saveSession } from "../session.js";
import {
  COLORS,
  FONT_BODY,
  FONT_DISPLAY,
  FONT_NUMERIC,
  GAME_WIDTH,
  css,
} from "../theme.js";
import { createButton } from "../ui/button.js";
import { createPlayingCard } from "../ui/card.js";
import { createTable } from "../ui/table.js";

const CHIP_VALUES = [5, 10, 25, 50, 100];
const HAND_SIZE = 5;
const CARD_WIDTH = 118;
const CARD_HEIGHT = 166;
const CARD_GAP = 16;
const CARDS_CENTER_X = 800;
const CARDS_CENTER_Y = 292;

export default class PokerScene extends Phaser.Scene {
  constructor() {
    super("Poker");
  }

  init(data) {
    this.session = data.session;
    this.displayedBalance = this.session.balance ?? 0;
    this.balance = this.displayedBalance;
    this.bet = CHIP_VALUES[0];
    this.phase = "idle";
    this.busy = false;
    this.alive = true;
    this.paytableHands = [];
  }

  create() {
    createTable(this);
    this.createTopBar();
    this.createMachine();
    this.createControls();
    this.createPaytablePanel();

    this.events.once("shutdown", () => {
      this.alive = false;
    });

    this.input.keyboard.on("keydown-SPACE", () => this.act());
    this.input.keyboard.on("keydown-ONE", () => this.toggleHold(0));
    this.input.keyboard.on("keydown-TWO", () => this.toggleHold(1));
    this.input.keyboard.on("keydown-THREE", () => this.toggleHold(2));
    this.input.keyboard.on("keydown-FOUR", () => this.toggleHold(3));
    this.input.keyboard.on("keydown-FIVE", () => this.toggleHold(4));

    this.cameras.main.fadeIn(320, 0, 0, 0);

    this.setBalance(this.session.balance ?? 0, { animate: false });
    this.loadPaytable();
    this.loadOpenHand();
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
      .text(GAME_WIDTH / 2, 48, "VIDEO POKER", {
        fontFamily: FONT_DISPLAY,
        fontSize: "34px",
        color: css(COLORS.gold),
      })
      .setOrigin(0.5);

    if (typeof title.setLetterSpacing === "function") {
      title.setLetterSpacing(6);
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

  createMachine() {
    const totalWidth = HAND_SIZE * CARD_WIDTH + (HAND_SIZE - 1) * CARD_GAP;
    const left = CARDS_CENTER_X - totalWidth / 2;

    this.add
      .image(CARDS_CENTER_X, CARDS_CENTER_Y, "glow")
      .setScale(3.4, 1.7)
      .setAlpha(0.32);

    const rail = this.add.graphics();
    rail.fillStyle(0x04150f, 0.78);
    rail.fillRoundedRect(left - 22, CARDS_CENTER_Y - CARD_HEIGHT / 2 - 22, totalWidth + 44, CARD_HEIGHT + 64, 18);
    rail.lineStyle(2, COLORS.goldDeep, 0.8);
    rail.strokeRoundedRect(left - 22, CARDS_CENTER_Y - CARD_HEIGHT / 2 - 22, totalWidth + 44, CARD_HEIGHT + 64, 18);

    this.cards = [];
    for (let i = 0; i < HAND_SIZE; i += 1) {
      const x = left + CARD_WIDTH / 2 + i * (CARD_WIDTH + CARD_GAP);
      this.cards.push(
        createPlayingCard(this, {
          x,
          y: CARDS_CENTER_Y,
          width: CARD_WIDTH,
          height: CARD_HEIGHT,
          onClick: () => this.toggleHold(i),
        }),
      );
    }

    this.resultText = this.add
      .text(CARDS_CENTER_X, 430, "Choose your stake and deal a hand.", {
        fontFamily: FONT_BODY,
        fontSize: "19px",
        color: css(COLORS.muted),
        align: "center",
        wordWrap: { width: 560 },
      })
      .setOrigin(0.5);

    this.netText = this.add
      .text(CARDS_CENTER_X, 462, "", {
        fontFamily: FONT_NUMERIC,
        fontSize: "23px",
        fontStyle: "700",
        color: css(COLORS.success),
      })
      .setOrigin(0.5);
  }

  createControls() {
    this.add
      .text(CARDS_CENTER_X, 506, "S T A K E", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    const spacing = 92;
    const start = CARDS_CENTER_X - ((CHIP_VALUES.length - 1) * spacing) / 2;

    this.chips = CHIP_VALUES.map((value, index) =>
      this.createChip(start + index * spacing, 560, value),
    );

    this.actButton = createButton(this, CARDS_CENTER_X, 646, "DEAL", {
      width: 260,
      height: 62,
      fontSize: 21,
      onClick: () => this.act(),
    });

    this.refreshChips();
  }

  createChip(x, y, value) {
    const image = this.add.image(0, 0, "chip-navy").setScale(0.66);
    const label = this.add
      .text(0, 0, String(value), {
        fontFamily: FONT_BODY,
        fontSize: "19px",
        fontStyle: "700",
        color: css(COLORS.cream),
      })
      .setOrigin(0.5);

    const ring = this.add.graphics();
    const container = this.add.container(x, y, [ring, image, label]);
    const radius = 36;

    container.setSize(radius * 2, radius * 2);
    container
      .setInteractive({
        hitArea: new Phaser.Geom.Circle(radius, radius, radius),
        hitAreaCallback: Phaser.Geom.Circle.Contains,
        useHandCursor: true,
      })
      .on("pointerup", () => this.selectBet(value));

    return { value, container, image, label, ring };
  }

  selectBet(value) {
    if (this.busy || this.phase === "dealt" || value > this.balance) return;

    this.bet = value;
    this.refreshChips();
  }

  refreshChips() {
    const locked = this.busy || this.phase === "dealt";

    this.chips.forEach((chip) => {
      const affordable = chip.value <= this.balance;
      const selected = chip.value === this.bet;

      chip.image.setTexture(selected ? "chip-gold" : "chip-navy");
      chip.label.setColor(css(selected ? 0x1c1405 : COLORS.cream));
      chip.container.setAlpha(!affordable || (locked && !selected) ? 0.32 : 1);

      chip.ring.clear();
      if (selected) {
        chip.ring.lineStyle(2.5, COLORS.gold, 0.9);
        chip.ring.strokeCircle(0, 0, 38);
      }
    });

    if (this.phase !== "dealt" && this.bet > this.balance) {
      const affordable = CHIP_VALUES.filter((value) => value <= this.balance);
      if (affordable.length > 0) {
        this.bet = affordable[affordable.length - 1];
        this.refreshChips();
      }
    }
  }

  createPaytablePanel() {
    const x = 196;
    const width = 280;
    const top = 118;
    const height = 470;

    const panel = this.add.graphics();
    panel.fillStyle(COLORS.panel, 0.72);
    panel.fillRoundedRect(x - width / 2, top, width, height, 16);
    panel.lineStyle(1.2, COLORS.goldDeep, 0.5);
    panel.strokeRoundedRect(x - width / 2, top, width, height, 16);

    this.add
      .text(x, top + 28, "J A C K S   O R   B E T T E R", {
        fontFamily: FONT_BODY,
        fontSize: "12px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.paytableAnchor = { x, top, width };
    this.paytableRows = this.add.container(0, 0);
    this.paytableLabels = [];

    this.rtpText = this.add
      .text(x, top + height - 26, "", {
        fontFamily: FONT_BODY,
        fontSize: "12px",
        color: css(COLORS.muted),
        align: "center",
        wordWrap: { width: width - 32 },
      })
      .setOrigin(0.5);
  }

  async loadPaytable() {
    let table;
    try {
      table = await api.videoPokerPaytable();
    } catch {
      if (this.alive) this.rtpText.setText("Paytable unavailable.");
      return;
    }

    if (!this.alive) return;

    this.paytableHands = table.hands;
    const { x, top, width } = this.paytableAnchor;

    table.hands.forEach((entry, index) => {
      const y = top + 64 + index * 38;

      const name = this.add
        .text(x - width / 2 + 18, y, entry.name, {
          fontFamily: FONT_BODY,
          fontSize: "15px",
          color: css(COLORS.cream),
        })
        .setOrigin(0, 0.5);

      const multiplier = this.add
        .text(x + width / 2 - 18, y, `${entry.multiplier}x`, {
          fontFamily: FONT_NUMERIC,
          fontSize: "16px",
          fontStyle: "700",
          color: css(COLORS.gold),
        })
        .setOrigin(1, 0.5);

      this.paytableRows.add(name);
      this.paytableRows.add(multiplier);
      this.paytableLabels.push({ kind: entry.kind, name, multiplier });
    });

    const rtp = Math.round(table.return_to_player * 1000) / 10;
    this.rtpText.setText(
      `Stakes from ${table.min_bet} to ${table.max_bet} coins.\nReturns ${rtp}% with optimal play.`,
    );
  }

  highlightPaytable(kind) {
    this.paytableLabels.forEach((row) => {
      const active = row.kind === kind;
      row.name.setColor(css(active ? COLORS.gold : COLORS.cream));
      row.multiplier.setColor(css(active ? COLORS.gold : COLORS.gold));
      row.name.setAlpha(kind && !active ? 0.45 : 1);
      row.multiplier.setAlpha(kind && !active ? 0.45 : 1);
    });
  }

  async loadOpenHand() {
    let payload;
    try {
      payload = await api.videoPokerHand(this.session.token);
    } catch (error) {
      if (error.isUnauthorized) {
        this.signOut();
      }
      return;
    }

    if (!this.alive) return;

    this.setBalance(payload.balance, { animate: false });

    if (!payload.hand) return;

    this.bet = payload.hand.bet;
    this.phase = "dealt";
    payload.hand.cards.forEach((card, index) => {
      this.cards[index].setCard(card).setHeld(false).setEnabled(true);
    });
    this.showResult("Hold any cards you want to keep, then draw.", COLORS.muted);
    this.refreshAction();
    this.refreshChips();
  }

  act() {
    if (this.phase === "dealt") {
      this.draw();
    } else {
      this.deal();
    }
  }

  toggleHold(index) {
    if (this.busy || this.phase !== "dealt" || !this.alive) return;
    this.cards[index].toggleHold();
  }

  async deal() {
    if (this.busy || this.phase === "dealt" || !this.alive) return;

    if (this.bet > this.balance) {
      this.showResult("Not enough coins for that stake.", COLORS.red);
      return;
    }

    this.busy = true;
    this.netText.setText("");
    this.highlightPaytable(null);
    this.cards.forEach((card) => card.setEnabled(false).setHeld(false));
    this.refreshAction();
    this.refreshChips();
    this.showResult("Dealing…", COLORS.muted);

    const hide = Promise.all(
      this.cards.map((card) => card.flipTo(null, { duration: 180 })),
    );

    let result;
    try {
      result = await api.videoPokerDeal(this.session.token, this.bet);
    } catch (error) {
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }

      if (error.code === "hand_in_progress") {
        this.busy = false;
        await this.loadOpenHand();
        return;
      }

      this.showResult(error.message, COLORS.red);
      this.busy = false;
      this.refreshAction();
      this.refreshChips();
      return;
    }

    if (!this.alive) return;

    this.setBalance(result.balance);
    await hide;

    await Promise.all(
      this.cards.map((card, index) =>
        card.flipTo(result.cards[index], { duration: 240 + index * 70 }),
      ),
    );

    if (!this.alive) return;

    this.phase = "dealt";
    this.busy = false;
    this.cards.forEach((card) => card.setEnabled(true));
    this.showResult("Hold any cards you want to keep, then draw.", COLORS.cream);
    this.refreshAction();
    this.refreshChips();
  }

  async draw() {
    if (this.busy || this.phase !== "dealt" || !this.alive) return;

    this.busy = true;
    this.cards.forEach((card) => card.setEnabled(false));
    this.refreshAction();
    this.refreshChips();
    this.showResult("Drawing…", COLORS.muted);

    const held = this.cards.map((card) => card.held);

    let result;
    try {
      result = await api.videoPokerDraw(this.session.token, held);
    } catch (error) {
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }

      this.showResult(error.message, COLORS.red);
      this.busy = false;
      this.cards.forEach((card) => card.setEnabled(true));
      this.refreshAction();
      this.refreshChips();
      return;
    }

    if (!this.alive) return;

    await Promise.all(
      this.cards.map((card, index) => {
        if (held[index]) return Promise.resolve();
        return card.flipTo(result.cards[index], { duration: 280 });
      }),
    );

    if (!this.alive) return;

    this.settle(result);
  }

  settle(result) {
    this.phase = "idle";
    this.busy = false;
    this.setBalance(result.balance);
    this.highlightPaytable(result.outcome.kind);

    if (result.outcome.kind === "nothing") {
      this.showResult("Nothing. Deal again.", COLORS.muted);
      this.netText.setText(`-${result.bet.toLocaleString()}`).setColor(css(COLORS.red));
    } else {
      const name = this.handName(result.outcome.kind);
      this.showResult(
        `${name} pays ${result.payout.toLocaleString()} coins.`,
        result.net >= 0 ? COLORS.gold : COLORS.cream,
      );
      this.cards.forEach((card) => card.celebrate());

      const sign = result.net >= 0 ? "+" : "-";
      this.netText
        .setText(`${sign}${Math.abs(result.net).toLocaleString()}`)
        .setColor(css(result.net >= 0 ? COLORS.success : COLORS.red));

      if (
        result.outcome.kind === "royal_flush" ||
        result.outcome.kind === "straight_flush" ||
        result.outcome.kind === "four_of_a_kind"
      ) {
        this.cameras.main.flash(220, 232, 196, 106);
      }
    }

    this.cards.forEach((card) => card.setEnabled(false));
    this.refreshAction();
    this.refreshChips();

    if (this.balance < CHIP_VALUES[0]) {
      this.showResult("You are out of coins. The bank will write a marker.", COLORS.red);
      this.actButton.setEnabled(false);
    }
  }

  handName(kind) {
    const entry = this.paytableHands.find((hand) => hand.kind === kind);
    if (entry) return entry.name;
    return kind.replaceAll("_", " ");
  }

  refreshAction() {
    if (this.busy) {
      this.actButton
        .setEnabled(false)
        .setLabel(this.phase === "dealt" ? "Drawing…" : "Dealing…");
      return;
    }

    if (this.phase === "dealt") {
      this.actButton.setEnabled(true).setLabel("DRAW");
      return;
    }

    this.actButton.setEnabled(this.balance >= CHIP_VALUES[0]).setLabel("DEAL");
  }

  showResult(message, color) {
    this.resultText.setText(message).setColor(css(color));
  }

  setBalance(value, { animate = true } = {}) {
    this.balance = value;
    this.session = { ...this.session, balance: value };
    saveSession(this.session);

    if (!animate) {
      this.displayedBalance = value;
      this.balanceText.setText(value.toLocaleString());
      this.refreshChips();
      return;
    }

    this.tweens.addCounter({
      from: this.displayedBalance,
      to: value,
      duration: 600,
      ease: "Cubic.easeOut",
      onUpdate: (tween) => {
        this.balanceText.setText(Math.round(tween.getValue()).toLocaleString());
      },
      onComplete: () => this.balanceText.setText(value.toLocaleString()),
    });

    this.displayedBalance = value;
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
