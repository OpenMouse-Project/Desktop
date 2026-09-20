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
import { getRememberedDevice, rememberDevice, rememberDeviceName } from "../native-hid/device-store";
import { isHidBusyError, isQueuedNotProcessedError } from "../native-hid/hid-open-lock";
import { dismissToast, showProgressToast, showToast, updateToast } from "../lib/toast";
import { pushStreamOverlayStatus, subscribeStreamOverlayDeviceKey } from "../lib/stream-overlay";

// How often a connected device's status re-reads itself in the background,
// so battery/DPI/etc. drift on their own instead of only updating after an
// explicit Refresh click. Every tick reuses connect()'s own hid-open-lock
// (native-hid/hid-open-lock.ts) — a tick that lands while a write or a
// manual refresh is still running just silently skips (isHidBusyError)
// rather than piling up or corrupting anything.
const AUTO_REFRESH_INTERVAL_MS = 5000;
// Slower cadences for the states where the 5 s walk is more than the
// situation is worth, chosen so the status never stops moving altogether.
// Before these existed the refresh simply *skipped* whenever the window was
// unfocused or the Performance tab was open, and the practical result (a real
// user report) was a battery figure and a Wired/Wireless label that only
// changed after quitting and relaunching the app. The window is very often
// visible while another app has focus, and the Performance tab is where
// people actually sit.
//
// - Visible but not the focused window: someone may well be glancing at it.
// - Performance tab open: a landing read used to look like a reconnect while
//   the user was mid-edit, so it stays gentle. The tab's staged values only
//   reset when the device value actually changes (DevicePerformanceTab.tsx),
//   so a read that comes back unchanged costs nothing on screen.
// - Hidden (minimized / tray): only the tray menu shows anything (tray.rs).
const UNFOCUSED_REFRESH_INTERVAL_MS = 15_000;
const EDITING_REFRESH_INTERVAL_MS = 30_000;
const HIDDEN_REFRESH_INTERVAL_MS = 60_000;
// How often to re-enumerate HID interfaces while something is connected. A
// plain hidapi device-list refresh, nothing is opened, so it is cheap enough
// to run this often. It is what notices the cable going in or out: the mouse
// enumerates as a *different* product id over USB than over its receiver, so
// "wireless -> wired" is not a status change on the connected interface, it
// is a new interface appearing and the old one going quiet.
const TRANSPORT_SCAN_INTERVAL_MS = 3000;

interface ConflictingApp {
  process: string;
  label: string;
}

// Vendor app (Razer Synapse, LGHUB, …) is running right now — return the
// conflict message instead of the raw driver error ("device is asleep", the
// garbled reply, etc.). Quitting the vendor app's process AND its background
// services typically fixes it, so say that rather than dumping a cryptic
// string on the connect page.
/** Waits between manual-connect attempts — see connectWithBusyRetry. */
function delay(ms: number): Promise<void> {
  // Executor form: this project's TS lib predates ES2024, so
  // `Promise.withResolvers` does not type-check here.
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

// A click must not lose to a background scan, nor to a mouse that has not
// woken up yet. Three transient states land here:
//   - native-hid's walk lock: another JS walk for this device is in flight.
//   - hid.rs's `with_hid_api`: an enumeration is already running on the main
//     thread, which refuses rather than blocking (that blocking lock
//     self-deadlocked the main thread and froze the whole window).
//   - the mouse echoing a command back unprocessed — `returned status 0x00`,
//     see isQueuedNotProcessedError. CONFIRMED in the on-disk log on a
//     wireless Razer: the status walk's own firmware getter came back that way
//     for every candidate, and the same connect succeeded moments later, which
//     is exactly the "click Connect until it takes" this app was reported for.
// A manual connect waits these out. The budget is deliberately long enough to
// ride out a flaky link: CONFIRMED with a Razer receiver that answers a command
// with status 0x02 (processed) in bursts and echoes 0x00 in between — a 5-second
// window mostly missed those bursts, while the progress toast below means the
// user can see the attempt is still running rather than clicking again.
// Repeating callers (the 5 s auto-refresh, the 3 s transport scan, the
// awaiting-return reconnect) keep a single attempt because they retry on their
// own timers — except the one-shot launch reconnect, which passes `retry`
// explicitly.
const MANUAL_CONNECT_ATTEMPTS = 60;
const MANUAL_CONNECT_DELAY_MS = 500;

async function connectWithRetry(
  info: HidInterfaceInfo,
  opts?: { silent?: boolean; silentErrors?: boolean; retry?: boolean },
): Promise<ConnectedDevice> {
  const attempts = opts?.retry || !(opts?.silent || opts?.silentErrors) ? MANUAL_CONNECT_ATTEMPTS : 1;
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await connectToInterface(info);
    } catch (error) {
      if (attempt >= attempts - 1) throw error;
      const busy = isHidBusyError(error);
      if (!busy && !isQueuedNotProcessedError(error)) throw error;
      void invoke("log_line", {
        line: `[connect] ${info.key}: retrying after ${busy ? "busy" : "queued command"} (${attempt + 1}/${attempts})`,
      }).catch(() => {});
      await delay(MANUAL_CONNECT_DELAY_MS);
    }
  }
}

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

