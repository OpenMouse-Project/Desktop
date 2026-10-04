// Shared 61-key 60% grid for the Wooting tabs (Wootility clone).
//
// Click toggles one key, drag paints a run, shift-click extends from the
// last-selected anchor. Selected badges follow the active theme accent
// (--accent/--accent-ink, so Dynamic wallpaper tint applies); the
// last-selected key gets the yellow ring (D in the Rapid Trigger shot).
// Every badge shows the profile default mm — per-key overrides are NOT
// readable on ARM (0x31/0x36 empty-OK), so variance is never invented.

import { useRef, useState } from "preact/hooks";
import { wootingMatrixKeyId } from "@openmouse/keyboard-protocol/wooting";

export interface KeyDef {
  id: string;
  label: string;
  w: number;
}

/** 61-key 60% layout in row order; `w` is relative width units. */
export const KEYBOARD_60_LAYOUT: readonly (readonly KeyDef[])[] = [
  [
    { id: "esc", label: "Esc", w: 1 }, { id: "1", label: "1", w: 1 },
    { id: "2", label: "2", w: 1 }, { id: "3", label: "3", w: 1 },
    { id: "4", label: "4", w: 1 }, { id: "5", label: "5", w: 1 },
    { id: "6", label: "6", w: 1 }, { id: "7", label: "7", w: 1 },
    { id: "8", label: "8", w: 1 }, { id: "9", label: "9", w: 1 },
    { id: "0", label: "0", w: 1 }, { id: "minus", label: "-", w: 1 },
    { id: "equal", label: "=", w: 1 }, { id: "backspace", label: "Backspace", w: 2 },
  ],
  [
    { id: "tab", label: "Tab", w: 1.5 }, { id: "q", label: "Q", w: 1 },
    { id: "w", label: "W", w: 1 }, { id: "e", label: "E", w: 1 },
    { id: "r", label: "R", w: 1 }, { id: "t", label: "T", w: 1 },
    { id: "y", label: "Y", w: 1 }, { id: "u", label: "U", w: 1 },
    { id: "i", label: "I", w: 1 }, { id: "o", label: "O", w: 1 },
    { id: "p", label: "P", w: 1 }, { id: "lbracket", label: "[", w: 1 },
    { id: "rbracket", label: "]", w: 1 }, { id: "backslash", label: "\\", w: 1.5 },
  ],
  [
    { id: "caps", label: "Caps", w: 1.75 }, { id: "a", label: "A", w: 1 },
    { id: "s", label: "S", w: 1 }, { id: "d", label: "D", w: 1 },
    { id: "f", label: "F", w: 1 }, { id: "g", label: "G", w: 1 },
    { id: "h", label: "H", w: 1 }, { id: "j", label: "J", w: 1 },
    { id: "k", label: "K", w: 1 }, { id: "l", label: "L", w: 1 },
    { id: "semicolon", label: ";", w: 1 }, { id: "quote", label: "'", w: 1 },
    { id: "enter", label: "Enter", w: 2.25 },
  ],
  [
    { id: "lshift", label: "L-Shift", w: 2.25 }, { id: "z", label: "Z", w: 1 },
    { id: "x", label: "X", w: 1 }, { id: "c", label: "C", w: 1 },
    { id: "v", label: "V", w: 1 }, { id: "b", label: "B", w: 1 },
    { id: "n", label: "N", w: 1 }, { id: "m", label: "M", w: 1 },
    { id: "comma", label: ",", w: 1 }, { id: "period", label: ".", w: 1 },
    { id: "slash", label: "/", w: 1 }, { id: "rshift", label: "R-Shift", w: 2.75 },
  ],
  [
    { id: "lctrl", label: "L-Ctrl", w: 1.25 }, { id: "lwin", label: "L-Win", w: 1.25 },
    { id: "lalt", label: "L-Alt", w: 1.25 }, { id: "space", label: "Spacebar", w: 6.25 },
    { id: "ralt", label: "R-Alt", w: 1.25 }, { id: "menu", label: "Menu", w: 1.25 },
    { id: "rctrl", label: "R-Ctrl", w: 1.25 }, { id: "fn1", label: "Fn 1", w: 1.25 },
  ],
];

export const KEYBOARD_60_COUNT = KEYBOARD_60_LAYOUT.reduce((n, row) => n + row.length, 0);

export interface KeySelection {
  selected: Set<string>;
  /** Last-clicked key id — the yellow ring anchor, null when empty. */
  lastSelected: string | null;
}

