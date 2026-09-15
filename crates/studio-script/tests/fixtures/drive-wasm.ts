// Differential harness driver: instantiate one Studio wasm module, capture
// every hostEmit call, drive studio_init then one studio_event per stdin
// line, and print each captured message as one JSON line.
//
// Usage: bun drive-wasm.ts <module.wasm>
// Stdin: JSON host-event envelopes, one per line (empty lines ignored).
import { readFileSync } from "node:fs";

const [wasmPath] = Bun.argv.slice(2);
if (!wasmPath) {
  console.error("usage: bun drive-wasm.ts <module.wasm>");
  process.exit(2);
}

const captured: string[] = [];
const compiled = await WebAssembly.compile(
  await Bun.file(wasmPath).arrayBuffer(),
);
let instance: WebAssembly.Instance;
instance = await WebAssembly.instantiate(compiled, {
  studio_host: {
    emit(pointer: number, length: number): number {
      const memory = instance.exports.memory as WebAssembly.Memory;
      const bytes = new Uint8Array(memory.buffer, pointer, length);
      captured.push(new TextDecoder().decode(bytes));
      return 0;
    },
  },
  env: {
    // AssemblyScript's abort signature (message, file, line, column);
    // modules that never abort link fine with the extra parameters.
    abort(_message: number, _file: number, _line: number, _column: number): void {
      throw new Error("guest abort");
    },
  },
});

const alloc = instance.exports.studio_alloc as (length: number) => number;
const init = instance.exports.studio_init as (pointer: number, length: number) => number;
const onEvent = instance.exports.studio_event as (pointer: number, length: number) => number;

function send(payload: string): void {
  const before = captured.length;
  const bytes = new TextEncoder().encode(payload);
  const pointer = alloc(bytes.length);
  const memory = instance.exports.memory as WebAssembly.Memory;
  new Uint8Array(memory.buffer, pointer, bytes.length).set(bytes);
  if (pointer === 0) {
    throw new Error("guest refused allocation");
  }
  if (payload === "") {
    init(pointer, 0);
  } else {
    onEvent(pointer, bytes.length);
  }
  for (const message of captured.slice(before)) {
    console.log(message);
  }
}

send("");
let pending = "";
const decoder = new TextDecoder();
for await (const chunk of Bun.stdin.stream()) {
  pending += decoder.decode(chunk, { stream: true });
  const lines = pending.split("\n");
  pending = lines.pop() ?? "";
  for (const line of lines) {
    if (line.trim().length > 0) {
      send(line.trim());
    }
  }
}
if (pending.trim().length > 0) {
  send(pending.trim());
}
