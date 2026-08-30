import Phaser from "phaser";

import { api } from "../api.js";
import { clearSession, loadSession, saveSession } from "../session.js";
import { COLORS, FONT_DISPLAY, GAME_HEIGHT, GAME_WIDTH, css } from "../theme.js";
import { createTextures } from "../ui/textures.js";

/**
 * Generates the artwork, then decides where the player lands: straight to the
 * lobby if their stored token still works, otherwise the sign-in screen.
 */
export default class BootScene extends Phaser.Scene {
  constructor() {
    super("Boot");
  }

  create() {
    createTextures(this, GAME_WIDTH, GAME_HEIGHT);

    this.add.image(GAME_WIDTH / 2, GAME_HEIGHT / 2, "felt");
    this.add
      .image(GAME_WIDTH / 2, GAME_HEIGHT / 2, "vignette")
      .setBlendMode(Phaser.BlendModes.MULTIPLY);

    const status = this.add
      .text(GAME_WIDTH / 2, GAME_HEIGHT / 2, "Shuffling the deck…", {
        fontFamily: FONT_DISPLAY,
        fontSize: "26px",
        color: css(COLORS.gold),
      })
      .setOrigin(0.5);

    this.tweens.add({
      targets: status,
      alpha: 0.35,
      duration: 900,
      yoyo: true,
      repeat: -1,
      ease: "Sine.easeInOut",
    });

    this.resume();
  }

  async resume() {
    const session = loadSession();

    if (!session) {
      this.goTo("Auth");
      return;
    }

    // The stored token may have been revoked or the account removed, so
    // confirm it against the server before showing a stale balance.
    try {
      const wallet = await api.balance(session.token);
      const refreshed = {
        ...session,
        balance: wallet.balance,
        username: wallet.username,
        loan: wallet.loan ?? null,
      };
      saveSession(refreshed);
      this.goTo("Lobby", { session: refreshed });
    } catch (error) {
      if (error.isUnauthorized) {
        clearSession();
        this.goTo("Auth", { notice: "Your session expired. Please sign in again." });
        return;
      }

      // A network blip should not cost the player their session; let them into
      // the lobby and surface the problem when it retries there.
      this.goTo("Lobby", { session, error: error.message });
    }
  }

  /** Small delay so the shuffle message does not flash by on a fast connection. */
  goTo(scene, data = {}) {
    this.time.delayedCall(450, () => this.scene.start(scene, data));
  }
}
