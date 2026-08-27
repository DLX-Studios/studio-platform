@external("studio_host", "emit")
declare function hostEmit(pointer: i32, length: i32): i32;

let sequence: i64 = 0;
let signedIn: bool = false;
function emit(value: string): i32 {
  const bytes = String.UTF8.encode(value, false);
  return hostEmit(changetype<i32>(bytes), bytes.byteLength);
}
function patch(node: string, property: string, value: string): i32 {
  sequence += 1;
  return emit('{"type":"patch","payload":{"sequence":' + sequence.toString() + ',"operations":[{"op":"update_prop","node_id":"' + node + '","property":"' + property + '","value":"' + value + '"}]}}');
}
export function studio_alloc(length: i32): i32 { return length <= 65536 ? 32768 : 0; }
export function studio_dealloc(_pointer: i32, _length: i32): void {}
export function studio_init(_pointer: i32, _length: i32): i32 {
  return emit('{"type":"mount","payload":{"protocol_version":1,"route":"/github","root":{"id":"root","kind":"scaffold","props":{},"children":[{"id":"bar","kind":"app_bar","props":{"title":"GitHub Viewer"},"children":[]},{"id":"content","kind":"column","props":{"gap":16},"children":[{"id":"title","kind":"text","props":{"text":"OAuth proof application"},"children":[]},{"id":"status","kind":"text","props":{"text":"Sign in to list repositories"},"children":[]},{"id":"signin","kind":"button","props":{"label":"Sign in with GitHub","enabled":true,"on_pressed":"signin_pressed"},"children":[]},{"id":"hint","kind":"text","props":{"text":"Repository data is requested only through declared GitHub routes."},"children":[]}]},{"id":"footer","kind":"status_bar","props":{"value":"Ready"},"children":[]}]}}');
}
export function studio_event(pointer: i32, length: i32): i32 {
  const event = String.UTF8.decodeUnsafe(pointer, length, false);
  if (signedIn || !event.includes('"node_id":"signin"')) return 0;
  signedIn = true;
  return patch("status", "text", "Waiting for host OAuth callback and repository response");
}
