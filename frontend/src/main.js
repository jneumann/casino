import Phaser from "phaser";

import AuthScene from "./scenes/AuthScene.js";
import BankScene from "./scenes/BankScene.js";
import BlackjackScene from "./scenes/BlackjackScene.js";
import BootScene from "./scenes/BootScene.js";
import LobbyScene from "./scenes/LobbyScene.js";
import PokerScene from "./scenes/PokerScene.js";
import SlotsScene from "./scenes/SlotsScene.js";
import { COLORS, GAME_HEIGHT, GAME_WIDTH, css } from "./theme.js";
import "./style.css";

const game = new Phaser.Game({
  type: Phaser.AUTO,
  parent: "game",
  width: GAME_WIDTH,
  height: GAME_HEIGHT,
  backgroundColor: css(COLORS.backdrop),
  // The sign-in form uses real <input> elements layered over the canvas, which
  // needs Phaser's DOM container enabled. That container spans the whole game,
  // so it must ignore pointer events or it swallows every click meant for the
  // canvas; the form re-enables them on itself in CSS.
  dom: { createContainer: true, pointerEvents: "none" },
  scale: {
    mode: Phaser.Scale.FIT,
    autoCenter: Phaser.Scale.CENTER_BOTH,
  },
  scene: [BootScene, AuthScene, LobbyScene, BankScene, SlotsScene, PokerScene, BlackjackScene],
});

// Phaser overlays the DOM layer with `position: absolute` but no `top`/`left`,
// so its static position picks up the canvas's centring margin and the layer
// ends up offset from the canvas by the letterbox height. Pinning it to the
// origin leaves only the margin Phaser sets itself, which is the offset we want.
game.events.once(Phaser.Core.Events.READY, () => {
  if (!game.domContainer) return;
  game.domContainer.style.top = "0";
  game.domContainer.style.left = "0";
});

// Canvas contents are invisible to devtools, so expose the game for poking at
// scenes from the console.
window.casinoGame = game;