interface MapProps {
  /** Profile default mm shown on every badge (never per-key variance). */
  badgeMm: number | null;
  selection: KeySelection;
  onChange: (next: KeySelection) => void;
  /** Yellow empty-state line when nothing is selected, like the screenshots. */
  emptyHint: string;
  /** "N KEYS SELECTED" line once something is selected. */
  selectedHint?: (count: number) => string;
  /**
   * Firmware matrix positions to outline (e.g. AKC primary/secondary
   * keys). Matched to rendered keys through wootingMatrixKeyId — the
   * same table selection uses — so a mark lands on the true key even
   * though firmware rows/cols differ from layout order.
   */
  highlight?: readonly { row: number; col: number; label: string }[];
}

export function DeviceKeyboardMap({ badgeMm, selection, onChange, emptyHint, selectedHint, highlight }: MapProps) {
  const [dragging, setDragging] = useState(false);
  const dragAdd = useRef(true);
  const anchor = useRef<string | null>(null);

  function commit(selected: Set<string>, lastSelected: string | null) {
    onChange({ selected, lastSelected });
  }

  function toggle(id: string, extend: boolean) {
    const selected = new Set(selection.selected);
    if (extend && anchor.current) {
      // Shift-click: select the row-span between anchor and clicked key.
      const flat = KEYBOARD_60_LAYOUT.flat();
      const from = flat.findIndex((key) => key.id === anchor.current);
      const to = flat.findIndex((key) => key.id === id);
      if (from >= 0 && to >= 0) {
        const [lo, hi] = from < to ? [from, to] : [to, from];
        for (let i = lo; i <= hi; i += 1) selected.add(flat[i].id);
        commit(selected, id);
        return;
      }
    }
    if (selected.has(id)) selected.delete(id);
    else selected.add(id);
    anchor.current = id;
    commit(selected, selected.has(id) ? id : null);
  }

  function paint(id: string) {
    if (!dragging) return;
    const selected = new Set(selection.selected);
    if (dragAdd.current) selected.add(id);
    else selected.delete(id);
    commit(selected, id);
  }

  const count = selection.selected.size;

  return (
    <div>
      <div
        class="kb-map"
        onMouseLeave={() => setDragging(false)}
        onMouseUp={() => setDragging(false)}
      >
        {KEYBOARD_60_LAYOUT.map((row, rowIndex) => (
          <div class="kb-map-row" key={rowIndex}>
            {row.map((key) => {
              const isSelected = selection.selected.has(key.id);
              const isLast = selection.lastSelected === key.id;
              // Firmware (row,col) → key id through the SDK matrix table
              // (not layout order — firmware row 3 = the A row, but layout
              // row 3 = the Z row). Marks land on the true key.
              const marks = (highlight ?? []).filter((mark) => wootingMatrixKeyId(mark.row, mark.col) === key.id);
              return (
                <button
                  key={key.id}
                  type="button"
                  class={`kb-key${isSelected ? " kb-key-selected" : ""}${isLast ? " kb-key-last" : ""}${marks.length > 0 ? " kb-key-marked" : ""}`}
                  style={{ flexGrow: key.w, flexBasis: 0 }}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    dragAdd.current = !selection.selected.has(key.id);
                    setDragging(true);
                    toggle(key.id, event.shiftKey);
                  }}
                  onMouseEnter={() => paint(key.id)}
                  onFocus={() => undefined}
                  title={marks.length > 0 ? `${key.label} — ${marks.map((mark) => mark.label).join(", ")}` : key.label}
                >
                  <span class="kb-key-badge">{badgeMm !== null ? badgeMm.toFixed(2) : "—"}</span>
                  <span class="kb-key-label">{key.label}</span>
                  {marks.length > 0 && (
                    <span class="kb-key-mark">{marks.map((mark) => mark.label).join(" · ")}</span>
                  )}
                </button>
              );
            })}
          </div>
        ))}
      </div>
      {count === 0 ? (
        <p class="kb-map-hint kb-map-hint-empty">{emptyHint}</p>
      ) : (
        <p class="kb-map-hint">{selectedHint ? selectedHint(count) : `${count} KEYS SELECTED`}</p>
      )}
      <div class="setting-row">
        <button
          class="rescan-button"
          onClick={() => {
            const all = new Set(KEYBOARD_60_LAYOUT.flat().map((key) => key.id));
            anchor.current = null;
            commit(all, null);
          }}
        >
          Select all keys
        </button>
        <button
          class="rescan-button"
          disabled={count === 0}
          onClick={() => {
            anchor.current = null;
            commit(new Set(), null);
          }}
        >
          Discard selection
        </button>
      </div>
    </div>
  );
}
