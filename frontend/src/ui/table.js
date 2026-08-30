import Phaser from "phaser";

import { COLORS, GAME_HEIGHT, GAME_WIDTH } from "../theme.js";

const CHIP_KEYS = ["chip-red", "chip-green", "chip-gold", "chip-navy"];

/**
 * Lays down the shared backdrop: felt, a gold table arc, drifting chips, and a
 * vignette on top. Both scenes call this so the background never jumps when
 * the player moves between them.
 */
export function createTable(scene) {
  scene.add.image(GAME_WIDTH / 2, GAME_HEIGHT / 2, "felt").setDepth(-100);

  drawTableArc(scene);
  createDriftingChips(scene, 14);

  scene.add
    .image(GAME_WIDTH / 2, GAME_HEIGHT / 2, "vignette")
    .setDepth(-10)
    .setBlendMode(Phaser.BlendModes.MULTIPLY);
}

/** The brass betting arc that suggests the edge of a card table. */
function drawTableArc(scene) {
  const arc = scene.add.graphics().setDepth(-90);
  const centerX = GAME_WIDTH / 2;
  const centerY = GAME_HEIGHT + 210;

  arc.lineStyle(3, COLORS.goldDeep, 0.35);
  arc.strokeCircle(centerX, centerY, 560);

  arc.lineStyle(1, COLORS.gold, 0.18);
  arc.strokeCircle(centerX, centerY, 588);

  arc.lineStyle(2, COLORS.goldDeep, 0.2);
  arc.strokeCircle(centerX, -260, 520);
}

/** Chips that float slowly upward, purely as ambience. */
function createDriftingChips(scene, count) {
  for (let i = 0; i < count; i += 1) {
    const key = Phaser.Utils.Array.GetRandom(CHIP_KEYS);
    const chip = scene.add
      .image(
        Phaser.Math.Between(40, GAME_WIDTH - 40),
        Phaser.Math.Between(0, GAME_HEIGHT),
        key,
      )
      .setDepth(-80)
      .setScale(Phaser.Math.FloatBetween(0.22, 0.5))
      .setAlpha(Phaser.Math.FloatBetween(0.05, 0.16))
      .setAngle(Phaser.Math.Between(0, 360));

    scene.tweens.add({
      targets: chip,
      y: chip.y - Phaser.Math.Between(140, 320),
      angle: chip.angle + Phaser.Math.Between(-120, 120),
      duration: Phaser.Math.Between(9000, 18000),
      ease: "Sine.easeInOut",
      yoyo: true,
      repeat: -1,
      delay: Phaser.Math.Between(0, 4000),
    });
  }
}
