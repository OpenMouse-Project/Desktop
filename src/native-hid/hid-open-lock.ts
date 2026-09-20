// Shared across every module that opens a HID interface — scan.ts's
// connectToInterface() (reads) and logitech-actions.ts's withClient()
// (writes), and any future brand's own write module.
//
// `hid_open` on the Rust side is idempotent per (vendorId, productId): a
// second open while one's already registered just returns Ok(()) and shares
// the SAME open group, reader threads included. Two independent callers
// walking that group at once (a status read racing a DPI write, two reads
// racing each other) end up with their requests interleaved on the wire and
// their replies cross-matched into each other's promises — CONFIRMED on
// real hardware as the cause of a spurious "the mouse did not answer": one
// walk's reply got consumed by the other walk's matching logic, so the
// first walk waited forever for a reply that had already arrived and been
// stolen. This is a plain module-level Set, not a ref, specifically so it
// survives a dev-server hot-reload remounting whichever component started a
// walk — a ref tied to that component's instance can't see a walk started
// by the instance hot-reloading replaced it with.
const openKeys = new Set<string>();

const BUSY_MESSAGE = "This device is already busy with another request. Try again in a moment.";

/** True if a walk (read or write) for this device is already in flight. */
export function isHidOpenLocked(key: string): boolean {
  return openKeys.has(key);
}

/**
 * True for the specific error `withHidOpenLock` throws when already locked,
 * and for the same message arriving from Rust: `invoke` rejects with the
 * command's error value, which is a plain string, not an Error — hid.rs's
 * `with_hid_api` refuses a re-entrant enumeration (hidapi's macOS enumerator
 * pumps the main run loop, so the webview's next invoke lands inside an
 * in-flight `hid_open`) with this exact message. Both mean the same thing to
 * every caller here.
 */
export function isHidBusyError(error: unknown): boolean {
  if (error instanceof Error) return error.message === BUSY_MESSAGE;
  return error === BUSY_MESSAGE;
}

/**
 * Runs `action` under the lock for `key`, refusing to start if another walk
 * for the same device is already running. Throws `isHidBusyError` rather
 * than silently no-opping — callers decide what "someone else is using this
 * device" should look like: a second auto-reconnect attempt racing itself
 * isn't user-visible, so use-mouse-connection.ts's connect() swallows it,
 * while a write action colliding with a live read should surface as a real
 * error to whoever clicked Apply.
 */
export async function withHidOpenLock<T>(key: string, action: () => Promise<T>): Promise<T> {
  if (openKeys.has(key)) {
    throw new Error(BUSY_MESSAGE);
  }
  openKeys.add(key);
  try {
    return await action();
  } finally {
    openKeys.delete(key);
  }
}

/**
 * Like `withHidOpenLock`, but retries on contention instead of failing on
 * the first collision — for a caller where the write actually has to land,
 * not just "try once, it's fine if a background tick beats it." CONFIRMED
 * as a real problem, not theoretical: a game-profile write (logitech-actions.ts)
 * landing in the same window as the status auto-refresh's background poll
 * (use-mouse-connection.ts, every 5s) got rejected outright as "busy," and
 * unlike that auto-refresh — which just tries again next tick regardless —
 * a game launching is a one-shot moment nothing retries on its own.
 *
 * Retries every `intervalMs` until `action` succeeds or `timeoutMs`
 * elapses; the auto-refresh's own walk is a handful of round trips (at
 * most a couple seconds even on a receiver doing a full multi-split read),
 * so a window a bit longer than its 5s interval is enough to guarantee
 * landing between two ticks rather than racing the same one repeatedly.
 */
export async function withHidOpenLockRetrying<T>(
  key: string,
  action: () => Promise<T>,
  opts: { timeoutMs?: number; intervalMs?: number } = {},
): Promise<T> {
  const timeoutMs = opts.timeoutMs ?? 8000;
  const intervalMs = opts.intervalMs ?? 300;
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    try {
      return await withHidOpenLock(key, action);
    } catch (error) {
      if (!isHidBusyError(error) || Date.now() >= deadline) throw error;
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
    }
  }
}

/**
 * True when the device answered a command with "queued, not processed": the
 * request echoed back before the mouse ran it. Razer-specific wording
 * (`returned status 0x00` — RAZER_STATUS has no 0x00 at all; `ok` is 0x02,
 * `busy` 0x01, `timeout` 0x04), so it means "received, not done yet" rather
 * than a refusal, and the *same idempotent* operation is safe to repeat after
 * a short wait. CONFIRMED as what a wireless DeathAdder does on both paths
 * here: `setDpi` reporting `Class 0x04 command 0x05 returned status 0x00` and
 * then applying cleanly on a retry, and the status walk failing `Class 0x00
 * command 0x81` for every candidate until the mouse answered — the driver
 * reads a setter's reply exactly once and refuses to re-send it (hid.js's
 * awaitReply), which is right, so retrying the whole call is the caller's job.
 *
 * Both shapes are accepted because the message reaches either as an Error
 * (thrown by a driver) or as a plain string (`invoke` rejecting with a
 * command's error value).
 */
export function isQueuedNotProcessedError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  return message.includes("returned status 0x00");
}
