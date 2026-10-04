// Wooting Remap tab: key-mapping rows for one slot.
//
// No mapping flash-save exists on this firmware (no save_mapping in the
// command table) — rows are read-only diagnosis, no Apply invented.
import { rowHex, type KeyboardTabProps } from "./DeviceKeyboardShared";

function MappingSection({ title, groups }: { title: string; groups: Uint8Array[] | null | undefined }) {
  if (!groups) return null;
  return (
    <>
      <span class="info-section-title">{title}</span>
      <div class="info-grid">
        <div class="info-row">
          <span class="info-label">Rows</span>
          <span class="info-value">{groups.length} × {groups[0]?.length ?? 0} bytes</span>
        </div>
      </div>
      {groups.slice(0, 6).map((group, index) => (
        <div class="info-grid" key={index}>
          <div class="info-row">
            <span class="info-label">Row {index + 1}</span>
            <span class="info-value info-value-mono">{rowHex(group)}</span>
          </div>
        </div>
      ))}
      {groups.length > 6 && <p class="game-profile-hint">+{groups.length - 6} more rows kept in status.</p>}
    </>
  );
}

export function DeviceKeyboardRemapTab({ status }: KeyboardTabProps) {
  const mappings = status.mappings ?? null;

  if (!mappings || (!mappings.mapping && !mappings.main && !mappings.function)) {
    return (
      <div class="info-section">
        <span class="info-section-title">Remap</span>
        <p class="game-profile-hint">Mapping blobs (0x30 generic, 0x11 main, 0x12 function) are silent on this slot — rows appear when the board answers.</p>
      </div>
    );
  }

  return (
    <div class="info-section">
      <span class="info-section-title">Remap (slot {mappings.slot + 1})</span>
      <MappingSection title="Key map" groups={mappings.mapping?.groups} />
      <MappingSection title="Main layer (Fn off)" groups={mappings.main?.groups} />
      <MappingSection title="Function layer (Fn on)" groups={mappings.function?.groups} />
      <p class="game-profile-hint">Mapping blobs are read-only on this firmware — no save command exists, so no Apply is offered.</p>
    </div>
  );
}
