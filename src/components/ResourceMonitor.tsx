import { Cpu, MemoryStick } from "lucide-preact";
import type { ResourceMonitorData } from "../hooks/use-resource-monitor";

function bytesToMb(bytes: number): number {
  return bytes / (1024 * 1024);
}

/** Minimal dependency-free line-chart — just an SVG polyline scaled to `max`. */
function Sparkline({ values, max }: { values: number[]; max: number }) {
  if (values.length < 2) {
    return <svg class="sparkline" viewBox="0 0 100 32" preserveAspectRatio="none" aria-hidden="true" />;
  }
  const points = values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * 100;
      const y = max > 0 ? 31 - (value / max) * 30 : 31;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
  return (
    <svg class="sparkline" viewBox="0 0 100 32" preserveAspectRatio="none" aria-hidden="true">
      <polyline points={points} class="sparkline-line" />
    </svg>
  );
}

interface Props {
  data: ResourceMonitorData;
}

/**
 * Live CPU%/RAM for this app's own process — Settings' "for now" resource
 * panel while there's no dedicated diagnostics surface yet. Purely
 * presentational: the actual polling/accumulation lives in
 * useResourceMonitor(), owned once by FullDesktopView so it keeps running
 * (and history/stats keep accumulating) across page navigation instead of
 * resetting every time this component remounts.
 */
export function ResourceMonitor({ data }: Props) {
  const { history, cpuStats, memStats } = data;
  const latest = history[history.length - 1];
  const cpuValues = history.map((sample) => sample.cpuPercent);
  const memValues = history.map((sample) => bytesToMb(sample.memoryBytes));
  // A little headroom above the session max so the line doesn't ride the
  // very top edge of the chart at a steady value.
  const cpuChartMax = Math.max(10, (cpuStats?.max ?? 0) * 1.15);
  const memChartMax = Math.max(10, (memStats ? bytesToMb(memStats.max) : 0) * 1.15);
  const cpuAvg = cpuStats ? cpuStats.sum / cpuStats.count : null;
  const memAvg = memStats ? memStats.sum / memStats.count : null;

  return (
    <div class="resource-monitor">
      <div class="resource-monitor-card">
        <div class="resource-monitor-header">
          <span class="resource-monitor-title">
            <Cpu size={13} aria-hidden="true" /> CPU
          </span>
          <span class="resource-monitor-current">{latest ? `${latest.cpuPercent.toFixed(1)}%` : "—"}</span>
        </div>
        <Sparkline values={cpuValues} max={cpuChartMax} />
        <div class="resource-monitor-stats">
          <span>Min {cpuStats ? `${cpuStats.min.toFixed(1)}%` : "—"}</span>
          <span>Avg {cpuAvg !== null ? `${cpuAvg.toFixed(1)}%` : "—"}</span>
          <span>Max {cpuStats ? `${cpuStats.max.toFixed(1)}%` : "—"}</span>
        </div>
      </div>

      <div class="resource-monitor-card">
        <div class="resource-monitor-header">
          <span class="resource-monitor-title">
            <MemoryStick size={13} aria-hidden="true" /> RAM
          </span>
          <span class="resource-monitor-current">{latest ? `${bytesToMb(latest.memoryBytes).toFixed(0)} MB` : "—"}</span>
        </div>
        <Sparkline values={memValues} max={memChartMax} />
        <div class="resource-monitor-stats">
          <span>Min {memStats ? `${bytesToMb(memStats.min).toFixed(0)} MB` : "—"}</span>
          <span>Avg {memAvg !== null ? `${bytesToMb(memAvg).toFixed(0)} MB` : "—"}</span>
          <span>Max {memStats ? `${bytesToMb(memStats.max).toFixed(0)} MB` : "—"}</span>
        </div>
      </div>
    </div>
  );
}