/**
 * Whether two interfaces are plausibly the same physical mouse reached over
 * different transports (cable vs. receiver). Vendors give the two separate
 * product ids but the same product string, sometimes with a transport word
 * bolted on ("(Wired)", "Wireless", "Dongle"), so compare on the name with
 * those stripped. This is only the *pre-filter*: the status read that follows
 * confirms with the unit id / serial where the driver reports one.
 */
export function isSiblingTransport(
  a: Pick<HidInterfaceInfo, "key" | "vendorId" | "productString">,
  b: Pick<HidInterfaceInfo, "key" | "vendorId" | "productString">,
): boolean {
  if (a.key === b.key || a.vendorId !== b.vendorId) return false;
  const norm = (name: string) =>
    name
      .toLowerCase()
      .replace(/\b(wired|wireless|dongle|receiver|hyperspeed|lightspeed|2\.4g(?:hz)?|usb)\b/g, "")
      .replace(/[()\-_]/g, " ")
      .replace(/\s+/g, " ")
      .trim();
  const na = norm(a.productString);
  const nb = norm(b.productString);
  return na.length > 0 && na === nb;
}

export function useMouseConnection() {
  const [list, setList] = useState<CandidateListState>({ status: "loading" });
  const [connected, setConnected] = useState<ConnectedDevice | null>(null);
  const [view, setView] = useState<"list" | "device">("list");
  const [connectingKey, setConnectingKey] = useState<string | null>(null);
  // The device whose last *user-initiated* connect failed, which is what the
  // tile's red light reports until the next attempt. Background callers (the
  // 5 s refresh, the 3 s transport scan) retry on their own timers and must
  // not leave a device looking broken.
  const [failedKey, setFailedKey] = useState<string | null>(null);

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
  // When the last successful walk finished, so the hidden-window cadence
  // below can be measured from real reads rather than from interval ticks.
  const lastReadAtRef = useRef(0);
  // Interface keys seen by the last transport scan, so a scan can tell a
  // newly-appeared interface from one that has been sitting there all along.
  const seenKeysRef = useRef<Set<string> | null>(null);
  // A transport switch is a connect() of its own; never start a second one
  // while the first is still walking the device.
  const switchingRef = useRef(false);
  // The interface we were on when the mouse vanished mid-session (cable or
  // dongle pulled). While set, the scan below keeps looking for it, or its
  // sibling transport, and reconnects on its own when either shows up. The
  // launch-time auto-reconnect is deliberately one-shot (see refresh()); this
  // is the in-session counterpart, scoped to a device that was connected and
  // then physically went away, not to whatever the user last browsed.
  const awaitingReturnRef = useRef<HidInterfaceInfo | null>(null);
  // Bumped by every walk that is going to commit a snapshot (`connect` and
  // `switchTo`). A walk whose generation is no longer the latest throws its
  // result away instead of committing it. The two kinds of walk are on
  // different hid-open-lock keys (the receiver's and the cable's), so nothing
  // else serializes them — and an in-flight background refresh used to land
  // *after* a transport switch had already committed the wired snapshot,
  // putting the older Wireless one back on top for the rest of the session
  // (no retry either, since that key had already been marked as seen). The
  // UI, tray and overlay then reported Wireless until the user manually
  // reconnected or relaunched.
  const walkGenerationRef = useRef(0);

  const connect = useCallback(async (candidate: CandidateInterface, opts?: { silent?: boolean; silentErrors?: boolean; retry?: boolean }) => {
    const key = candidate.info.key;
    // Same underlying call whether this is a first connect or a background/
    // manual re-read of the device already showing — only the toast wording
    // differs, so tell them apart before the read (status.name isn't known
    // for a fresh connect's error case).
    const isRefresh = connectedRef.current?.key === key;
    const generation = walkGenerationRef.current + 1;
    walkGenerationRef.current = generation;
    setConnectingKey(key);
    // A connect that needs retries used to look like a click that did nothing,
    // which is what made people click again — and land on the walk lock for
    // their trouble. The progress toast is sticky until the outcome is known.
    // Background callers get none: they run every few seconds and must stay
    // silent (updateToast(null) below is a no-op).
    const name = candidate.info.productString || "device";
    const progress = opts?.silent || opts?.silentErrors
      ? null
      : showProgressToast(isRefresh ? `Refreshing ${name}…` : `Connecting to ${name}…`);
    try {
      const device = await connectWithRetry(candidate.info, opts);
      // Superseded while this walk was on the wire (a transport switch, or a
      // newer reconnect) — see walkGenerationRef.
      if (generation !== walkGenerationRef.current) {
        if (progress !== null) dismissToast(progress);
        return;
      }
      lastReadAtRef.current = Date.now();
      awaitingReturnRef.current = null;
      setConnected(device);
      lastCandidateRef.current = candidate;
      rememberDevice(candidate.info, device.brand);
      // What it actually is, not what its interface called itself: a receiver
      // enumerates as "USB Receiver" and only reveals "PRO X SUPERLIGHT 2c"
      // when the HID++ read above asks it. The device list shows this.
      rememberDeviceName(candidate.info.key, device.status.name);
      setFailedKey((previous) => (previous === candidate.info.key ? null : previous));
      // Only switch to the device view on a *new* connection — background
      // re-reads of the already-connected device must not yank the user back
      // to the device page when they've navigated to the list.
      if (!isRefresh) setView("device");
      updateToast(progress, {
        text: isRefresh ? `${device.status.name} refreshed.` : `Connected to ${device.status.name}.`,
        kind: "success",
        loading: false,
        durationMs: 4000,
      });
    } catch (error) {
      // connectToInterface() enforces the "one walk per device at a time"
      // lock itself now (native-hid/hid-open-lock.ts) — a busy error here
      // means some OTHER call (background auto-refresh racing a manual
      // click, or a dev-server hot-reload remount) is already mid-walk on
      // this same device, not that this one actually failed. Swallow it
      // rather than showing the user an error for something that isn't one.
      if (isHidBusyError(error)) {
        if (progress !== null) dismissToast(progress);
        return;
      }
      // `silentErrors` is for a caller that retries on its own timer: the
      // awaiting-return reconnect runs every transport scan (3 s), so a
      // device that is enumerated but not answering — a receiver whose mouse
      // is asleep, say — used to toast the raw driver error again and again
      // with no way to make it stop short of unplugging the dongle. The
      // success path still toasts, since that one is worth interrupting for.
      if (!opts?.silent && !opts?.silentErrors) {
        setFailedKey(key);
        // The toast is the user-facing half; this line is the one that
        // survives the session. A connect that needs several clicks is
        // exactly the kind of report with no reproduction attached, so the
        // raw driver error goes to the on-disk log as well.
        void invoke("log_line", {
          line: `[connect] ${key} failed: ${error instanceof Error ? error.message : String(error)}`,
        }).catch(() => {});
        // The device *is* answering: its receiver replies to every command with
        // status 0x00 — received, not processed (0x02 is done) — echoing our own
        // class/command back. CONFIRMED by reading those replies in the log for
        // a Razer whose receiver was enumerated, accepted every write, and
        // never ran one. That is a receiver with nothing behind it: unpaired,
        // out of range, or wedged. It is not something the app can retry past,
        // so name the remedy rather than dumping the status wall.
        if (isQueuedNotProcessedError(error)) {
          const name = candidate.info.productString || "This device";
          updateToast(progress, {
            text: `${name} isn't accepting commands — unplug its receiver for a moment and plug it back in, then try again.`,
            kind: "error",
            loading: false,
            durationMs: 8000,
          });
          return;
        }
        const message = await conflictMessageOr(error);
        updateToast(progress, { text: message, kind: "error", loading: false, durationMs: 6000 });
      }
    } finally {
      // Only the walk that is still current owns the "connecting…" state; a
      // superseded one must not clear it out from under its replacement.
      if (generation === walkGenerationRef.current) setConnectingKey(null);
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
        // Exact key first; failing that, the same mouse on its other transport
        // (remembered over the cable, launched with only the receiver in, say).
        const match = remembered
          && (candidates.find((c) => c.info.key === remembered.key)
            ?? candidates.find((c) => isSiblingTransport(c.info, remembered)));
        // Silent: a remembered device that's simply not plugged in yet
        // shouldn't greet the user with an error toast on every launch. `retry`
        // because this is the one connect with no timer behind it — a mouse
        // that answers an unprocessed echo for a moment would otherwise leave
        // the app sitting on the device list until the user clicked.
        if (match) void connect(match, { silent: true, retry: true });
      }
    } catch (error) {
      setList({ status: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }, [connect]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // On Linux the portable .AppImage has no install step, so the bundled udev
  // rule (which lets the app open mice' HID interfaces at all) may be
  // missing — without it every connect would hang on "connecting…". Install
  // it proactively once at launch. The Rust command is idempotent and
  // no-ops off-Linux and when the rule is already present (deb/rpm ship it
  // via their maintainer script), so this is safe to fire unconditionally
  // every start.
  useEffect(() => {
    let cancelled = false;
    void invoke<string>("install_udev_rules").then((result) => {
      if (cancelled || result !== "installed") return;
      showToast(
        "Enabled mouse access — if you already had a device selected, reconnect it now.",
        "success",
      );
    });
    return () => {
      cancelled = true;
    };
  }, []);

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
      // A full readStatus() walk (5 splits opened, 20-30 HID++ round trips)
      // every 5 s adds up when nobody is looking at the result, so the
      // cadence follows what the user can currently see. Whichever slow
      // state applies, the read still happens eventually: the old behaviour
      // of skipping outright is what left battery and connection type stale
      // until a relaunch. The tick itself stays at 5 s; the slower states
      // just let more ticks go by since the last successful read.
      let due = AUTO_REFRESH_INTERVAL_MS;
      if (document.hidden) due = HIDDEN_REFRESH_INTERVAL_MS;
      else if (!windowFocusedRef.current) due = UNFOCUSED_REFRESH_INTERVAL_MS;
      if (autoRefreshPausedRef.current) due = Math.max(due, EDITING_REFRESH_INTERVAL_MS);
      if (Date.now() - lastReadAtRef.current < due) return;
      if (switchingRef.current) return;
      if (lastCandidateRef.current) void connect(lastCandidateRef.current, { silent: true });
    }, AUTO_REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected?.key, connect]);

  // Transport hot-swap. Re-enumerates interfaces on a short timer (nothing is
  // opened) and reacts to two events for the connected mouse:
  //
  // 1. Its interface disappeared (cable unplugged, receiver pulled): connect
  //    to the same mouse on whatever sibling transport is still present, or
  //    fall back to the device list if there is none.
  // 2. A sibling interface appeared (cable plugged into a mouse that was on
  //    its receiver): read it, and if the unit id says it is the same mouse,
  //    move to it when it is the wired side. Wired is where the mouse really
  //    is at that point (the receiver often keeps answering from a cache),
  //    and it is the only side that reports charging. The reverse (dongle
  //    inserted while wired) deliberately does not switch.
  //
  // Both used to need a quit and relaunch, because nothing re-scanned once
  // something was connected and the auto-reconnect only ever ran once.
  const [awaitingReturn, setAwaitingReturn] = useState(false);
  useEffect(() => {
    if (!connected && !awaitingReturn) {
      seenKeysRef.current = null;
      return;
    }
    const interval = setInterval(async () => {
      if (switchingRef.current) return;
      let candidates: CandidateInterface[];
      try {
        candidates = await listCandidateInterfaces();
      } catch {
        return;
      }
      // 0. Waiting for a mouse that went away: reconnect the moment it, or
      //    its other transport, is back. Silent connect; the success toast
      //    and the switch to the device view come from connect() itself.
      const awaiting = awaitingReturnRef.current;
      if (!connectedRef.current) {
        if (!awaiting) return;
        const back = candidates.find((c) => c.info.key === awaiting.key)
          ?? candidates.find((c) => isSiblingTransport(c.info, awaiting));
        if (back) {
          setList((prev) => (prev.status === "loaded" ? { status: "loaded", candidates } : prev));
          // Errors stay quiet: this scan runs every few seconds and will try
          // again, so a device that is present but not answering must not
          // produce a toast per tick. The success toast still comes from
          // connect() itself.
          await connect(back, { silentErrors: true });
          if (connectedRef.current) setAwaitingReturn(false);
        }
        return;
      }
      const current = connectedRef.current;
      const currentInfo = lastCandidateRef.current?.info;
      if (!current || !currentInfo) return;
      const keys = new Set(candidates.map((c) => c.info.key));
      const seen = seenKeysRef.current;
      seenKeysRef.current = keys;
      // Keep the device list current too, quietly (no loading flicker), so
      // "Back" shows what is actually plugged in right now.
      setList((prev) => {
        if (prev.status !== "loaded") return prev;
        const same = prev.candidates.length === candidates.length
          && prev.candidates.every((c, i) => c.info.key === candidates[i].info.key);
        return same ? prev : { status: "loaded", candidates };
      });

      const switchTo = async (target: CandidateInterface, expectWired: boolean): Promise<boolean> => {
        switchingRef.current = true;
        // Claiming the newest generation is what retires any status refresh
        // already on the wire for the old transport — see
        // walkGenerationRef. Without it that refresh committed its older
        // snapshot after this switch, silently reverting the app to the
        // Wireless transport for the rest of the session.
        const generation = walkGenerationRef.current + 1;
        walkGenerationRef.current = generation;
        try {
          const device = await connectToInterface(target.info);
          // Superseded by a newer walk while this one was reading.
          if (generation !== walkGenerationRef.current) return false;
          // Same mouse? The serial settles it where the driver reports one;
          // the product-string match already got us here otherwise.
          const sameUnit = device.status.unitId == null || current.status.unitId == null
            || device.status.unitId === current.status.unitId;
          if (!sameUnit) return false;
          if (expectWired && device.status.connectionType !== "Wired") return false;
          lastReadAtRef.current = Date.now();
          setConnected(device);
          lastCandidateRef.current = target;
          rememberDevice(target.info, device.brand);
          showToast(`${device.status.name}: now ${device.status.connectionType?.toLowerCase() ?? "connected"}.`, "success");
          return true;
        } catch (error) {
          if (!isHidBusyError(error)) {
            const message = error instanceof Error ? error.message : String(error);
            void invoke("log_line", { line: `[transport] switch to ${target.info.key} failed: ${message}` }).catch(() => {});
          }
          return false;
        } finally {
          switchingRef.current = false;
        }
      };

      if (!keys.has(current.key)) {
        // 1. Gone. Try a sibling; otherwise drop to the list.
        const sibling = candidates.find((c) => isSiblingTransport(c.info, currentInfo));
        if (sibling && (await switchTo(sibling, false))) return;
        awaitingReturnRef.current = currentInfo;
        setAwaitingReturn(true);
        setConnected(null);
        lastCandidateRef.current = null;
        setView("list");
        showToast(`${current.status.name} disconnected. It will reconnect when plugged back in.`, "error");
        return;
      }
      if (seen && current.status.connectionType !== "Wired") {
        // 2. Something new since the last scan that looks like this mouse.
        const appeared = candidates.find((c) => !seen.has(c.info.key) && isSiblingTransport(c.info, currentInfo));
        if (appeared) await switchTo(appeared, true);
      }
    }, TRANSPORT_SCAN_INTERVAL_MS);
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected?.key, awaitingReturn]);

  // Mirror the cached device name and battery into the tray menu (tray.rs),
  // so a right-click on the tray icon shows the charge without bringing the
  // window back. Keyed on the three fields the menu shows, not on `connected`
  // itself, so a patchStatus() that changes only DPI doesn't re-send it.
  const trayName = connected?.status.name ?? null;
  const trayBattery = connected?.status.batteryPercent ?? null;
  const trayBatteryState = connected?.status.batteryState ?? null;
  useEffect(() => {
    const status = trayName === null
      ? null
      : { name: trayName, batteryPercent: trayBattery, batteryState: trayBatteryState ?? "Unknown" };
    // Tray text is cosmetic — a failure here (tray failed to build at
    // startup, say) shouldn't surface as a device error.
    void invoke("tray_set_device_status", { status }).catch(() => {});
  }, [trayName, trayBattery, trayBatteryState]);

  // Same cache, pushed to the OBS overlay server (stream_overlay.rs) — a
  // separate effect from the tray one above because the overlay cares about
  // DPI/polling rate too, which the tray menu doesn't show and so isn't in
  // its dependency list.
  //
  // Deliberately unconditional, not gated on isStreamOverlayEnabled(): the
  // Rust side just caches the latest snapshot (cheap) and a new `/events`
  // connection is painted from that cache the moment it opens
  // (stream_overlay.rs's handle_sse), so pushing while the overlay is *off*
  // is what makes turning it on show the already-connected mouse right away.
  // Gating it here is a real bug that was fixed: the effect's own
  // dependencies (name/DPI/polling rate) don't change when the user flips the
  // setting, so the server's cache stayed empty and both the Settings preview
  // and the OBS source said "No mouse connected" until some field happened to
  // change on the device — looking broken exactly when someone first turns
  // the feature on.
  const overlayDpi = connected?.status.dpi ?? null;
  const overlayPollingRateHz = connected?.status.pollingRateHz ?? null;
  const overlayKey = connected?.key ?? null;
  // A user with more than one mouse can pin the overlay to a specific one
  // (Settings' device picker, saved via stream-overlay.ts) instead of it
  // following whichever device happens to be active — otherwise switching
  // devices mid-stream would silently swap what the overlay shows. Held in
  // state via subscription, not read once, so flipping the pin in Settings
  // re-pushes immediately rather than waiting for the active device's own
  // name/DPI/rate to next change.
  const [overlayPinnedKey, setOverlayPinnedKey] = useState<string | null>(null);
  useEffect(() => subscribeStreamOverlayDeviceKey(setOverlayPinnedKey), []);
  useEffect(() => {
    const matchesPin = overlayPinnedKey === null || overlayKey === overlayPinnedKey;
    const status = trayName === null || !matchesPin
      ? null
      : { name: trayName, dpi: overlayDpi, pollingRateHz: overlayPollingRateHz };
    void pushStreamOverlayStatus(status).catch(() => {});
  }, [trayName, overlayDpi, overlayPollingRateHz, overlayKey, overlayPinnedKey]);

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
    failedKey,
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
