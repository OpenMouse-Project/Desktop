// Wooting Advanced Keys tab (Wootility clone): LAYERS switcher + Add
// Advanced Key list + Active Advanced Keys n/40.
//
// Layers: Main / Fn Layer 1 / Fn Layer 2. Fn layers read from
// mappings.main / mappings.function - silent on this firmware means "not
// reported by board", never an empty layer claim. The Add list stages
// entries only - no per-key bind encoder exists, so Add toasts the capture
// TODO. Active rows decode from akc.combos (live: A+D socd Last Input
// Priority, layer 0): type, primary/secondary matrix keys, mode, layer.

import { useState } from "preact/hooks";
import type { KeyboardStatus } from "@openmouse/keyboard-protocol/drivers/keyboard-types";
import { wootingMatrixKeyId, wootingSocdModeLabel, type WootingAkcCombo } from "@openmouse/keyboard-protocol/wooting";
import { showToast } from "../lib/toast";
import { DeviceKeyboardMap, type KeySelection } from "./DeviceKeyboardMap";

type Layer = "main" | "fn1" | "fn2";

interface Props {
  status: KeyboardStatus;
}

const ADVANCED_KEY_TYPES: readonly { id: string; title: string; description: string; socd: boolean }[] = [
  {
    id: "rappy",
    title: "Rappy Snappy",
    description: "Rappy Snappy monitors the 2 selected keys and activates whichever key is pressed down further.",
    socd: true,
  },
  {
    id: "snappy",
    title: "Snappy Tappy (SOCD)",
    description: "Snappy Tappy (SOCD Cleaning) monitors the 2 selected keys and activates them based on your chosen settings.",
    socd: true,
  },
  {
    id: "dks",
    title: "Dynamic Keystroke (DKS)",
    description: "4 different actions on a single key based on key position. Activate 1 up to 4 bindings on 4 different parts of the key press.",
    socd: false,
  },
  {
    id: "modtap",
    title: "Mod Tap",
    description: "2 different actions on a single key based on press behaviour — tap for one, hold for the other.",
    socd: false,
  },
];

/** Display label per AKC kind; unknown kinds never invent a name. */
function akcKindLabel(kind: WootingAkcCombo["kind"]): string {
  switch (kind) {
    case "socd": return "Snappy Tappy (SOCD)";
    case "rappySnappy": return "Rappy Snappy";
    case "modTap": return "Mod Tap";
    case "toggleKey": return "Toggle Key";
    case "dks": return "DKS";
    default: return "Unknown";
  }
}

// One active combo row. Rappy/SOCD show primary + secondary - mode label -
// layer; DKS/ModTap/Toggle show primary - layer plus a bound-sample hint
// (their sub-schemas are unconfirmed — this board binds none). Null keys
// render ?; modes 0-2 render via wootingSocdModeLabel as-is (Mode N).
// Every Rappy/SOCD row keeps the Banned in CS2 line.
function AkcRow({ combo, onSelect }: { combo: WootingAkcCombo; onSelect: (combo: WootingAkcCombo) => void }) {
  const isRappySocd = combo.kind === "socd" || combo.kind === "rappySnappy";
  const keyName = (position: { row: number; col: number } | null) => {
    if (!position) return "?";
    return wootingMatrixKeyId(position.row, position.col)?.toUpperCase() ?? `(${position.row},${position.col})`;
  };
  return (
    <button type="button" class="akc-card" onClick={() => onSelect(combo)} title="Show this combo's keys on the map">
      <span class="akc-card-head">
        <span class="akc-card-kind">{akcKindLabel(combo.kind)}</span>
        <span class="akc-card-keys">
          {keyName(combo.key)}{isRappySocd ? ` + ${keyName(combo.secondaryKey)}` : ""}
        </span>
      </span>
      <span class="akc-card-meta">
        {combo.socdMode !== null ? wootingSocdModeLabel(combo.socdMode) : ""}{combo.socdMode !== null ? " · " : ""}Layer {combo.layer}{combo.inputBothWhenBottomedOut !== null ? ` · ${combo.inputBothWhenBottomedOut ? "both stay active when bottomed out" : "winner only when bottomed out"}` : ""}
      </span>
      {isRappySocd && (
        <span class="akc-card-warn">Banned in CS2 — SOCD cleaning modes are not tournament-legal.</span>
      )}
    </button>
  );
}

