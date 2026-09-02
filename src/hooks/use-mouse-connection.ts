// Owns the one piece of state every device-aware part of the UI needs to
// agree on — the currently connected device, if any. Lifted out of
// OverviewPage (which used to own this alone) so Sidebar and Header can show
// the same connection instead of staying static placeholders next to a page
// that's actually connected.
//
// `connected` and `view` are deliberately separate: `connected` is a cached
// snapshot from the last successful `connectToInterface()` call (a full
// open + resolveDeviceIndex + readStatus() walk — genuinely slow, tens of
// request/response round trips on a feature-rich mouse), and `view` is just
// which page the user is looking at. They used to be the same flag, so
// hitting "Back" threw the snapshot away and looking at the device again
// meant paying that whole walk a second time for data that hadn't changed.
// Now "Back" only changes `view`; the snapshot survives until either a
// different device connects or the user explicitly asks for a fresh read.

import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseStatus } from "@openmouse/protocol/drivers/mouse-types";
import {
  connectToInterface,
  listCandidateInterfaces,
  type CandidateInterface,
  type ConnectedDevice,
} from "../native-hid/scan";
import type { HidInterfaceInfo } from "../native-hid/tauri-hid-device";
import { getRememberedDevice, rememberDevice } from "../native-hid/device-store";
import { isHidBusyError } from "../native-hid/hid-open-lock";
import { showToast } from "../lib/toast";

// How often a connected device's status re-reads itself in the background,
// so battery/DPI/etc. drift on their own instead of only updating after an
// explicit Refresh click. Every tick reuses connect()'s own hid-open-lock
// (native-hid/hid-open-lock.ts) — a tick that lands while a write or a
// manual refresh is still running just silently skips (isHidBusyError)
// rather than piling up or corrupting anything.
const AUTO_REFRESH_INTERVAL_MS = 5000;

interface ConflictingApp {
  process: string;
  label: string;
}

// Vendor app (Razer Synapse, LGHUB, …) is running right now — return the
// conflict message instead of the raw driver error ("device is asleep", the
// garbled reply, etc.). Quitting the vendor app's process AND its background
// services typically fixes it, so say that rather than dumping a cryptic
// string on the connect page.
async function conflictMessageOr(error: unknown): Promise<string> {
  try {
    const apps = await invoke<ConflictingApp[]>("detect_conflicting_apps");
    if (apps.length > 0) return conflictErrorLabel(apps);
  } catch {
    // Fall through — conflict detection itself failing shouldn't suppress
    // the real connect error.
  }
  return error instanceof Error ? error.message : String(error);
}

function conflictErrorLabel(apps: ConflictingApp[]): string {
  const names = [...new Set(apps.map((a) => a.label))];
  const joined =
    names.length === 1
      ? names[0]
      : names.length === 2
        ? `${names[0]} and ${names[1]}`
        : `${names.slice(0, -1).join(", ")}, and ${names[names.length - 1]}`;
  return `Conflicting software detected — ${joined} ${names.length === 1 ? "is" : "are"} blocking access to your device. Please quit ${names.length === 1 ? "its" : "their"} process and close any background service, then reconnect.`;
}

export type CandidateListState =
  | { status: "loading" }
  | { status: "loaded"; candidates: CandidateInterface[] }
  | { status: "error"; message: string };

