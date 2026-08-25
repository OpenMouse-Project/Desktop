import type { ComponentType } from "preact";
import type { LucideProps } from "lucide-preact";
import { Zap } from "lucide-preact";

interface Props {
  icon: ComponentType<LucideProps>;
  label: string;
  value: string;
  /** Small bolt icon next to the value — battery chip, while charging. */
  charging?: boolean;
}

export function StatusChip({ icon: Icon, label, value, charging }: Props) {
  return (
    <div class="status-chip">
      <Icon class="status-chip-icon" size={16} aria-hidden="true" />
      <div class="status-chip-text">
        <span class="status-chip-label">{label}</span>
        <span class="status-chip-value">
          {value}
          {charging && <Zap class="status-chip-charging" size={11} aria-label="Charging" />}
        </span>
      </div>
    </div>
  );
}
