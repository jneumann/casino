import Phaser from "phaser";

import { COLORS, FONT_BODY, css } from "../theme.js";

const VARIANTS = {
  primary: {
    fill: COLORS.gold,
    fillHover: 0xf5d68a,
    fillDown: COLORS.goldDeep,
    border: COLORS.goldDeep,
    text: 0x1c1405,
  },
  ghost: {
    fill: 0x000000,
    fillHover: 0x0e3527,
    fillDown: 0x082018,
    border: COLORS.goldDeep,
    text: COLORS.gold,
  },
};

/**
 * A rounded pill button drawn with Graphics. Returns the container with
 * `setEnabled` and `setLabel` attached.
 */
export function createButton(scene, x, y, label, options = {}) {
  const {
    width = 220,
    height = 52,
    variant = "primary",
    fontSize = 17,
    onClick = () => {},
  } = options;

  const palette = VARIANTS[variant] ?? VARIANTS.primary;
  const radius = height / 2;
  const background = scene.add.graphics();

  const text = scene.add
    .text(0, 0, label, {
      fontFamily: FONT_BODY,
      fontSize: `${fontSize}px`,
      fontStyle: "600",
      color: css(palette.text),
    })
    .setOrigin(0.5);

  const container = scene.add.container(x, y, [background, text]);
  container.setSize(width, height);

  let enabled = true;
  let state = "idle";

  const draw = () => {
    background.clear();

    if (!enabled) {
      background.fillStyle(0x0a2019, 0.6);
      background.fillRoundedRect(-width / 2, -height / 2, width, height, radius);
      background.lineStyle(1.5, COLORS.muted, 0.25);
      background.strokeRoundedRect(-width / 2, -height / 2, width, height, radius);
      text.setColor(css(COLORS.muted)).setAlpha(0.7);
      return;
    }

    const fill =
      state === "down"
        ? palette.fillDown
        : state === "hover"
          ? palette.fillHover
          : palette.fill;
    const alpha = variant === "ghost" ? (state === "idle" ? 0.25 : 0.55) : 1;

    background.fillStyle(fill, alpha);
    background.fillRoundedRect(-width / 2, -height / 2, width, height, radius);
    background.lineStyle(1.5, palette.border, state === "idle" ? 0.7 : 1);
    background.strokeRoundedRect(-width / 2, -height / 2, width, height, radius);
    text.setColor(css(palette.text)).setAlpha(1);
  };

  const setState = (next) => {
    state = next;
    draw();
  };

  // Hit areas are measured from the top-left, not from the origin: before
  // calling the callback Phaser adds displayOrigin to the local point, so a
  // click at the centre of this container arrives as (width/2, height/2).
  // A rectangle centred on (0, 0) would therefore sit up and to the left of
  // the button people can see.
  container
    .setInteractive({
      hitArea: new Phaser.Geom.Rectangle(0, 0, width, height),
      hitAreaCallback: Phaser.Geom.Rectangle.Contains,
      useHandCursor: true,
    })
    .on("pointerover", () => enabled && setState("hover"))
    .on("pointerout", () => enabled && setState("idle"))
    .on("pointerdown", () => enabled && setState("down"))
    .on("pointerup", () => {
      if (!enabled) return;
      setState("hover");
      onClick();
    });

  container.setEnabled = (value) => {
    enabled = value;
    container.input.enabled = value;
    if (!value) {
      state = "idle";
    }
    draw();
    return container;
  };

  container.setLabel = (value) => {
    text.setText(value);
    return container;
  };

  draw();
  return container;
}