export function useMouseConnection() {
  const [list, setList] = useState<CandidateListState>({ status: "loading" });
  const [connected, setConnected] = useState<ConnectedDevice | null>(null);
  const [view, setView] = useState<"list" | "device">("list");
  const [connectingKey, setConnectingKey] = useState<string | null>(null);

  // Auto-reconnect is a one-shot courtesy on launch, not something a later
  // manual "Refresh" should repeat — once the user is looking at the device
  // list themselves, respect their own clicks instead of jumping ahead of
  // them.
  const autoReconnectAttempted = useRef(false);
  // connect()/refresh() close over `connected` by ref, not by hook
  // dependency, so their identities stay stable (safe to call from the
  // mount-only effect below and to pass down as props) while still seeing
  // up-to-date state when they actually run.
  const connectedRef = useRef(connected);
  connectedRef.current = connected;
  // The candidate behind the current snapshot, so an explicit refresh can
  // redo the exact same connect without the caller needing to still have it
  // around (the device list may have been re-scanned since).
  const lastCandidateRef = useRef<CandidateInterface | null>(null);
  // A ref, not state — OverviewPage flips this every render while the
  // Performance tab is open, and that shouldn't itself cause a re-render
  // here. CONFIRMED as a real problem, not just a theoretical one: a
  // silent background walk landing mid-edit was visibly "looking up the
  // mouse" (full reconnect) while the user was actively clicking through
  // DPI/rate/lift-off controls — jarring even though the hid-open-lock kept
  // it from corrupting anything. The Performance tab pauses this for as
  // long as it's open.
  const autoRefreshPausedRef = useRef(false);
  const setAutoRefreshPaused = useCallback((paused: boolean) => {
    autoRefreshPausedRef.current = paused;
  }, []);
  // Distinct from document.hidden below: this is true for "window visible
  // but not the focused one" (clicked into another app, this one just
  // sitting in the background) — document.hidden alone doesn't cover that,
  // only minimized/occluded/hidden-to-tray. Tauri's own window-focus event
  // is what actually tracks OS-level focus, not a DOM visibility proxy for
  // it. Assume focused until told otherwise — the event may not have fired
  // yet on first mount.
  const windowFocusedRef = useRef(true);

  const connect = useCallback(async (candidate: CandidateInterface, opts?: { silent?: boolean }) => {
    const key = candidate.info.key;
    // Same underlying call whether this is a first connect or a background/
    // manual re-read of the device already showing — only the toast wording
    // differs, so tell them apart before the read (status.name isn't known
    // for a fresh connect's error case).
    const isRefresh = connectedRef.current?.key === key;
    setConnectingKey(key);
    try {
      const device = await connectToInterface(candidate.info);
      setConnected(device);
      lastCandidateRef.current = candidate;
      rememberDevice(candidate.info, device.brand);
      // Only switch to the device view on a *new* connection — background
      // re-reads of the already-connected device must not yank the user back
      // to the device page when they've navigated to the list.
      if (!isRefresh) setView("device");
      if (!opts?.silent) {
        showToast(isRefresh ? `${device.status.name} refreshed.` : `Connected to ${device.status.name}.`, "success");
      }
    } catch (error) {
      // connectToInterface() enforces the "one walk per device at a time"
      // lock itself now (native-hid/hid-open-lock.ts) — a busy error here
      // means some OTHER call (background auto-refresh racing a manual
      // click, or a dev-server hot-reload remount) is already mid-walk on
      // this same device, not that this one actually failed. Swallow it
      // rather than showing the user an error for something that isn't one.
      if (isHidBusyError(error)) return;
      if (!opts?.silent) {
        const message = await conflictMessageOr(error);
        showToast(message, "error");
      }
    } finally {
      setConnectingKey(null);
    }
  }, []);

  // The interface behind the cached snapshot — what a device-control tab
  // needs to open its own short-lived connection for a write (see
  // native-hid/logitech-actions.ts), without having to re-derive it from
  // `connected.key` alone.
  const connectedInfo: HidInterfaceInfo | null =
    connected && lastCandidateRef.current?.info.key === connected.key
      ? lastCandidateRef.current.info
      : null;

  // A write action (setDpi, setPollingRate, ...) returns the value the
  // device actually applied — patch that straight into the cached status
  // instead of re-running the whole readStatus() walk just to see it.
  const patchStatus = useCallback((patch: Partial<MouseStatus>) => {
    setConnected((prev) => (prev ? { ...prev, status: { ...prev.status, ...patch } } : prev));
  }, []);

  // What the device list's "Connect" click calls: if this is the device
  // already cached, just switch views — no reason to pay the walk again for
  // data that's already sitting there. A different device still connects
  // for real.
  const select = useCallback((candidate: CandidateInterface) => {
    if (connectedRef.current?.key === candidate.info.key) {
      setView("device");
      return;
    }
    void connect(candidate);
  }, [connect]);

  // What the device card's own refresh button calls: re-run the full walk
  // for the CURRENTLY cached device on purpose (e.g. DPI was changed from
  // somewhere else since the last read).
  const refreshCurrent = useCallback(() => {
    if (lastCandidateRef.current) void connect(lastCandidateRef.current);
  }, [connect]);

  const refresh = useCallback(async () => {
    setList({ status: "loading" });
    try {
      const candidates = await listCandidateInterfaces();
      setList({ status: "loaded", candidates });
      if (!autoReconnectAttempted.current && !connectedRef.current) {
        autoReconnectAttempted.current = true;
        const remembered = getRememberedDevice();
        const match = remembered && candidates.find((c) => c.info.key === remembered.key);
        // Silent: a remembered device that's simply not plugged in yet
        // shouldn't greet the user with an error toast on every launch.
        if (match) void connect(match, { silent: true });
      }
    } catch (error) {
      setList({ status: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }, [connect]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Tracks OS-level window focus for the auto-refresh gate below. A plain
  // event subscription, not state — nothing here needs to re-render on
  // focus change, the interval callback just reads the ref when it fires.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        windowFocusedRef.current = focused;
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Background auto-refresh — keyed on connected?.key, not `connected`
  // itself, so patchStatus()'s own updates (a new object each time) don't
  // tear down and restart this interval on every write.
  useEffect(() => {
    if (!connected) return;
    const interval = setInterval(() => {
      if (autoRefreshPausedRef.current) return;
      // A full readStatus() walk (5 splits opened, 20-30 HID++ round
      // trips) every 5s adds up if it keeps running while nobody can even
      // see the result — minimized, hidden to the tray, occluded, or just
      // not the focused window right now. Skip the tick entirely rather
      // than spend that on a window nobody's actively looking at.
      if (document.hidden || !windowFocusedRef.current) return;
      if (lastCandidateRef.current) void connect(lastCandidateRef.current, { silent: true });
    }, AUTO_REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected?.key, connect]);

  // Just switches back to the list — the snapshot stays cached (see module
  // docs above). Re-scans in the background so a newly plugged-in device
  // shows up, same as a manual Refresh would.
  function back() {
    setView("list");
    void refresh();
  }

  // For Sidebar's connected-device chip: jump straight to the cached
  // device's card from wherever the user currently is (another page, or the
  // device list) — no reconnect, same as clicking a matching row in the
  // list.
  function viewConnectedDevice() {
    if (connectedRef.current) setView("device");
  }

  return {
    list,
    connected,
    connectedInfo,
    view,
    connectingKey,
    select,
    refreshCurrent,
    refresh,
    back,
    viewConnectedDevice,
    patchStatus,
    setAutoRefreshPaused,
  };
}

export type MouseConnection = ReturnType<typeof useMouseConnection>;
