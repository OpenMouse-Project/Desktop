// Posts a release notification to Discord when a new version ships.
//
// Run from a GitHub Actions job that has already checked out the repo. Reads:
//   - the release tag (github.ref_name, e.g. "v0.0.23") from argv[2]
//   - the repo slug (github.repository) from argv[3]
//   - the Discord webhook URL from DISCORD_WEBHOOK_URL
//
// The changelog is the hand-maintained, user-facing src/lib/changelog.ts —
// the same source Settings → Changelog reads — so the Discord post and the
// in-app changelog never drift. It finds the entry matching the release
// version and posts its full change list as the patch notes.
//
// Platforms are reported in a fixed, consolidated format: Windows and Linux
// are available today (signed installers and distro-neutral Linux bundles),
// macOS is not built yet and shows as "coming soon". Linux collapses its
// .deb/.rpm/.AppImage into a single entry rather than one line per file.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const changelogPath = join(__dirname, "..", "..", "src", "lib", "changelog.ts");

const tag = process.argv[2] ?? "";
const repo = process.argv[3] ?? "";
const version = tag.replace(/^v/, "");

const webhookUrl = process.env.DISCORD_WEBHOOK_URL;
if (!webhookUrl) {
  console.error("DISCORD_WEBHOOK_URL is not set; skipping Discord notification.");
  process.exit(0);
}

// ── extract the latest changelog entry for this version ──────────────────
const source = readFileSync(changelogPath, "utf8");
const entryRe = /version:\s*"([^"]+)",\s*date:\s*"([^"]+)",\s*changes:\s*\[([\s\S]*?)\]\s*,?\s*\}/g;
let match;
let entry = null;
while ((match = entryRe.exec(source)) !== null) {
  if (match[1] === version) {
    entry = match;
    break;
  }
}
if (!entry) {
  console.error(`No changelog entry found for version "${version}".`);
  process.exit(1);
}

const changeRe = /"((?:[^"\\]|\\.)*)"/g;
const changes = [];
let cm;
while ((cm = changeRe.exec(entry[3])) !== null) {
  changes.push(cm[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\"));
}

const patchNotes = changes.map((line) => `• ${line}`).join("\n") || "No patch notes for this release.";

// Release stage from the version's major number — must stay in lockstep with
// src/lib/version-stage.ts (0.x Alpha, 1.x Beta, 2.x+ Public).
function stageForVersion(v) {
  const major = parseInt(v.replace(/^v/, "").split(".")[0] ?? "0", 10);
  if (isNaN(major) || major <= 0) return "Alpha";
  if (major === 1) return "Beta";
  return "Public";
}
const stage = stageForVersion(version);

const releaseUrl = `https://github.com/${repo}/releases/tag/${tag}`;

// ── build the embed ──────────────────────────────────────────────────────
const colors = {
  Alpha: 0x5865f2, // blurple
  Beta: 0xf1c40f, // yellow
  Public: 0x2ecc71, // green
};
const embed = {
  title: `OpenMouse Desktop ${version} · ${stage}`,
  url: releaseUrl,
  color: colors[stage],
  description: patchNotes,
  fields: [
    { name: "Windows", value: "Available", inline: true },
    { name: "Linux", value: "Available", inline: true },
    { name: "macOS", value: "Coming soon", inline: true },
    { name: "Download", value: `**[⬇️ Get it here](<${releaseUrl}>)** — installer for your platform`, inline: false },
  ],
  footer: { text: `OpenMouse Desktop · ${stage}` },
};

const payload = {
  content: `**New ${stage} version released: ${tag}** 🚀`,
  embeds: [embed],
};

for (let attempt = 1; attempt <= 3; attempt++) {
  try {
    const res = await fetch(webhookUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      console.error(`Discord webhook returned ${res.status} ${res.statusText}: ${await res.text()}`);
      if (attempt === 3) process.exit(1);
    } else {
      console.log(`Discord notification sent for ${tag}.`);
      process.exit(0);
    }
  } catch (err) {
    console.error(`Discord webhook attempt ${attempt} failed: ${err.message}`);
    if (attempt === 3) process.exit(1);
  }
  await new Promise((r) => setTimeout(r, 2000 * attempt));
}
process.exit(1);
