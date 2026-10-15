// Bundles js/entry.ts (and, through it, @openmouse/protocol plus the Tauri
// app's src/native-hid/*) into one script for the embedded QuickJS runtime.
// Run from build.rs; output is js/dist/bundle.js.
import { build } from "../../node_modules/esbuild/lib/main.js";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
await build({
  entryPoints: [path.join(here, "entry.ts")],
  outfile: path.join(here, "dist/bundle.js"),
  bundle: true,
  format: "iife",
  target: "es2020",
  platform: "neutral",
  mainFields: ["module", "main"],
  conditions: ["import"],
  // Identifiers stay: scan.ts reports `client.constructor.name` in errors.
  minifyWhitespace: true,
  minifySyntax: true,
  legalComments: "none",
  alias: {
    "@tauri-apps/api/core": path.join(here, "shims/tauri-core.ts"),
    "@tauri-apps/api/event": path.join(here, "shims/tauri-event.ts"),
  },
  logLevel: "warning",
});

// The Tauri app's hand-maintained changelog (src/lib/changelog.ts), evaluated at
// build time to JSON so the native app shows exactly the same entries from one
// source. It is plain data, so no runtime is needed to read it.
const changelog = await build({
  entryPoints: [path.join(here, "../../src/lib/changelog.ts")],
  bundle: true,
  format: "cjs",
  platform: "node",
  write: false,
  logLevel: "warning",
});
const mod = { exports: {} };
new Function("module", "exports", changelog.outputFiles[0].text)(mod, mod.exports);
fs.writeFileSync(path.join(here, "dist/changelog.json"), JSON.stringify(mod.exports.CHANGELOG));
