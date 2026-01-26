// Polyfills - loaded as entry point before other modules
// Sets up global APIs that just-bash and other code may need

const ops = Deno.core.ops;

// ============================================================================
// Process (Node.js compatibility)
// ============================================================================

globalThis.process = {
  env: {},
  cwd: () => "/",
  platform: "linux",
  version: "v20.0.0",
  versions: { node: "20.0.0" },
  argv: [],
  stdout: { write: () => {} },
  stderr: { write: () => {} },
};

// ============================================================================
// TextEncoder / TextDecoder (Web APIs)
// ============================================================================

globalThis.TextEncoder = class TextEncoder {
  encode(str) {
    const utf8 = [];
    for (let i = 0; i < str.length; i++) {
      let charCode = str.charCodeAt(i);
      if (charCode < 0x80) {
        utf8.push(charCode);
      } else if (charCode < 0x800) {
        utf8.push(0xc0 | (charCode >> 6), 0x80 | (charCode & 0x3f));
      } else if (charCode < 0xd800 || charCode >= 0xe000) {
        utf8.push(
          0xe0 | (charCode >> 12),
          0x80 | ((charCode >> 6) & 0x3f),
          0x80 | (charCode & 0x3f)
        );
      } else {
        // Surrogate pair
        i++;
        charCode = 0x10000 + (((charCode & 0x3ff) << 10) | (str.charCodeAt(i) & 0x3ff));
        utf8.push(
          0xf0 | (charCode >> 18),
          0x80 | ((charCode >> 12) & 0x3f),
          0x80 | ((charCode >> 6) & 0x3f),
          0x80 | (charCode & 0x3f)
        );
      }
    }
    return new Uint8Array(utf8);
  }
};

globalThis.TextDecoder = class TextDecoder {
  decode(bytes) {
    if (!bytes) return "";
    const arr = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    let result = "";
    let i = 0;
    while (i < arr.length) {
      const byte1 = arr[i++];
      if (byte1 < 0x80) {
        result += String.fromCharCode(byte1);
      } else if (byte1 < 0xe0) {
        const byte2 = arr[i++];
        result += String.fromCharCode(((byte1 & 0x1f) << 6) | (byte2 & 0x3f));
      } else if (byte1 < 0xf0) {
        const byte2 = arr[i++];
        const byte3 = arr[i++];
        result += String.fromCharCode(
          ((byte1 & 0x0f) << 12) | ((byte2 & 0x3f) << 6) | (byte3 & 0x3f)
        );
      } else {
        const byte2 = arr[i++];
        const byte3 = arr[i++];
        const byte4 = arr[i++];
        const codePoint =
          ((byte1 & 0x07) << 18) |
          ((byte2 & 0x3f) << 12) |
          ((byte3 & 0x3f) << 6) |
          (byte4 & 0x3f);
        // Convert to surrogate pair
        const surrogate = codePoint - 0x10000;
        result += String.fromCharCode(
          0xd800 + (surrogate >> 10),
          0xdc00 + (surrogate & 0x3ff)
        );
      }
    }
    return result;
  }
};

// ============================================================================
// Timers (setTimeout, setInterval, etc.)
// ============================================================================

let nextTimerId = 1;
const activeTimers = new Map();

globalThis.setTimeout = (callback, delay = 0, ...args) => {
  const id = nextTimerId++;
  activeTimers.set(id, { type: "timeout", cancelled: false });

  ops.op_sleep(BigInt(Math.max(0, delay))).then(() => {
    const timer = activeTimers.get(id);
    if (timer && !timer.cancelled) {
      activeTimers.delete(id);
      callback(...args);
    }
  });

  return id;
};

globalThis.clearTimeout = (id) => {
  const timer = activeTimers.get(id);
  if (timer) {
    timer.cancelled = true;
    activeTimers.delete(id);
  }
};

globalThis.setInterval = (callback, delay = 0, ...args) => {
  const id = nextTimerId++;
  activeTimers.set(id, { type: "interval", cancelled: false });

  const runInterval = async () => {
    while (true) {
      await ops.op_sleep(BigInt(Math.max(0, delay)));
      const timer = activeTimers.get(id);
      if (!timer || timer.cancelled) {
        break;
      }
      callback(...args);
    }
  };

  runInterval();
  return id;
};

globalThis.clearInterval = (id) => {
  const timer = activeTimers.get(id);
  if (timer) {
    timer.cancelled = true;
    activeTimers.delete(id);
  }
};
