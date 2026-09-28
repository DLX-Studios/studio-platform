@external("studio_host", "emit")
declare function hostEmit(pointer: i32, length: i32): i32;

let sequence: i64 = 0;
let signInRequested: bool = false;
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
function repositoryList(event: string): string {
  const marker = '"repositoryNames":[';
  let cursor = event.indexOf(marker);
  if (cursor < 0) return "";
  cursor += marker.length;
  let output = "";
  let count = 0;
  while (cursor < event.length && count < 5) {
    while (cursor < event.length && event.charCodeAt(cursor) === 32) cursor++;
    if (cursor >= event.length || event.charCodeAt(cursor) === 93) break;
    if (event.charCodeAt(cursor) !== 34) return "";
    const start = cursor + 1;
    const end = event.indexOf('"', start);
    if (end < 0) return "";
    if (count > 0) output += ", ";
    output += event.substring(start, end);
    cursor = end + 1;
    while (cursor < event.length && event.charCodeAt(cursor) !== 44 && event.charCodeAt(cursor) !== 93) cursor++;
    if (cursor < event.length && event.charCodeAt(cursor) === 44) cursor++;
    count++;
  }
  return output;
}
export function studio_event(pointer: i32, length: i32): i32 {
  const event = String.UTF8.decodeUnsafe(pointer, length, false);
  if (event.includes('"type":"action_result"')) {
    if (event.includes('"status":"success"')) {
      signInRequested = true;
      const repositories = repositoryList(event);
      patch("status", "text", "Signed in with GitHub");
      return patch("hint", "text", repositories.length > 0 ? "Repositories: " + repositories : "No visible repositories");
    }
    signInRequested = false;
    return patch("status", "text", "GitHub sign-in failed; try again");
  }
  if (signInRequested || !event.includes('"node_id":"signin"')) return 0;
  signInRequested = true;
  sequence += 1;
  return emit('{"type":"action","payload":{"request_id":"github-sign-in","capability":"github.oauth","operation":"sign_in","payload":{"provider":"github","scopes":["read:user","user:email"]}}}');
}
