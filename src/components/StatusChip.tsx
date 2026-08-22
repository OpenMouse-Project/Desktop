import type { ComponentType } from "preact";
import type { LucideProps } from "lucide-preact";

interface Props {
  icon: ComponentType<LucideProps>;
  label: string;
  value: string;
}

export function StatusChip({ icon: Icon, label, value }: Props) {
  return (
    <div class="status-chip">
      <Icon class="status-chip-icon" size={16} aria-hidden="true" />
      <div class="status-chip-text">
        <span class="status-chip-label">{label}</span>
        <span class="status-chip-value">{value}</span>
      </div>
    </div>
  );
}
