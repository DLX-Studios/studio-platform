@external("studio_host", "emit")
declare function hostEmit(pointer: i32, length: i32): i32;

let count: i32 = 0;
let sequence: i64 = 0;
function retained(): void {}
let tableEntry: () => void = retained;

function emit(message: string): i32 {
  const bytes = String.UTF8.encode(message, false);
  return hostEmit(changetype<i32>(bytes), bytes.byteLength);
}
function total(): i32 { return count * 125; }
function formatMinor(value: i32): string {
  const cents = value % 100;
  return (value / 100).toString() + "." + (cents < 10 ? "0" : "") + cents.toString();
}
function patch(target: string): string {
  sequence += 1;
  return '{"type":"patch","payload":{"sequence":' + sequence.toString() +
    ',"operations":[{"op":"update_prop","node_id":"' + target +
    '","property":"text","value":"Count: ' + count.toString() +
    ' — Derived total: $' + formatMinor(total()) + '"}]}}';
}

export function studio_alloc(length: i32): i32 { return length <= 65536 ? 32768 : 0; }
export function studio_dealloc(_pointer: i32, _length: i32): void {}
export function studio_init(_pointer: i32, _length: i32): i32 {
  tableEntry();
  return emit('{"type":"mount","payload":{"protocol_version":1,"route":"/counter",' +
    '"root":{"id":"root","kind":"column","props":{"gap":12},"children":[' +
    '{"id":"total","kind":"text","props":{"text":"Count: 0 — Derived total: $0.00"},"children":[]},' +
    '{"id":"increment","kind":"button","props":{"label":"Increment","enabled":true,' +
    '"on_pressed":"increment_pressed"},"children":[]}]}}}');
}
export function studio_event(pointer: i32, length: i32): i32 {
  tableEntry = retained;
  const event = String.UTF8.decodeUnsafe(pointer, length, false);
  if (event.includes('"node_id":"increment"')) { count += 1; return emit(patch("total")); }
  if (event.includes('"node_id":"invalid-demo"')) return emit(patch("missing-node"));
  return 0;
}
