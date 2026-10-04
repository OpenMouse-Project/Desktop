// Shared building blocks for the Wooting keyboard tabs.
//
// The keyboard driver exposes readStatus + read* helpers plus five write
// methods (switchProfile RAM switch, saveProfile confirm-gated flash saves,
// setSingleKeyColor / resetRgb RAM RGB, setRgbBuffer RAM push with an
// applied:false path). Every flash save goes through confirmFlashOverwrite:
// the modal names the exact slot, and the answer is passed as confirmed.

import type { KeyboardStatus } from "@openmouse/keyboard-protocol/drivers/keyboard-types";

/** Props every keyboard tab receives: live status plus staged-value persistence. */
export interface KeyboardTabProps {
  status: KeyboardStatus;
  onApplied: (patch: Partial<KeyboardStatus>) => void;
}

/** First `count` bytes as spaced hex with an ellipsis when truncated. */
export function hexPreview(bytes: Uint8Array, count = 20): string {
  const shown = [...bytes.subarray(0, count)].map((byte) => byte.toString(16).padStart(2, "0")).join(" ");
  return bytes.length > count ? `${shown} …` : shown;
}

/** Verbatim bytes of one opaque row group as spaced hex. */
export function rowHex(row: Uint8Array): string {
  return [...row].map((byte) => byte.toString(16).padStart(2, "0")).join(" ");
}

/**
 * Blocking confirm for a flash-persisting action. The modal names the exact
 * slot being overwritten; confirming resolves true, cancelling or
 * dismissing resolves false. The caller passes the result as
 * `confirmed` — the codec encoder throws without confirmed: true, so an
 * unconfirmed call fails before any HID send.
 */
export function confirmFlashOverwrite(slot: number, what: string): Promise<boolean> {
  // Executor form: this project's TS lib predates ES2024, so
  // `Promise.withResolvers` does not type-check here (same reason
  // use-mouse-connection.ts's delay() uses the executor).
  return new Promise<boolean>((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "flash-confirm-overlay";
    const slotLabel = `slot ${slot + 1}`;
    const dialog = document.createElement("div");
    dialog.className = "flash-confirm-dialog";
    dialog.setAttribute("role", "alertdialog");
    const title = document.createElement("h2");
    title.textContent = `Overwrite onboard flash ${slotLabel}?`;
    const body = document.createElement("p");
    body.textContent = `This permanently overwrites ${what} in ${slotLabel} on the board. This cannot be undone from OpenMouse.`;
    const actions = document.createElement("div");
    actions.className = "flash-confirm-actions";
    const cancel = document.createElement("button");
    cancel.className = "rescan-button";
    cancel.textContent = "Cancel";
    const confirm = document.createElement("button");
    confirm.className = "rescan-button rescan-button-danger";
    confirm.textContent = `Overwrite ${slotLabel}`;
    const done = (value: boolean) => {
      overlay.remove();
      resolve(value);
    };
    cancel.onclick = () => done(false);
    confirm.onclick = () => done(true);
    overlay.onclick = (event) => {
      if (event.target === overlay) done(false);
    };
    actions.append(cancel, confirm);
    dialog.append(title, body, actions);
    overlay.append(dialog);
    document.body.append(overlay);
    confirm.focus();
  });
}

/** Curve preset selector (0x27 nested field 6) as a display label, never a claim. */
export function curvePresetLabel(preset: number | null | undefined): string | null {
  if (preset === null || preset === undefined) return null;
  return `Curve preset ${preset}`;
}
