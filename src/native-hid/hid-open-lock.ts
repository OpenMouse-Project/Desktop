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

/** True for the specific error `withHidOpenLock` throws when already locked. */
export function isHidBusyError(error: unknown): boolean {
  return error instanceof Error && error.message === BUSY_MESSAGE;
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
