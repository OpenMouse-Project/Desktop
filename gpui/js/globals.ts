// The browser globals @openmouse/protocol's drivers and src/native-hid reach
// for, which QuickJS does not provide. Imported first by entry.ts. Each is
// defined only when missing.

declare const __host: { sleep(ms: number): Promise<void>; platform: string; log(line: string): void };
const g = globalThis as any;

g.window ??= g;
g.self ??= g;

const platform: string = __host.platform;
g.navigator ??= {
  userAgent: platform === "windows" ? "Windows" : platform === "macos" ? "Macintosh" : "Linux",
  platform: platform === "windows" ? "Win32" : platform === "macos" ? "MacIntel" : "Linux",
};

g.console ??= {};
for (const level of ["log", "info", "warn", "error", "debug"]) {
  g.console[level] ??= (...args: unknown[]) => __host.log(`[js ${level}] ${args.map(String).join(" ")}`);
}

const started = Date.now();
g.performance ??= { now: () => Date.now() - started };

// Timers ride on one native async sleep each; clearing just marks the id dead.
let nextTimer = 1;
const live = new Set<number>();
g.setTimeout ??= (fn: (...a: unknown[]) => void, ms = 0, ...args: unknown[]) => {
  const id = nextTimer++;
  live.add(id);
  __host.sleep(Math.max(0, ms)).then(() => {
    if (live.delete(id)) fn(...args);
  });
  return id;
};
g.clearTimeout ??= (id: number) => {
  live.delete(id);
};
g.setInterval ??= (fn: (...a: unknown[]) => void, ms = 0, ...args: unknown[]) => {
  const id = nextTimer++;
  live.add(id);
  const tick = () =>
    __host.sleep(Math.max(1, ms)).then(() => {
      if (!live.has(id)) return;
      fn(...args);
      tick();
    });
  tick();
  return id;
};
g.clearInterval ??= g.clearTimeout;

g.queueMicrotask ??= (fn: () => void) => Promise.resolve().then(fn);
g.structuredClone ??= (value: unknown) => JSON.parse(JSON.stringify(value));
g.crypto ??= {
  getRandomValues<T extends ArrayBufferView>(array: T): T {
    const bytes = new Uint8Array(array.buffer, array.byteOffset, array.byteLength);
    for (let i = 0; i < bytes.length; i += 1) bytes[i] = Math.floor(Math.random() * 256);
    return array;
  },
};

g.TextEncoder ??= class TextEncoder {
  encode(input = ""): Uint8Array {
    const out: number[] = [];
    for (const ch of input) {
      const c = ch.codePointAt(0)!;
      if (c < 0x80) out.push(c);
      else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63));
      else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
      else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
    }
    return Uint8Array.from(out);
  }
};
g.TextDecoder ??= class TextDecoder {
  decode(input?: ArrayBufferView | ArrayBuffer): string {
    if (!input) return "";
    const b = input instanceof ArrayBuffer ? new Uint8Array(input) : new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    let out = "";
    for (let i = 0; i < b.length; ) {
      const c = b[i++];
      let cp: number;
      if (c < 0x80) cp = c;
      else if (c < 0xe0) cp = ((c & 31) << 6) | (b[i++] & 63);
      else if (c < 0xf0) { cp = ((c & 15) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63); }
      else { cp = ((c & 7) << 18) | ((b[i++] & 63) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63); }
      out += String.fromCodePoint(cp);
    }
    return out;
  }
};