export function DeviceKeyboardAdvancedKeysTab({ status }: Props) {
  // Bound AKC keys start selected: the map opens showing the keys the live
  // bindings actually cover. Matrix (row,col) is layout order, so ids
  // resolve through the same layout the map renders. Clicking a combo row
  // below re-selects just that combo's keys.
  const [selection, setSelection] = useState<KeySelection>(() => ({
    selected: new Set(
      (status.akc?.combos ?? []).flatMap((combo) =>
        [combo.key, combo.secondaryKey]
          .filter((position) => position !== null)
          .map((position) => wootingMatrixKeyId(position!.row, position!.col))
          .filter((id): id is string => typeof id === "string"),
      ),
    ),
    lastSelected: null,
  }));
  // Clicking a combo row selects just that combo's keys on the map.
  function selectCombo(combo: WootingAkcCombo) {
    const ids = [combo.key, combo.secondaryKey]
      .filter((position) => position !== null)
      .map((position) => wootingMatrixKeyId(position!.row, position!.col))
      .filter((id): id is string => typeof id === "string");
    setSelection({ selected: new Set(ids), lastSelected: ids[0] ?? null });
  }
  const [pending, setPending] = useState(false);
  const [layer, setLayer] = useState<Layer>("main");
  const [stagedTypes, setStagedTypes] = useState<string[]>([]);
  const akc = status.akc ?? null;
  const activeCount = akc?.combos.length ?? 0;
  const layerNote = layer === "main"
    ? status.mappings?.mapping
      ? `${status.mappings.mapping.groups.length} rows`
      : "Main layer rows not reported by board"
    : layer === "fn1"
      ? status.mappings?.main
        ? `${status.mappings.main.groups.length} rows`
        : "Fn Layer 1 not reported by board"
      : status.mappings?.function
        ? `${status.mappings.function.groups.length} rows`
        : "Fn Layer 2 not reported by board";

  function stageType(id: string) {
    if (selection.selected.size === 0) {
      showToast("Select one or more keys on the map first.", "info");
      return;
    }
    setPending(true);
    try {
      // TODO(woot-per-key-write): no per-key bind encoder exists.
      setStagedTypes((prev) => (prev.includes(id) ? prev : [...prev, id]));
      showToast("Per-key write needs USB capture — advanced key staged only.", "info");
    } finally {
      setPending(false);
    }
  }

  return (
    <div>
      <div class="setting-row">
        <span class="setting-description">LAYERS</span>
        <div class="performance-chip-group">
          {(["main", "fn1", "fn2"] as const).map((value) => (
            <button
              key={value}
              class={`performance-chip ${layer === value ? "active" : ""}`}
              onClick={() => setLayer(value)}
            >
              {value === "main" ? "Main Layer" : value === "fn1" ? "Fn Layer 1" : "Fn Layer 2"}
            </button>
          ))}
        </div>
        <span class="setting-description">{layerNote}</span>
      </div>
      <DeviceKeyboardMap
        badgeMm={status.analogProfile?.actuationMm ?? null}
        selection={selection}
        onChange={setSelection}
        emptyHint="TO ADD AN ADVANCED KEY, PLEASE SELECT ONE OR MORE KEYS FIRST"
        selectedHint={(n) => `${n} KEYS SELECTED`}
        highlight={akc?.combos.flatMap((combo, index) => {
          const marks: { row: number; col: number; label: string }[] = [];
          if (combo.key) marks.push({ ...combo.key, label: `P${index + 1}` });
          if (combo.secondaryKey) marks.push({ ...combo.secondaryKey, label: `S${index + 1}` });
          return marks;
        }) ?? []}
      />
      {akc && akc.combos.length > 0 && (
        <p class="game-profile-hint">
          Matrix legend: Pn = combo N primary, Sn = combo N secondary — (row,col) in layout order (0,0 = Esc).
        </p>
      )}
      <div class="kb-feedback">
        <div class="kb-card">
          <span class="kb-card-title">Add Advanced Key</span>
          <p class="game-profile-hint">
            To create a binding, first select a type of advanced key from the list below. Then follow further instructions.
          </p>
          {ADVANCED_KEY_TYPES.map((type) => (
            <div class="kb-card" key={type.id}>
              <span class="kb-card-title">
                {type.title}
                <button
                  class="rescan-button"
                  disabled={pending}
                  onClick={() => stageType(type.id)}
                >
                  {stagedTypes.includes(type.id) ? "Staged ✓" : "Add"}
                </button>
              </span>
              <p class="game-profile-hint">{type.description}</p>
              {type.socd && (
                <p class="game-profile-hint">Banned in CS2 - SOCD cleaning modes are not tournament-legal.</p>
              )}
              {type.id === "snappy" && (
                <p class="game-profile-hint">Live board mode decodes via wootingSocdModeLabel — modes 0-2 render as-is (Mode N).</p>
              )}
            </div>
          ))}
        </div>
        <div class="kb-card">
          <span class="kb-card-title">
            Active Advanced Keys <span class="kb-card-count">{activeCount} / 40</span>
          </span>
          {akc && akc.combos.length > 0 ? (
            <>
              <p class="game-profile-hint">Select an Advanced Key to edit - MAIN LAYER</p>
              {akc.combos.slice(0, 8).map((combo, index) => (
                <AkcRow key={index} combo={combo} onSelect={selectCombo} />
              ))}
            </>
          ) : (
            <p class="game-profile-hint">No active advanced keys on this slot.</p>
          )}
          {stagedTypes.length > 0 && (
            <p class="game-profile-hint">Staged this session: {stagedTypes.join(", ")} - capture TODO on Apply.</p>
          )}
        </div>
      </div>
    </div>
  );
}
