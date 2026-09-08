// Cross-grade: switching between the Bridge-only and full Desktop builds.
//
// These are genuinely separate installs (see src-tauri/tauri.bridge.conf.json
// — a different `identifier`, so macOS/Windows/Linux all treat them as
// distinct apps that can coexist). All the actual work — the shared
// "what's installed" manifest, downloading, and handing the result to the
// OS's own installer — lives in src-tauri/src/cross_grade.rs; this module
// is just the thin JS-side wrapper around those commands, plus the
// fallback when neither path is available (offline, or GitHub has no
// matching asset — e.g. this OS isn't in the release matrix yet).
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

const RELEASES_URL = "https://github.com/OpenMouse-Project/Desktop/releases/latest";

export type AppVariant = "bridge" | "full-desktop";

interface InstalledVariant {
  variant: string;
  exe_path: string;
  version: string;
}

/**
 * Records this build's own variant + exe path in the shared manifest.
 * Call once on startup (App.tsx / App.bridge.tsx each pass their own
 * hardcoded variant, since the Rust side has no other way to know which
 * one it was built as — see cross_grade.rs's doc comment). Best-effort:
 * failures are swallowed on the Rust side already, nothing to catch here.
 */
export function registerVariant(variant: AppVariant): void {
  void invoke("register_variant", { variant });
}

/**
 * Switches to `target`: if it's already installed (per the shared
 * manifest), just launches it and exits this process — no network
 * involved. Otherwise fetches the matching installer from the latest
 * GitHub release and hands it to the OS's own installer/opener. Falls
 * back to just opening the releases page if neither the manifest lookup
 * nor GitHub has a usable entry (offline, or this OS isn't in the release
 * matrix), so the user isn't left with a dead button.
 */
export async function requestVariant(target: AppVariant): Promise<void> {
  const installed = await invoke<InstalledVariant[]>("get_installed_variants").catch(() => []);
  const existing = installed.find((v) => v.variant === target);
  if (existing) {
    await invoke("switch_to_installed_variant", { exePath: existing.exe_path });
    return;
  }

  const asset = await invoke<[name: string, url: string] | null>("find_installer_asset", {
    variant: target,
  }).catch(() => null);
  if (!asset) {
    await openUrl(RELEASES_URL);
    return;
  }

  const [fileName, url] = asset;
  await invoke("download_and_launch_installer", { url, fileName });
}
