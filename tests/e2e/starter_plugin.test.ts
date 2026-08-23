import { describe, expect, test } from "bun:test";
import { buildExample } from "../../scripts/build-example";

describe("documented starter plugin", () => {
  test("builds, packages, mounts, patches one property, and emits a visible invalid operation", async () => {
    const built = await buildExample("starter");
    expect(built.bytes.length).toBeGreaterThan(100);
    const module = await WebAssembly.compile(
      await Bun.file("examples/starter/build/starter.wasm").arrayBuffer(),
    );
    let instance: WebAssembly.Instance;
    const emissions: unknown[] = [];
    instance = await WebAssembly.instantiate(module, {
      studio_host: {
        emit(pointer: number, length: number) {
          const memory = instance.exports.memory as WebAssembly.Memory;
          const bytes = new Uint8Array(memory.buffer, pointer, length);
          emissions.push(JSON.parse(new TextDecoder().decode(bytes)));
          return 0;
        },
      },
      env: { abort() { throw new Error("guest abort"); } },
    });
    const init = instance.exports.studio_init as (pointer: number, length: number) => number;
    const event = instance.exports.studio_event as (pointer: number, length: number) => number;
    const alloc = instance.exports.studio_alloc as (length: number) => number;
    init(0, 0);
    expect(emissions[0]).toMatchObject({ type: "mount", payload: { route: "/counter" } });

    function send(nodeId: string): void {
      const bytes = new TextEncoder().encode(JSON.stringify({ type: "ui", payload: { node_id: nodeId, event: "pressed", payload: {} } }));
      const pointer = alloc(bytes.length);
      new Uint8Array((instance.exports.memory as WebAssembly.Memory).buffer, pointer, bytes.length).set(bytes);
      event(pointer, bytes.length);
    }
    send("increment");
    expect(emissions[1]).toMatchObject({ type: "patch", payload: { operations: [{ node_id: "total", property: "text", value: "Count: 1 — Derived total: $1.25" }] } });
    send("invalid-demo");
    expect(emissions[2]).toMatchObject({ payload: { operations: [{ node_id: "missing-node" }] } });
  });
});
