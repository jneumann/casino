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
const MAX_CARDS = 7;
const CARD_WIDTH = 92;
const CARD_HEIGHT = 128;
const CARD_GAP = 12;
const CARDS_CENTER_X = 800;
const DEALER_Y = 188;
const PLAYER_Y = 368;
const SPLIT_OFFSET = 205;
const SPLIT_SCALE = 0.74;

const OUTCOME_LABEL = {
  player_blackjack: "Blackjack",
  player_win: "You win",
  push: "Push",
  dealer_win: "Dealer wins",
  player_bust: "Bust",
  dealer_blackjack: "Dealer blackjack",
};

export default class BlackjackScene extends Phaser.Scene {
  constructor() {
    super("Blackjack");
  }

  init(data) {
    this.session = data.session;
    this.displayedBalance = this.session.balance ?? 0;
    this.balance = this.displayedBalance;
    this.bet = CHIP_VALUES[0];
    this.phase = "idle";
    this.busy = false;
    this.alive = true;
    this.canHit = false;
    this.canStand = false;
    this.canDouble = false;
    this.canSplit = false;
  }

  create() {
    createTable(this);
    this.createTopBar();
    this.createLayout();
    this.createControls();
    this.createRulesPanel();

    this.events.once("shutdown", () => {
      this.alive = false;
    });

    this.input.keyboard.on("keydown-SPACE", () => this.onSpace());
    this.input.keyboard.on("keydown-H", () => this.act("hit"));
    this.input.keyboard.on("keydown-S", () => this.act("stand"));
    this.input.keyboard.on("keydown-D", () => this.act("double"));
    this.input.keyboard.on("keydown-P", () => this.act("split"));

    this.cameras.main.fadeIn(320, 0, 0, 0);

    this.setBalance(this.session.balance ?? 0, { animate: false });
    this.loadRules();
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
      .text(GAME_WIDTH / 2, 48, "BLACKJACK", {
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

  createLayout() {
    this.add
      .image(CARDS_CENTER_X, 280, "glow")
      .setScale(3.2, 2.1)
      .setAlpha(0.28);

    this.dealerLabel = this.add
      .text(0, 108, "DEALER", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0, 0.5);

    this.dealerTotal = this.add
      .text(0, 108, "", {
        fontFamily: FONT_NUMERIC,
        fontSize: "18px",
        fontStyle: "700",
        color: css(COLORS.cream),
      })
      .setOrigin(0, 0.5);

    this.layoutHeading(this.dealerLabel, this.dealerTotal);
    this.dealerCards = this.createCardRow(CARDS_CENTER_X, DEALER_Y);

    this.handUi = [0, 1].map((index) => {
      const label = this.add
        .text(0, 278, index === 0 ? "YOU" : "HAND 2", {
          fontFamily: FONT_BODY,
          fontSize: "13px",
          fontStyle: "600",
          color: css(COLORS.muted),
        })
        .setOrigin(0, 0.5);

      const total = this.add
        .text(0, 278, "", {
          fontFamily: FONT_NUMERIC,
          fontSize: "18px",
          fontStyle: "700",
          color: css(COLORS.cream),
        })
        .setOrigin(0, 0.5);

      this.layoutHeading(label, total);
      return { label, total };
    });

    this.handUi[1].label.setVisible(false);
    this.handUi[1].total.setVisible(false);
    this.playerLabel = this.handUi[0].label;
    this.playerTotal = this.handUi[0].total;
    this.playerRows = [
      this.createCardRow(CARDS_CENTER_X, PLAYER_Y),
      this.createCardRow(CARDS_CENTER_X + SPLIT_OFFSET, PLAYER_Y),
    ];
    this.playerCards = this.playerRows[0];
    this.playerRows[1].forEach((card) => card.container.setVisible(false));

    this.resultText = this.add
      .text(CARDS_CENTER_X, 458, "Choose your stake and deal a hand.", {
        fontFamily: FONT_BODY,
        fontSize: "18px",
        color: css(COLORS.muted),
        align: "center",
        wordWrap: { width: 560 },
      })
      .setOrigin(0.5);

    this.netText = this.add
      .text(CARDS_CENTER_X, 486, "", {
        fontFamily: FONT_NUMERIC,
        fontSize: "22px",
        fontStyle: "700",
        color: css(COLORS.success),
      })
      .setOrigin(0.5);
  }

  /**
   * Centres a hand title and its total as one group so the score sits
   * beside the label, above the cards, instead of under them.
   */
  layoutHeading(label, total, centerX = CARDS_CENTER_X) {
    const gap = total.text ? 10 : 0;
    const width = label.width + gap + (total.text ? total.width : 0);
    const left = centerX - width / 2;

    label.setX(left);
    total.setX(left + label.width + gap);
  }

  createCardRow(centerX, y) {
    const totalWidth = MAX_CARDS * CARD_WIDTH + (MAX_CARDS - 1) * CARD_GAP;
    const left = centerX - totalWidth / 2;
    const cards = [];

    for (let i = 0; i < MAX_CARDS; i += 1) {
      const x = left + CARD_WIDTH / 2 + i * (CARD_WIDTH + CARD_GAP);
      const card = createPlayingCard(this, {
        x,
        y,
        width: CARD_WIDTH,
        height: CARD_HEIGHT,
      });
      card.container.setVisible(false);
      cards.push(card);
    }

    return cards;
  }

  layoutCardRow(slots, centerX, y, scale = 1) {
    const width = CARD_WIDTH * scale;
    const gap = CARD_GAP * scale;
    const totalWidth = MAX_CARDS * width + (MAX_CARDS - 1) * gap;
    const left = centerX - totalWidth / 2;

    slots.forEach((slot, index) => {
      slot.container.setPosition(left + width / 2 + index * (width + gap), y);
      slot.container.setScale(scale);
    });
  }

  createControls() {
    this.add
      .text(CARDS_CENTER_X, 516, "S T A K E", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    const spacing = 92;
    const start = CARDS_CENTER_X - ((CHIP_VALUES.length - 1) * spacing) / 2;

    this.chips = CHIP_VALUES.map((value, index) =>
      this.createChip(start + index * spacing, 562, value),
    );

    this.dealButton = createButton(this, CARDS_CENTER_X, 646, "DEAL", {
      width: 220,
      height: 56,
      fontSize: 20,
      onClick: () => this.deal(),
    });

    this.hitButton = createButton(this, CARDS_CENTER_X - 216, 646, "HIT", {
      width: 128,
      height: 56,
      fontSize: 18,
      onClick: () => this.act("hit"),
    });

    this.standButton = createButton(this, CARDS_CENTER_X - 72, 646, "STAND", {
      width: 128,
      height: 56,
      fontSize: 18,
      onClick: () => this.act("stand"),
    });

    this.doubleButton = createButton(this, CARDS_CENTER_X + 72, 646, "DOUBLE", {
      width: 128,
      height: 56,
      fontSize: 18,
      onClick: () => this.act("double"),
    });

    this.splitButton = createButton(this, CARDS_CENTER_X + 216, 646, "SPLIT", {
      width: 128,
      height: 56,
      fontSize: 18,
      onClick: () => this.act("split"),
    });

    this.refreshChips();
    this.refreshActions();
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
    if (this.busy || this.phase === "player" || value > this.balance) return;

    this.bet = value;
    this.refreshChips();
  }

  refreshChips() {
    const locked = this.busy || this.phase === "player";

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

    if (this.phase !== "player" && this.bet > this.balance) {
      const affordable = CHIP_VALUES.filter((value) => value <= this.balance);
      if (affordable.length > 0) {
        this.bet = affordable[affordable.length - 1];
        this.refreshChips();
      }
    }
  }

  createRulesPanel() {
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
      .text(x, top + 28, "B L A C K J A C K", {
        fontFamily: FONT_BODY,
        fontSize: "12px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.rulesAnchor = { x, top, width };
    this.rulesRows = this.add.container(0, 0);

    this.rtpText = this.add
      .text(x, top + height - 36, "", {
        fontFamily: FONT_BODY,
        fontSize: "12px",
        color: css(COLORS.muted),
        align: "center",
        wordWrap: { width: width - 32 },
      })
      .setOrigin(0.5);
  }

  async loadRules() {
    let table;
    try {
      table = await api.blackjackRules();
    } catch {
      if (this.alive) this.rtpText.setText("Rules unavailable.");
      return;
    }

    if (!this.alive) return;

    const { x, top, width } = this.rulesAnchor;

    table.hands.forEach((entry, index) => {
      const y = top + 64 + index * 42;

      this.rulesRows.add(
        this.add
          .text(x - width / 2 + 18, y, entry.name, {
            fontFamily: FONT_BODY,
            fontSize: "15px",
            color: css(COLORS.cream),
          })
          .setOrigin(0, 0.5),
      );

      this.rulesRows.add(
        this.add
          .text(x + width / 2 - 18, y, entry.pays, {
            fontFamily: FONT_NUMERIC,
            fontSize: "15px",
            fontStyle: "700",
            color: css(COLORS.gold),
          })
          .setOrigin(1, 0.5),
      );
    });

    const rtp = Math.round(table.return_to_player * 1000) / 10;
    const dealer = table.dealer_hits_soft_17 ? "hits soft 17" : "stands on 17";
    this.rtpText.setText(
      `Single deck, ${dealer}. Split pairs once.\nReturns ${rtp}% over the long run.`,
    );
  }

  async loadOpenHand() {
    let payload;
    try {
      payload = await api.blackjackHand(this.session.token);
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
    await this.renderTable(payload.hand, { animate: false });
    this.showResult(this.playPrompt(payload.hand), COLORS.cream);
  }

  onSpace() {
    if (this.phase === "player") {
      this.act("stand");
    } else {
      this.deal();
    }
  }

  async deal() {
    if (this.busy || this.phase === "player" || !this.alive) return;

    if (this.bet > this.balance) {
      this.showResult("Not enough coins for that stake.", COLORS.red);
      return;
    }

    this.busy = true;
    this.netText.setText("");
    this.refreshActions();
    this.refreshChips();
    this.showResult("Dealing…", COLORS.muted);

    let result;
    try {
      result = await api.blackjackDeal(this.session.token, this.bet);
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
      this.refreshActions();
      this.refreshChips();
      return;
    }

    if (!this.alive) return;

    this.setBalance(result.balance);
    await this.renderTable(result);
    this.announce(result);
  }

  async act(action) {
    if (this.busy || this.phase !== "player" || !this.alive) return;

    if (action === "hit" && !this.canHit) return;
    if (action === "stand" && !this.canStand) return;
    if (action === "double" && !this.canDouble) return;
    if (action === "split" && !this.canSplit) return;

    this.busy = true;
    this.refreshActions();
    const pending =
      action === "hit"
        ? "Hitting…"
        : action === "double"
          ? "Doubling…"
          : action === "split"
            ? "Splitting…"
            : "Standing…";
    this.showResult(pending, COLORS.muted);

    let result;
    try {
      result = await api.blackjackAct(this.session.token, action);
    } catch (error) {
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }

      this.showResult(error.message, COLORS.red);
      this.busy = false;
      this.refreshActions();
      return;
    }

    if (!this.alive) return;

    this.setBalance(result.balance);
    await this.renderTable(result);
    this.announce(result);
  }

  async renderTable(table, { animate = true } = {}) {
    this.phase = table.phase === "player" ? "player" : "idle";
    this.canHit = Boolean(table.can_hit);
    this.canStand = Boolean(table.can_stand);
    this.canDouble = Boolean(table.can_double);
    this.canSplit = Boolean(table.can_split);
    this.bet = table.bet ?? this.bet;

    const hands = table.hands?.length
      ? table.hands
      : [{ cards: table.player, total: table.player_total, active: true }];
    const split = hands.length > 1;
    const scale = split ? SPLIT_SCALE : 1;

    this.playerRows.forEach((slots, index) => {
      const centerX = split
        ? CARDS_CENTER_X + (index === 0 ? -SPLIT_OFFSET : SPLIT_OFFSET)
        : CARDS_CENTER_X;
      this.layoutCardRow(slots, centerX, PLAYER_Y, index === 0 || split ? scale : 1);
    });

    for (let index = 0; index < this.playerRows.length; index += 1) {
      const hand = hands[index];
      const slots = this.playerRows[index];
      if (!hand) {
        slots.forEach((card) => {
          card.container.setVisible(false);
          card.container.setAlpha(1);
        });
        continue;
      }

      await this.showCards(slots, hand.cards, { animate });
      const dim = table.phase === "player" && !hand.active;
      slots.forEach((card) => {
        if (card.container.visible) card.container.setAlpha(dim ? 0.5 : 1);
      });
    }

    await this.showCards(this.dealerCards, table.dealer, {
      animate,
      hole: table.phase === "player" && table.dealer.length === 1,
    });

    this.dealerTotal.setText(
      table.dealer_total != null ? String(table.dealer_total) : table.phase === "player" ? "?" : "",
    );
    this.layoutHeading(this.dealerLabel, this.dealerTotal);

    hands.forEach((hand, index) => {
      const ui = this.handUi[index];
      if (!ui) return;
      ui.label.setText(split ? `HAND ${index + 1}` : "YOU").setVisible(true);
      ui.label.setColor(css(hand.active || table.phase !== "player" ? COLORS.cream : COLORS.muted));
      ui.total.setText(hand.total ? String(hand.total) : "").setVisible(true);
      const centerX = split
        ? CARDS_CENTER_X + (index === 0 ? -SPLIT_OFFSET : SPLIT_OFFSET)
        : CARDS_CENTER_X;
      this.layoutHeading(ui.label, ui.total, centerX);
    });

    for (let index = hands.length; index < this.handUi.length; index += 1) {
      this.handUi[index].label.setVisible(false);
      this.handUi[index].total.setVisible(false);
    }

    this.playerLabel = this.handUi[0].label;
    this.playerTotal = this.handUi[0].total;
    this.playerCards = this.playerRows[0];

    this.busy = false;
    this.refreshActions();
    this.refreshChips();
  }

  async showCards(slots, cards, { animate, hole = false } = {}) {
    const shown = cards ?? [];
    const total = shown.length + (hole ? 1 : 0);

    for (let i = 0; i < slots.length; i += 1) {
      const slot = slots[i];
      if (i >= total) {
        slot.container.setVisible(false);
        slot.setFaceDown();
        continue;
      }

      const wasVisible = slot.container.visible;
      const existing = slot.container.getData("card");
      slot.container.setVisible(true);

      if (hole && i === shown.length) {
        slot.setFaceDown();
        continue;
      }

      if (animate && (!wasVisible || !existing)) {
        await slot.flipTo(shown[i], { duration: 180 });
      } else {
        slot.setCard(shown[i]);
      }
    }
  }

  playPrompt(table) {
    const hands = table?.hands ?? [];
    const index = (table?.active_hand ?? 0) + 1;
    const parts = ["Hit", "stand"];
    if (table?.can_double) parts.push("double");
    if (table?.can_split) parts.push("split");
    const last = parts.pop();
    const list = parts.length === 1 ? `${parts[0]} or ${last}` : `${parts.join(", ")}, or ${last}`;
    const lead = hands.length > 1 ? `Hand ${index}. ` : "";
    return `${lead}Your play. ${list}.`;
  }

  announce(result) {
    if (result.phase === "player") {
      this.showResult(this.playPrompt(result), COLORS.cream);
      this.netText.setText("");
      return;
    }

    const hands = result.hands ?? [];
    const kind = result.outcome;
    const payout = result.payout ?? 0;
    const net = result.net ?? 0;
    const lost =
      kind === "player_bust" || kind === "dealer_win" || kind === "dealer_blackjack";

    let message;
    if (hands.length > 1) {
      message = hands
        .map((hand, index) => `Hand ${index + 1}: ${OUTCOME_LABEL[hand.outcome] ?? "done"}`)
        .join(". ");
    } else if (kind === "push") {
      message = "Push. Stake returned.";
    } else if (lost) {
      message = `${OUTCOME_LABEL[kind] ?? "Hand over"}.`;
    } else {
      message = `${OUTCOME_LABEL[kind] ?? "Hand over"}. Pays ${payout.toLocaleString()} coins.`;
    }

    if (hands.length > 1 && net > 0) {
      message = `${message}. Pays ${payout.toLocaleString()} coins.`;
    }

    const color = net > 0 ? COLORS.gold : net === 0 ? COLORS.cream : COLORS.muted;
    this.showResult(message, color);

    if (net > 0) {
      this.netText.setText(`+${net.toLocaleString()}`).setColor(css(COLORS.success));
    } else if (net < 0) {
      this.netText.setText(`${net.toLocaleString()}`).setColor(css(COLORS.red));
    } else {
      this.netText.setText("0").setColor(css(COLORS.muted));
    }

    const winners = hands.length
      ? hands
      : kind === "player_win" || kind === "player_blackjack"
        ? [{ outcome: kind }]
        : [];
    winners.forEach((hand, index) => {
      if (hand.outcome !== "player_win" && hand.outcome !== "player_blackjack") return;
      const row = this.playerRows[index] ?? this.playerRows[0];
      row.forEach((card) => {
        if (card.container.visible) card.celebrate();
      });
    });
    if (kind === "player_blackjack" || winners.some((hand) => hand.outcome === "player_blackjack")) {
      this.cameras.main.flash(220, 232, 196, 106);
    }

    if (this.balance < CHIP_VALUES[0]) {
      this.showResult("You are out of coins. The bank will write a marker.", COLORS.red);
    }
  }

  refreshActions() {
    const playing = this.phase === "player" && !this.busy;
    const dealing = this.phase !== "player" && !this.busy;

    this.dealButton.setVisible(this.phase !== "player");
    this.hitButton.setVisible(this.phase === "player");
    this.standButton.setVisible(this.phase === "player");
    this.doubleButton.setVisible(this.phase === "player");
    this.splitButton.setVisible(this.phase === "player");

    this.dealButton.setEnabled(dealing && this.balance >= CHIP_VALUES[0]).setLabel("DEAL");
    this.hitButton.setEnabled(playing && this.canHit);
    this.standButton.setEnabled(playing && this.canStand);
    this.doubleButton.setEnabled(playing && this.canDouble);
    this.splitButton.setEnabled(playing && this.canSplit);

    if (this.busy && this.phase === "player") {
      this.hitButton.setEnabled(false);
      this.standButton.setEnabled(false);
      this.doubleButton.setEnabled(false);
      this.splitButton.setEnabled(false);
    }
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
