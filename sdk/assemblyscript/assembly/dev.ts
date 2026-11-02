/** Development-only state transfer. The CLI entry wrapper exports the ABI hooks.
 * Register each module at module scope with a stable id. Each snapshot is an
 * application-owned, versioned document; restore runs before studio_init.
 * Production compilation removes registration when STUDIO_DEV is undefined.
 */
@external("studio_host", "emit")
declare function emit(pointer: i32, length: i32): i32;

class HotState {
  constructor(public read: () => string, public write: (document: string) => void) {}
}
let states: Map<string, HotState> | null = null;
const HEADER = "studio-hot/1\n";

export function registerHotState(id: string, read: () => string, write: (document: string) => void): void {
  if (isDefined(STUDIO_DEV)) {
    let registry = states;
    if (registry === null) {
      registry = new Map<string, HotState>();
      states = registry;
    }
    assert(id.length > 0 && !registry.has(id), "hot state ids must be unique");
    registry.set(id, new HotState(read, write));
  }
}

export function studio_dev_snapshot(_pointer: i32, _length: i32): i32 {
  const registry = states;
  if (registry === null || registry.size === 0) return 0;
  let document = HEADER;
  const keys = registry.keys().sort();
  for (let i = 0; i < keys.length; i++) {
    const id = keys[i];
    const value = registry.get(id).read();
    document += id.length.toString() + ":" + id + value.length.toString() + ":" + value;
  }
  const bytes = String.UTF8.encode(document, false);
  if (bytes.byteLength > 65536) return 1;
  return emit(changetype<i32>(bytes), bytes.byteLength);
}

class Cursor {
  position: i32 = HEADER.length;
  constructor(public document: string) {}
  read(): string {
    const end = this.document.indexOf(":", this.position);
    assert(end > this.position, "invalid hot state frame");
    let length: i32 = 0;
    for (let i = this.position; i < end; i++) {
      const digit = this.document.charCodeAt(i) - 48;
      assert(digit >= 0 && digit <= 9, "invalid hot state length");
      assert(length <= (65536 - digit) / 10, "hot state field exceeds limit");
      length = length * 10 + digit;
    }
    this.position = end + 1;
    assert(length <= this.document.length - this.position, "truncated hot state frame");
    const value = this.document.substring(this.position, this.position + length);
    this.position += length;
    return value;
  }
}

export function studio_dev_restore(pointer: i32, length: i32): i32 {
  if (length < 0 || length > 65536) return 1;
  const document = String.UTF8.decodeUnsafe(changetype<usize>(pointer), length, false);
  if (!document.startsWith(HEADER)) return 1;
  const cursor = new Cursor(document);
  const registry = states;
  while (cursor.position < document.length) {
    const id = cursor.read();
    const value = cursor.read();
    if (registry !== null && registry.has(id)) registry.get(id).write(value);
  }
  return 0;
}
