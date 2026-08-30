import Phaser from "phaser";

import { COLORS, FONT_DISPLAY, SYMBOLS, css } from "../theme.js";

const SYMBOL_KEYS = Object.keys(SYMBOLS);

/**
 * One reel: a tall strip of symbols behind a one-cell window. Spinning builds
 * a fresh strip that ends on the symbol the server already chose, then scrolls
 * to it — the animation reveals the result, it never decides it.
 */
export function createReel(scene, { x, centerY, cellWidth, cellHeight }) {
  const container = scene.add.container(x, centerY);

  const window = scene.make.graphics({ add: false });
  window.fillRect(x - cellWidth / 2, centerY - cellHeight / 2, cellWidth, cellHeight);
  container.setMask(window.createGeometryMask());

  let symbols = [];

  const build = (keys) => {
    symbols.forEach((text) => text.destroy());
    symbols = keys.map((key, index) => {
      const symbol = SYMBOLS[key];
      const text = scene.add
        .text(0, index * cellHeight, symbol.glyph, {
          fontFamily: FONT_DISPLAY,
          fontSize: "96px",
          color: css(symbol.color),
        })
        .setOrigin(0.5)
        .setShadow(0, 3, "rgba(0, 0, 0, 0.5)", 8);
      container.add(text);
      return text;
    });
  };

  const showAt = (index) => container.setY(centerY - index * cellHeight);

  build([SYMBOL_KEYS[0]]);
  showAt(0);

  return {
    container,

    /** Puts a symbol in the window with no animation. */
    show(key) {
      build([key]);
      showAt(0);
    },

    /**
     * Scrolls through `blanks` random symbols before settling on `key`.
     * Resolves once the reel has stopped.
     */
    spinTo(key, { blanks = 18, duration = 1200 } = {}) {
      const strip = [];
      for (let i = 0; i < blanks; i += 1) {
        strip.push(Phaser.Utils.Array.GetRandom(SYMBOL_KEYS));
      }
      strip.push(key);

      build(strip);
      showAt(0);

      return new Promise((resolve) => {
        scene.tweens.add({
          targets: container,
          y: centerY - (strip.length - 1) * cellHeight,
          duration,
          ease: "Cubic.easeOut",
          onComplete: () => resolve(),
        });
      });
    },

    /** Pulses the symbol currently in the window to mark a win. */
    celebrate() {
      const landed = symbols[symbols.length - 1];
      if (!landed) return;

      scene.tweens.add({
        targets: landed,
        scale: 1.22,
        duration: 240,
        ease: "Sine.easeInOut",
        yoyo: true,
        repeat: 2,
      });
    },

    destroy() {
      window.destroy();
      container.destroy();
    },
  };
}
