import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "preact/hooks";

export interface ConflictingApp {
  process: string;
  label: string;
}

const CHECK_INTERVAL_MS = 3_000;

export function useConflictingApps() {
  const [apps, setApps] = useState<ConflictingApp[]>([]);

  const check = useCallback(async () => {
    try {
      const result = await invoke<ConflictingApp[]>("detect_conflicting_apps");
      setApps(result);
    } catch {
      // Non-critical — ignore errors
    }
  }, []);

  /** User clicked "I closed it" — re-check immediately. */
  const dismiss = useCallback(() => {
    void check();
  }, [check]);

  useEffect(() => {
    void check();
    const interval = setInterval(check, CHECK_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [check]);

  return { apps, dismiss, recheck: check };
}
