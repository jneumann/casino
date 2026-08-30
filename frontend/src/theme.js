export const GAME_WIDTH = 1280;
export const GAME_HEIGHT = 720;

/** Phaser wants colours as numbers; CSS wants strings. Keep one source. */
export const COLORS = {
  backdrop: 0x050d0a,
  feltCenter: 0x14684a,
  feltEdge: 0x061c14,
  rim: 0x3b2a16,
  gold: 0xe8c46a,
  goldDeep: 0xb8912f,
  cream: 0xf6f1e4,
  muted: 0x93a89c,
  panel: 0x0a2b20,
  red: 0xc2413a,
  success: 0x4fbf87,
};

export const css = (color) => `#${color.toString(16).padStart(6, "0")}`;

// System font stacks only. Fetching a web font would mean a network round trip
// the server cannot serve offline, plus a flash of unstyled canvas text.
export const FONT_DISPLAY =
  '"Playfair Display", "Bodoni 72", Didot, Georgia, "Times New Roman", serif';
export const FONT_BODY =
  'system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif';
export const FONT_NUMERIC =
  '"SF Mono", ui-monospace, Menlo, Consolas, "Courier New", monospace';

export const STARTING_BALANCE = 1000;

/**
 * How each reel symbol is drawn. The server owns the odds and payouts and
 * sends symbols by name; this is only their appearance. Card suits and a seven
 * keep to the existing typography instead of pulling in emoji, which render
 * differently on every platform.
 */
export const SYMBOLS = {
  clubs: { glyph: "\u2663", color: 0xf6f1e4, label: "Clubs" },
  diamonds: { glyph: "\u2666", color: 0xd8544c, label: "Diamonds" },
  hearts: { glyph: "\u2665", color: 0xd8544c, label: "Hearts" },
  spades: { glyph: "\u2660", color: 0xf6f1e4, label: "Spades" },
  star: { glyph: "\u2605", color: 0xe8c46a, label: "Star" },
  seven: { glyph: "7", color: 0xe8c46a, label: "Seven" },
};
