type AppMode = "bridge" | "full-desktop";

interface Props {
  mode: AppMode;
  compact?: boolean;
  onChange: (mode: AppMode) => void;
}

export function ModeToggle({ mode, compact, onChange }: Props) {
  return (
    <div class={`mode-toggle ${compact ? "compact" : ""}`}>
      <button
        class={mode === "bridge" ? "active" : ""}
        onClick={() => onChange("bridge")}
        title="Bridge Mode"
      >
        Bridge
      </button>
      <button
        class={mode === "full-desktop" ? "active" : ""}
        onClick={() => onChange("full-desktop")}
        title="Full Desktop Mode"
      >
        {compact ? "Desktop" : "Full Desktop"}
      </button>
    </div>
  );
}
