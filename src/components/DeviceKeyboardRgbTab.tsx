// Wooting RGB tab: block sizes, row counts, colour swatches.
//
// Read-only: core (0x32), colour pages (0x23/0x24), layer (0x39) render as
// sizes + row counts with a swatch placeholder — no effect parsing, no RGB
// writes. Apply is stubbed: RGB persists to onboard flash, so the write
// wave adds a blocking confirm plus the real payload after a capture.
// Per-key RGB painting is out of scope for this wave.

import { useState } from "preact/hooks";
import { WOOTING_COMMAND } from "@openmouse/keyboard-protocol/wooting";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { resetWootingRgb, saveWootingProfile, setWootingKeyColor, setWootingRgbBuffer } from "../native-hid/write";
import { showToast } from "../lib/toast";
import { confirmFlashOverwrite, rowHex, type KeyboardTabProps } from "./DeviceKeyboardShared";
import type { WootingRgbBlock } from "@openmouse/keyboard-protocol/wooting";

interface Props extends KeyboardTabProps {
  info: HidInterfaceInfo;
}

function RgbBlockSection({ title, block }: { title: string; block: WootingRgbBlock | null | undefined }) {
  if (!block) return null;
  return (
    <>
      <span class="info-section-title">{title}</span>
      <div class="info-grid">
        <div class="info-row">
          <span class="info-label">Rows</span>
          <span class="info-value">{block.groups.length} group{block.groups.length === 1 ? "" : "s"} · {block.raw.length} bytes</span>
        </div>
        {block.trailer && (
          <div class="info-row">
            <span class="info-label">Trailer</span>
            <span class="info-value info-value-mono">{rowHex(block.trailer).slice(0, 60)}{block.trailer.length > 20 ? " …" : ""}</span>
          </div>
        )}
      </div>
      {block.groups.slice(0, 3).map((group, index) => (
        <div class="swatch-placeholder" key={index} title={rowHex(group)}>
          <span class="swatch-placeholder-chip" aria-hidden="true" />
          <span class="info-value info-value-mono">Row {index + 1}: {rowHex(group).slice(0, 48)}{group.length > 16 ? " …" : ""}</span>
        </div>
      ))}
    </>
  );
}

export function DeviceKeyboardRgbTab({ info, status }: Props) {
  const rgb = status.rgb ?? null;
  const [pending, setPending] = useState(false);

  if (!rgb || (!rgb.core && !rgb.colors1 && !rgb.colors2 && !rgb.layer)) {
    return (
      <div class="info-section">
        <span class="info-section-title">RGB</span>
        <p class="game-profile-hint">RGB blocks (core 0x32, colours 0x23/0x24, layer 0x39) are silent on this slot — rows appear when the board answers.</p>
      </div>
    );
  }

  // RAM-only full-board buffer push (report index 5). applied:false means
  // the board kept its profile RGB (refresh 0x1D answers 0x66) — shown as
  // "not displayed", never as a painted preview.
  async function pushTestBuffer() {
    setPending(true);
    try {
      const colors = new Uint8Array(6 * 21 * 2).fill(0x7f);
      const { applied } = await setWootingRgbBuffer(info, colors);
      showToast(
        applied ? "Test buffer pushed (RAM — profile RGB unchanged)." : "Board kept its profile RGB — push not displayed.",
        applied ? "success" : "info",
      );
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  // RAM-only single-key set + resets (0x1E/0x1F/0x20). No confirm needed.
  async function paintFirstKey() {
    setPending(true);
    try {
      await setWootingKeyColor(info, 0, 255, 0, 0);
      showToast("Key 1 set to red (RAM).", "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  async function resetAll() {
    setPending(true);
    try {
      await resetWootingRgb(info);
      showToast("RGB reset (RAM).", "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  // FLASH save (0x08): blocking modal first, then confirmed:true.
  async function saveRgb() {
    if (!rgb) return;
    const slot = rgb.slot;
    const confirmed = await confirmFlashOverwrite(slot, "RGB profile");
    if (!confirmed) return;
    setPending(true);
    try {
      await saveWootingProfile(info, WOOTING_COMMAND.saveRgbProfile, slot, true);
      showToast(`RGB profile saved to flash slot ${slot + 1}.`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <div class="info-section">
      <span class="info-section-title">RGB (slot {rgb.slot + 1})</span>
      <RgbBlockSection title="Core" block={rgb.core} />
      <RgbBlockSection title="Colours page 1" block={rgb.colors1} />
      <RgbBlockSection title="Colours page 2" block={rgb.colors2} />
      <RgbBlockSection title="Layer" block={rgb.layer} />
      <div class="setting-row">
        <button class="rescan-button" disabled={pending} onClick={() => void pushTestBuffer()}>Push test buffer (RAM)</button>
        <span class="setting-description">applied:false → "board kept its profile RGB", never a painted preview.</span>
      </div>
      <div class="setting-row">
        <button class="rescan-button" disabled={pending} onClick={() => void paintFirstKey()}>Set key 1 red (RAM)</button>
        <button class="rescan-button" disabled={pending} onClick={() => void resetAll()}>Reset all (RAM)</button>
      </div>
      <div class="setting-row">
        <button class="rescan-button" disabled={pending} onClick={() => void saveRgb()}>Save RGB to flash</button>
        <span class="setting-description">Overwrites onboard flash — blocking confirm first.</span>
      </div>
    </div>
  );
}
