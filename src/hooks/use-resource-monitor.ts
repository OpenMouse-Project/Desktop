// Owned once by FullDesktopView (same pattern as useMouseConnection) rather
// than by the ResourceMonitor component itself, specifically so the poll
// keeps running — and history/min/avg/max keep accumulating — across page
// navigation. FullDesktopView doesn't unmount when the user switches pages
// (only the page components underneath it route-swap), so a hook owned
// here survives a trip to Overview or Games and back, where one owned by
// ResourceMonitor itself would have reset on every remount.

import { useEffect, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";

export interface ResourceSample {
  cpuPercent: number;
  memoryBytes: number;
  timestampMs: number;
}

// This app's own process, sampled — see src-tauri/src/resource_monitor.rs
// for why a live poll interval (not a one-shot read) is what makes the
// CPU% figure meaningful at all: sysinfo needs two refreshes with real time
// between them to compute a percentage, not just one.
const POLL_INTERVAL_MS = 1500;
// ~90s of trend at the interval above — enough to see a spike and its
// tail-off without the chart turning into an unreadable smear.
const HISTORY_LENGTH = 60;

export interface ResourceAccumulator {
  min: number;
  max: number;
  sum: number;
  count: number;
}

function fold(acc: ResourceAccumulator | null, value: number): ResourceAccumulator {
  if (!acc) return { min: value, max: value, sum: value, count: 1 };
  return { min: Math.min(acc.min, value), max: Math.max(acc.max, value), sum: acc.sum + value, count: acc.count + 1 };
}

export function useResourceMonitor() {
  const [history, setHistory] = useState<ResourceSample[]>([]);
  const [cpuStats, setCpuStats] = useState<ResourceAccumulator | null>(null);
  const [memStats, setMemStats] = useState<ResourceAccumulator | null>(null);
  // sysinfo's cpu_usage() is diff-based (see its own doc comment: "CPU
  // usage is based on diff") — the very first sample for this process has
  // nothing prior to diff against and reads a garbage 0%, not a real
  // measurement. Folding that phantom zero into min/avg/max permanently
  // understated the average and pinned Min at 0% regardless of the app's
  // real idle floor. Memory has no such warm-up — it's an absolute
  // reading, valid from sample one — so only the CPU accumulator skips its
  // first value.
  const firstCpuSampleSeen = useRef(false);

  useEffect(() => {
    let cancelled = false;

    async function poll() {
      // No reason to keep sampling while nothing can even see the result —
      // minimized, hidden to the tray, or occluded. Unlike the HID
      // auto-refresh (use-mouse-connection.ts), this doesn't also pause on
      // plain window-blur: watching a trend over time is exactly the kind
      // of thing worth keeping accurate while the window sits visible but
      // unfocused in the background.
      if (document.hidden) return;
      try {
        const sample = await invoke<ResourceSample | null>("sample_resource_usage");
        if (cancelled || !sample) return;
        setHistory((prev) => [...prev, sample].slice(-HISTORY_LENGTH));
        setMemStats((prev) => fold(prev, sample.memoryBytes));
        if (firstCpuSampleSeen.current) {
          setCpuStats((prev) => fold(prev, sample.cpuPercent));
        } else {
          firstCpuSampleSeen.current = true;
        }
      } catch {
        // Best-effort — a failed sample just leaves the last known point in
        // place rather than resetting the chart.
      }
    }

    void poll();
    const interval = setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  return { history, cpuStats, memStats };
}

export type ResourceMonitorData = ReturnType<typeof useResourceMonitor>;
