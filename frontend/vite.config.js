import { defineConfig } from "vite";

const API_TARGET = process.env.CASINO_API_URL ?? "http://127.0.0.1:8080";

export default defineConfig({
  build: {
    // The Rust server serves `static/` at `/`, so the build lands there
    // directly. `emptyOutDir` stays off because `static/admin/` is
    // hand-written and must survive a rebuild.
    outDir: "../static",
    emptyOutDir: false,
    assetsDir: "assets",
    sourcemap: false,
    // Phaser alone is ~1.2 MB unminified; the default 500 kB warning is noise.
    chunkSizeWarningLimit: 1600,
    rollupOptions: {
      // Fixed filenames instead of content hashes: the output is committed
      // alongside `static/admin/`, and hashed names would leave a trail of
      // stale bundles behind every rebuild.
      output: {
        entryFileNames: "assets/casino.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: "assets/casino.[ext]",
      },
    },
  },
  server: {
    port: 5173,
    // `npm run dev` serves the game itself but forwards API calls to the Rust
    // server, so the client can keep using same-origin relative URLs.
    proxy: {
      "/api": { target: API_TARGET, changeOrigin: true },
    },
  },
});
