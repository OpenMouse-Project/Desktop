import { AlertTriangle } from "lucide-preact";
import type { ConflictingApp } from "../hooks/use-conflicting-apps";

interface Props {
  apps: ConflictingApp[];
}

export function ConflictingAppsBanner({ apps }: Props) {
  if (apps.length === 0) return null;

  const names = [...new Set(apps.map((a) => a.label))];
  const list =
    names.length === 1
      ? names[0]
      : names.length === 2
        ? `${names[0]} and ${names[1]}`
        : `${names.slice(0, -1).join(", ")}, and ${names[names.length - 1]}`;

  return (
    <div class="conflicting-apps-banner">
      <AlertTriangle size={16} class="conflicting-apps-icon" />
      <span class="conflicting-apps-text">
        <strong>{list}</strong> {names.length === 1 ? "is" : "are"} running and may block device access. Quit {names.length === 1 ? "it" : "them"} and their background services, then refresh.
      </span>
    </div>
  );
}
