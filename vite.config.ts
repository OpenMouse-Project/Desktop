import path from "node:path";
import { rm } from "node:fs/promises";
import { defineConfig, type Plugin } from "vite";
import preact from "@preact/preset-vite";

/**
 * public/devices/*.png are ~15MB of device-card product photos, referenced
 * only by string path (native-hid/device-images.ts builds the path from a
 * vendor:product id lookup) rather than statically imported — so Rollup has
 * no reference to tree-shake and Vite copies the whole public/ dir into
 * every build verbatim regardless of mode. BridgeView never renders a
 * device image, so for the bridge build these are 15MB of dead weight in
 * the installer. Vite's `publicDir` is all-or-nothing (no per-subfolder
 * exclude), so this just deletes the copied dist-bridge/devices/ dir after
 * the fact instead.
 */
function pruneBridgeDeviceImages(): Plugin {
  return {
    name: "prune-bridge-device-images",
    apply: "build",
    async closeBundle() {
      await rm(path.resolve(__dirname, "dist-bridge/devices"), { recursive: true, force: true });
    },
  };
}

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async ({ mode }) => ({
  plugins: [preact(), ...(mode === "bridge" ? [pruneBridgeDeviceImages()] : [])],

  // Bridge/Desktop split: main.tsx imports its App component from the
  // virtual specifier "~app-entry" rather than "./App" directly, and this
  // alias picks which actual file that resolves to based on build mode
  // (`vite build --mode bridge`, driven by package.json's `build:bridge`
  // script). This is a real file swap, not a runtime flag — App.bridge.tsx
  // never imports FullDesktopView, so nothing FullDesktopView pulls in
  // (device tabs, device images, game-profile code) enters the bridge
  // build's module graph at all, instead of just being unreachable at
  // runtime. `tauri dev` and the default `npm run build` don't pass
  // --mode, so they keep resolving to App.tsx exactly as before.
  resolve: {
    alias: {
      "~app-entry": path.resolve(__dirname, mode === "bridge" ? "src/App.bridge.tsx" : "src/App.tsx"),
    },
  },

  // Bridge build writes to its own dist-bridge/ dir (see
  // src-tauri/tauri.bridge.conf.json's frontendDist) so it never shares —
  // or gets clobbered by — the desktop build's dist/.
  build: mode === "bridge" ? { outDir: "dist-bridge" } : undefined,

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
