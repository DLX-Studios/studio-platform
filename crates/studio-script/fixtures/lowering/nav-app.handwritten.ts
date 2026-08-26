// Hand-written counterpart of `nav-app.studio`.
//
// This module follows the exact conventions of the existing example guests
// (`examples/starter`, `examples/pos-desktop`) and implements the same
// observable behavior by hand: mounting the home screen at startup and
// issuing one navigation command per declared interaction.  The parity test
// in `tests/wasm_emission.rs` asserts that the compiler-generated module's
// observable messages are byte-identical to the string literals below.
//
// It is a reviewed reference artifact, not part of any build.

@external("studio_host", "emit")
declare function hostEmit(pointer: i32, length: i32): i32;

function emit(message: string): i32 {
  const bytes = String.UTF8.encode(message, false);
  return hostEmit(changetype<i32>(bytes), bytes.byteLength);
}

export function studio_alloc(length: i32): i32 {
  return length <= 65536 ? 32768 : 0;
}
export function studio_dealloc(_pointer: i32, _length: i32): void {}

const MOUNT_PAYLOAD: string =
  '{"type":"mount","payload":{"protocol_version":1,"route":"/home","root":{"id":"home","kind":"screen","props":{"title":"Home"},"children":[{"id":"home-title","kind":"text","props":{"text":"Home"},"children":[]},{"id":"open-detail","kind":"button","props":{"label":"Open detail"},"children":[]},{"id":"search","kind":"text_input","props":{"placeholder":"Search"},"children":[]}]}}}';

const PUSH_DETAIL: string =
  '{"type":"navigate","payload":{"operation":"push","route":"/detail"}}';
const REPLACE_DETAIL: string =
  '{"type":"navigate","payload":{"operation":"replace","route":"/detail"}}';
const POP_DETAIL: string =
  '{"type":"navigate","payload":{"operation":"pop"}}';

export function studio_init(_pointer: i32, _length: i32): i32 {
  return emit(MOUNT_PAYLOAD);
}

export function studio_event(pointer: i32, length: i32): i32 {
  const event = String.UTF8.decodeUnsafe(pointer, length, false);
  if (
    event.includes('"node_id":"open-detail"') &&
    event.includes('"event":"pressed"')
  ) {
    return emit(PUSH_DETAIL);
  }
  if (
    event.includes('"node_id":"search"') &&
    event.includes('"event":"changed"')
  ) {
    return emit(REPLACE_DETAIL);
  }
  if (
    event.includes('"node_id":"back-button"') &&
    event.includes('"event":"pressed"')
  ) {
    return emit(POP_DETAIL);
  }
  return 0;
}
