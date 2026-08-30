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
  SYMBOLS,
  css,
} from "../theme.js";
import { createButton } from "../ui/button.js";
import { createReel } from "../ui/reel.js";
import { createTable } from "../ui/table.js";

const CHIP_VALUES = [5, 10, 25, 50, 100];

const REEL_COUNT = 3;
const CELL_WIDTH = 150;
const CELL_HEIGHT = 150;
const REEL_GAP = 18;
const REELS_CENTER_X = 800;
const REELS_CENTER_Y = 300;

export default class SlotsScene extends Phaser.Scene {
  constructor() {
    super("Slots");
  }

  init(data) {
    this.session = data.session;
    this.displayedBalance = this.session.balance ?? 0;
    this.bet = CHIP_VALUES[0];
    this.spinning = false;
    this.alive = true;
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

    this.input.keyboard.on("keydown-SPACE", () => this.spin());
    this.cameras.main.fadeIn(320, 0, 0, 0);

    this.setBalance(this.session.balance ?? 0, { animate: false });
    this.loadPaytable();
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
      .text(GAME_WIDTH / 2, 48, "SLOTS", {
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

  createMachine() {
    const totalWidth = REEL_COUNT * CELL_WIDTH + (REEL_COUNT - 1) * REEL_GAP;
    const left = REELS_CENTER_X - totalWidth / 2;
    const top = REELS_CENTER_Y - CELL_HEIGHT / 2;

    this.add
      .image(REELS_CENTER_X, REELS_CENTER_Y, "glow")
      .setScale(3.2, 1.6)
      .setAlpha(0.35);

    const cabinet = this.add.graphics();
    cabinet.fillStyle(0x04150f, 0.9);
    cabinet.fillRoundedRect(left - 26, top - 30, totalWidth + 52, CELL_HEIGHT + 60, 20);
    cabinet.lineStyle(2, COLORS.goldDeep, 0.85);
    cabinet.strokeRoundedRect(left - 26, top - 30, totalWidth + 52, CELL_HEIGHT + 60, 20);

    this.reels = [];
    for (let i = 0; i < REEL_COUNT; i += 1) {
      const x = left + CELL_WIDTH / 2 + i * (CELL_WIDTH + REEL_GAP);

      const slot = this.add.graphics();
      slot.fillStyle(0x000000, 0.5);
      slot.fillRoundedRect(x - CELL_WIDTH / 2, top, CELL_WIDTH, CELL_HEIGHT, 12);

      this.reels.push(
        createReel(this, {
          x,
          centerY: REELS_CENTER_Y,
          cellWidth: CELL_WIDTH,
          cellHeight: CELL_HEIGHT,
        }),
      );

      const frame = this.add.graphics();
      frame.lineStyle(1.5, COLORS.goldDeep, 0.55);
      frame.strokeRoundedRect(x - CELL_WIDTH / 2, top, CELL_WIDTH, CELL_HEIGHT, 12);
    }

    // The payline the three windows sit on.
    const payline = this.add.graphics();
    payline.lineStyle(2, COLORS.gold, 0.22);
    payline.lineBetween(left - 10, REELS_CENTER_Y, left + totalWidth + 10, REELS_CENTER_Y);

    this.resultText = this.add
      .text(REELS_CENTER_X, 424, "Choose your stake and pull the handle.", {
        fontFamily: FONT_BODY,
        fontSize: "19px",
        color: css(COLORS.muted),
        align: "center",
        wordWrap: { width: 520 },
      })
      .setOrigin(0.5);

    this.netText = this.add
      .text(REELS_CENTER_X, 458, "", {
        fontFamily: FONT_NUMERIC,
        fontSize: "23px",
        fontStyle: "700",
        color: css(COLORS.success),
      })
      .setOrigin(0.5);
  }

  createControls() {
    this.add
      .text(REELS_CENTER_X, 506, "S T A K E", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    const spacing = 92;
    const start = REELS_CENTER_X - ((CHIP_VALUES.length - 1) * spacing) / 2;

    this.chips = CHIP_VALUES.map((value, index) =>
      this.createChip(start + index * spacing, 560, value),
    );

    this.spinButton = createButton(this, REELS_CENTER_X, 646, "SPIN", {
      width: 260,
      height: 62,
      fontSize: 21,
      onClick: () => this.spin(),
    });

    this.refreshChips();
  }

  /** A clickable betting chip. Selected chips sit raised and gold-ringed. */
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
        // Centred on (radius, radius), not (0, 0): Phaser shifts the hit test
        // point by displayOrigin, so hit areas are measured from the top-left.
        hitArea: new Phaser.Geom.Circle(radius, radius, radius),
        hitAreaCallback: Phaser.Geom.Circle.Contains,
        useHandCursor: true,
      })
      .on("pointerup", () => this.selectBet(value));

    return { value, container, image, label, ring };
  }

  selectBet(value) {
    if (this.spinning || value > this.balance) return;

    this.bet = value;
    this.refreshChips();
  }

  refreshChips() {
    this.chips.forEach((chip) => {
      const affordable = chip.value <= this.balance;
      const selected = chip.value === this.bet;

      chip.image.setTexture(selected ? "chip-gold" : "chip-navy");
      chip.label.setColor(css(selected ? 0x1c1405 : COLORS.cream));
      chip.container.setAlpha(affordable ? 1 : 0.32);

      chip.ring.clear();
      if (selected) {
        chip.ring.lineStyle(2.5, COLORS.gold, 0.9);
        chip.ring.strokeCircle(0, 0, 38);
      }
    });

    // Falling below the current stake should not leave an unaffordable bet
    // selected; drop to the largest chip still within reach.
    if (this.bet > this.balance) {
      const affordable = CHIP_VALUES.filter((value) => value <= this.balance);
      if (affordable.length > 0) {
        this.bet = affordable[affordable.length - 1];
        this.refreshChips();
      }
    }
  }

  createPaytablePanel() {
    const x = 196;
    const width = 272;
    const top = 118;
    const height = 470;

    const panel = this.add.graphics();
    panel.fillStyle(COLORS.panel, 0.72);
    panel.fillRoundedRect(x - width / 2, top, width, height, 16);
    panel.lineStyle(1.2, COLORS.goldDeep, 0.5);
    panel.strokeRoundedRect(x - width / 2, top, width, height, 16);

    this.add
      .text(x, top + 28, "P A Y T A B L E", {
        fontFamily: FONT_BODY,
        fontSize: "13px",
        fontStyle: "600",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.add
      .text(x - width / 2 + 108, top + 56, "THREE", {
        fontFamily: FONT_BODY,
        fontSize: "11px",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.add
      .text(x + width / 2 - 46, top + 56, "TWO", {
        fontFamily: FONT_BODY,
        fontSize: "11px",
        color: css(COLORS.muted),
      })
      .setOrigin(0.5);

    this.paytableAnchor = { x, top, width };
    this.paytableRows = this.add.container(0, 0);

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
      table = await api.paytable();
    } catch {
      // Cosmetic only — the machine still plays without the panel filled in.
      if (this.alive) this.rtpText.setText("Paytable unavailable.");
      return;
    }

    if (!this.alive) return;

    const { x, top, width } = this.paytableAnchor;

    table.symbols.forEach((entry, index) => {
      const symbol = SYMBOLS[entry.symbol];
      if (!symbol) return;

      const y = top + 88 + index * 44;

      this.paytableRows.add(
        this.add
          .text(x - width / 2 + 30, y, symbol.glyph, {
            fontFamily: FONT_DISPLAY,
            fontSize: "30px",
            color: css(symbol.color),
          })
          .setOrigin(0.5),
      );

      this.paytableRows.add(
        this.add
          .text(x - width / 2 + 108, y, `${entry.three_of_a_kind}x`, {
            fontFamily: FONT_NUMERIC,
            fontSize: "17px",
            fontStyle: "700",
            color: css(COLORS.cream),
          })
          .setOrigin(0.5),
      );

      this.paytableRows.add(
        this.add
          .text(x + width / 2 - 46, y, `${entry.pair}x`, {
            fontFamily: FONT_NUMERIC,
            fontSize: "17px",
            color: css(COLORS.muted),
          })
          .setOrigin(0.5),
      );
    });

    const rtp = Math.round(table.return_to_player * 1000) / 10;
    this.rtpText.setText(
      `Stakes from ${table.min_bet} to ${table.max_bet} coins.\nReturns ${rtp}% over the long run.`,
    );
  }

  async spin() {
    if (this.spinning || !this.alive) return;

    if (this.bet > this.balance) {
      this.showResult("Not enough coins for that stake.", COLORS.red);
      return;
    }

    this.spinning = true;
    this.spinButton.setEnabled(false).setLabel("Spinning…");
    this.netText.setText("");
    this.showResult("Rolling…", COLORS.muted);

    let result;
    try {
      result = await api.spin(this.session.token, this.bet);
    } catch (error) {
      if (error.isUnauthorized) {
        this.signOut();
        return;
      }

      this.showResult(error.message, COLORS.red);
      this.spinning = false;
      this.spinButton.setEnabled(true).setLabel("SPIN");
      return;
    }

    if (!this.alive) return;

    // Stagger the stops so the last reel lands last, the way a real machine
    // draws out the reveal.
    await Promise.all(
      this.reels.map((reel, index) =>
        reel.spinTo(result.reels[index], { duration: 1100 + index * 320 }),
      ),
    );

    if (!this.alive) return;

    this.settle(result);
  }

  settle(result) {
    this.setBalance(result.balance);

    if (result.outcome.kind === "nothing") {
      this.showResult("No match. Spin again.", COLORS.muted);
      this.netText.setText(`-${result.bet.toLocaleString()}`).setColor(css(COLORS.red));
    } else {
      const symbol = SYMBOLS[result.outcome.symbol];
      const name = symbol ? symbol.label.toLowerCase() : result.outcome.symbol;
      const heading =
        result.outcome.kind === "three"
          ? `Three ${name}! Pays ${result.payout.toLocaleString()} coins.`
          : `Pair of ${name} pays ${result.payout.toLocaleString()} coins.`;

      this.showResult(heading, result.net >= 0 ? COLORS.gold : COLORS.cream);
      this.reels.forEach((reel) => reel.celebrate());

      const sign = result.net >= 0 ? "+" : "-";
      this.netText
        .setText(`${sign}${Math.abs(result.net).toLocaleString()}`)
        .setColor(css(result.net >= 0 ? COLORS.success : COLORS.red));

      if (result.outcome.kind === "three") {
        this.cameras.main.flash(220, 232, 196, 106);
      }
    }

    this.spinning = false;
    this.spinButton.setEnabled(true).setLabel("SPIN");
    this.refreshChips();

    if (this.balance < CHIP_VALUES[0]) {
      this.showResult("You are out of coins. The bank will write a marker.", COLORS.red);
      this.spinButton.setEnabled(false);
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
    if (this.spinning) return;

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
