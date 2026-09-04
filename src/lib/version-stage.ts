// Maps an app version to its release stage label (Alpha / Beta / Public),
// shared by the titlebar badge and every place that needs to describe what
// kind of build a version is.
//
// Rule (by major version):
//   0.x → Alpha
//   1.x → Beta
//   2.x+ → Public
//
// NOTE: .github/scripts/discord-release.mjs computes the same stage for the
// Discord notification and must stay in lockstep with stageForVersion().

export type ReleaseStage = "Alpha" | "Beta" | "Public";

export function stageForMajor(major: number): ReleaseStage {
  if (major <= 0) return "Alpha";
  if (major === 1) return "Beta";
  return "Public";
}

export function stageForVersion(version: string): ReleaseStage {
  const major = parseInt(version.replace(/^v/, "").split(".")[0] ?? "0", 10);
  return stageForMajor(Number.isNaN(major) ? 0 : major);
}
