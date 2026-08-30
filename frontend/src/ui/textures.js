import { COLORS, css } from "../theme.js";

/**
 * Every visual in the game is generated at boot rather than loaded, so the
 * server has no image assets to host and the client has nothing to wait for.
 */
export function createTextures(scene, width, height) {
  createFelt(scene, "felt", width, height);
  createVignette(scene, "vignette", width, height);
  createGlow(scene, "glow", 256, COLORS.gold);
  createChip(scene, "chip-red", 96, 0xb0332c, 0xf2e6d8);
  createChip(scene, "chip-green", 96, 0x1f7a52, 0xf2e6d8);
  createChip(scene, "chip-gold", 96, 0xd6a83c, 0x53401a);
  createChip(scene, "chip-navy", 96, 0x2a3f70, 0xf2e6d8);
}

/** The table surface: a warm pool of light in the middle fading to shadow. */
function createFelt(scene, key, width, height) {
  if (scene.textures.exists(key)) return;

  const texture = scene.textures.createCanvas(key, width, height);
  if (!texture) return;

  const ctx = texture.getContext();
  const gradient = ctx.createRadialGradient(
    width / 2,
    height / 2,
    Math.min(width, height) * 0.05,
    width / 2,
    height / 2,
    Math.max(width, height) * 0.75,
  );
  gradient.addColorStop(0, css(COLORS.feltCenter));
  gradient.addColorStop(0.55, "#0b3a29");
  gradient.addColorStop(1, css(COLORS.feltEdge));

  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);

  // A speckle of lighter dots reads as felt weave once it is behind everything.
  ctx.fillStyle = "rgba(255, 255, 255, 0.022)";
  for (let i = 0; i < 2600; i += 1) {
    const x = Math.random() * width;
    const y = Math.random() * height;
    ctx.fillRect(x, y, 2, 2);
  }

  texture.refresh();
}

/** Darkens the corners so the centre of the table draws the eye. */
function createVignette(scene, key, width, height) {
  if (scene.textures.exists(key)) return;

  const texture = scene.textures.createCanvas(key, width, height);
  if (!texture) return;

  const ctx = texture.getContext();
  const gradient = ctx.createRadialGradient(
    width / 2,
    height / 2,
    Math.min(width, height) * 0.25,
    width / 2,
    height / 2,
    Math.max(width, height) * 0.72,
  );
  gradient.addColorStop(0, "rgba(0, 0, 0, 0)");
  gradient.addColorStop(0.7, "rgba(0, 0, 0, 0.35)");
  gradient.addColorStop(1, "rgba(0, 0, 0, 0.82)");

  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);
  texture.refresh();
}

/** A soft radial falloff, used for highlights behind the balance readout. */
function createGlow(scene, key, size, color) {
  if (scene.textures.exists(key)) return;

  const texture = scene.textures.createCanvas(key, size, size);
  if (!texture) return;

  const ctx = texture.getContext();
  const radius = size / 2;
  const gradient = ctx.createRadialGradient(radius, radius, 0, radius, radius, radius);
  const { r, g, b } = splitColor(color);

  gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, 0.55)`);
  gradient.addColorStop(0.45, `rgba(${r}, ${g}, ${b}, 0.18)`);
  gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, size, size);
  texture.refresh();
}

/** A poker chip: coloured face, contrasting edge, six edge dashes, inset ring. */
function createChip(scene, key, size, faceColor, edgeColor) {
  if (scene.textures.exists(key)) return;

  const g = scene.make.graphics({ add: false });
  const radius = size / 2;

  g.fillStyle(edgeColor, 1);
  g.fillCircle(radius, radius, radius);

  g.fillStyle(faceColor, 1);
  const dashes = 6;
  const arc = (Math.PI * 2) / dashes;
  for (let i = 0; i < dashes; i += 1) {
    const start = i * arc + arc * 0.28;
    const end = start + arc * 0.44;
    g.slice(radius, radius, radius, start, end, false);
    g.fillPath();
  }

  g.fillStyle(faceColor, 1);
  g.fillCircle(radius, radius, radius * 0.76);

  g.lineStyle(Math.max(2, size * 0.022), edgeColor, 0.85);
  g.strokeCircle(radius, radius, radius * 0.6);

  g.lineStyle(Math.max(1, size * 0.012), 0xffffff, 0.18);
  g.strokeCircle(radius, radius, radius * 0.44);

  g.generateTexture(key, size, size);
  g.destroy();
}

function splitColor(color) {
  return {
    r: (color >> 16) & 0xff,
    g: (color >> 8) & 0xff,
    b: color & 0xff,
  };
}
