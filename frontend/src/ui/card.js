import { COLORS, FONT_BODY, FONT_DISPLAY, FONT_NUMERIC, RANKS, SUITS, css } from "../theme.js";

/**
 * A playing card drawn with Graphics. Face-down it shows the generated back
 * texture; face-up it draws rank and suit. Clicking toggles HOLD while the
 * table is waiting on a draw.
 */
export function createPlayingCard(scene, { x, y, width = 118, height = 166, onClick = () => {} }) {
  const background = scene.add.graphics();
  const back = scene.add.image(0, 0, "card-back").setDisplaySize(width, height);

  const rankText = scene.add
    .text(-width / 2 + 10, -height / 2 + 8, "", {
      fontFamily: FONT_NUMERIC,
      fontSize: "22px",
      fontStyle: "700",
      color: css(COLORS.cream),
    })
    .setOrigin(0, 0);

  const cornerSuit = scene.add
    .text(-width / 2 + 10, -height / 2 + 30, "", {
      fontFamily: FONT_DISPLAY,
      fontSize: "18px",
      color: css(COLORS.cream),
    })
    .setOrigin(0, 0);

  const centerSuit = scene.add
    .text(0, 8, "", {
      fontFamily: FONT_DISPLAY,
      fontSize: "52px",
      color: css(COLORS.cream),
    })
    .setOrigin(0.5);

  const holdBadge = scene.add
    .text(0, height / 2 + 18, "HOLD", {
      fontFamily: FONT_BODY,
      fontSize: "13px",
      fontStyle: "700",
      color: css(COLORS.gold),
    })
    .setOrigin(0.5)
    .setAlpha(0);

  const face = [background, rankText, cornerSuit, centerSuit];
  const container = scene.add.container(x, y, [...face, back, holdBadge]);
  container.setSize(width, height);

  let held = false;
  let enabled = false;
  let faceUp = false;
  let flipping = false;

  const drawFace = (card) => {
    const suit = SUITS[card.suit];
    const rank = RANKS[card.rank];
    if (!suit || !rank) return;

    const color = css(suit.color);

    background.clear();
    background.fillStyle(0xf6f1e4, 1);
    background.fillRoundedRect(-width / 2, -height / 2, width, height, 10);
    background.lineStyle(2, held ? COLORS.gold : COLORS.goldDeep, held ? 0.95 : 0.55);
    background.strokeRoundedRect(-width / 2, -height / 2, width, height, 10);

    rankText.setText(rank).setColor(color);
    cornerSuit.setText(suit.glyph).setColor(color);
    centerSuit.setText(suit.glyph).setColor(color);
  };

  const showBack = () => {
    face.forEach((item) => item.setVisible(false));
    back.setVisible(true);
    faceUp = false;
  };

  const showFace = (card) => {
    drawFace(card);
    face.forEach((item) => item.setVisible(true));
    back.setVisible(false);
    faceUp = true;
  };

  const setHeld = (value) => {
    held = Boolean(value);
    holdBadge.setAlpha(held ? 1 : 0);
    if (faceUp && container.getData("card")) {
      drawFace(container.getData("card"));
    }
  };

  showBack();
  setHeld(false);

  container
    .setInteractive({
      hitArea: new Phaser.Geom.Rectangle(0, 0, width, height),
      hitAreaCallback: Phaser.Geom.Rectangle.Contains,
      useHandCursor: true,
    })
    .on("pointerup", () => {
      if (!enabled || flipping) return;
      onClick();
    });

  container.input.enabled = false;

  return {
    container,

    get held() {
      return held;
    },

    setHeld,

    toggleHold() {
      setHeld(!held);
      return held;
    },

    setEnabled(value) {
      enabled = value;
      container.input.enabled = value;
      if (enabled) {
        container.setAlpha(1);
      }
      return this;
    },

    setCard(card) {
      container.setData("card", card);
      showFace(card);
      return this;
    },

    setFaceDown() {
      container.setData("card", null);
      showBack();
      setHeld(false);
      return this;
    },

    async flipTo(card, { duration = 220 } = {}) {
      if (flipping) return;
      flipping = true;

      await tweenScaleX(scene, container, 0, duration / 2);
      if (card) {
        container.setData("card", card);
        showFace(card);
      } else {
        container.setData("card", null);
        showBack();
      }
      await tweenScaleX(scene, container, 1, duration / 2);

      flipping = false;
    },

    celebrate() {
      scene.tweens.add({
        targets: container,
        y: y - 10,
        duration: 180,
        yoyo: true,
        ease: "Sine.easeOut",
      });
    },
  };
}

function tweenScaleX(scene, target, to, duration) {
  return new Promise((resolve) => {
    scene.tweens.add({
      targets: target,
      scaleX: to,
      duration,
      ease: "Sine.easeInOut",
      onComplete: resolve,
    });
  });
}
