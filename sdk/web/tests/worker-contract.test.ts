import { describe, expect, test } from "bun:test";
import { WorkerRuntime, decodeGuestBuffer, validateGuestModule } from "../worker";
import type { UiNode, WebConfig } from "../worker-protocol";

const root = (id = "home#root"): UiNode => ({ id, kind: "column", props: {}, children: [] });
const config: WebConfig = { schemaVersion: 1, title: "Stream", routing: { schemaVersion: 1, routes: [
  { id: "home", path: "/", source: "routes/+page.studio" },
  { id: "live", path: "/live", source: "routes/live/+page.studio" },
] }, timers: { schemaVersion: 1, pages: [] }, assets: [] };

function fixture() {
  const memory = new WebAssembly.Memory({ initial: 2, maximum: 4 });
  let emit: (pointer: number, length: number) => number;
  let insideGuest = false;
  let events = 0;
  let selected = { route: "/", id: "home", params: {} };
  let previous = selected;
  let canceled = 0;
  let tree: UiNode = root();
  const configuration = structuredClone(config);
  let failMount = false;
  let nextMessages: unknown[] = [];
  const core = {
    current: () => JSON.stringify(selected),
    navigate: (json: string) => {
      expect(insideGuest).toBe(false);
      const command = JSON.parse(json);
      if (command.route === "/denied") throw new Error("unknown route");
      previous = selected;
      selected = { route: command.route ?? "/", id: command.route === "/live" ? "live" : "home", params: {} };
      return JSON.stringify(selected);
    },
    cancel_navigation: () => { canceled++; selected = previous; return true; },
    accept: (json: string) => {
      const message = JSON.parse(json);
      if (failMount) throw new Error("invalid owner mount");
      tree = message.payload.root ?? tree;
      return JSON.stringify(tree);
    },
    tree: () => JSON.stringify(tree),
    can_input: (id: string, event: string) => id === "go" && event === "pressed",
    timer_allowed: (_id: string, _event: string) => false,
    observe: (_now: number, _completed: string) => {},
    poll_one: (_now: number, _completed: string) => "null",
  };
  function output(message: unknown) {
    const bytes = new TextEncoder().encode(JSON.stringify(message));
    new Uint8Array(memory.buffer, 1024, bytes.length).set(bytes);
    emit(1024, bytes.length);
  }
  const guest = {
    memory,
    studio_alloc: () => 4096,
    studio_dealloc: () => {},
    studio_init: () => { output({ type: "mount", payload: { protocol_version: 1, route: "/", root: root() } }); return 0; },
    studio_event: (pointer: number, length: number) => {
      insideGuest = true;
      events++;
      const event = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, pointer, length)));
      if (event.type === "navigation" && event.payload.accepted) {
        output({ type: "mount", payload: { protocol_version: 1, route: event.payload.route, root: root(event.payload.route === "/live" ? "live#root" : "home#root") } });
      } else if (event.type === "ui") {
        for (const message of nextMessages) output(message);
        nextMessages = [];
      }
      insideGuest = false;
      return 0;
    },
  };
  const runtime = new WorkerRuntime({
    loadConfig: async () => configuration,
    loadCore: async () => core,
    loadGuest: async (imports: { studio_host: { emit: typeof emit } }) => { emit = imports.studio_host.emit; return guest; },
    now: () => 0,
  });
  return { runtime, core, configuration, canceled: () => canceled, events: () => events, output: (messages: unknown[]) => { nextMessages = messages; }, failMount: () => { failMount = true; } };
}

describe("browser worker boundary", () => {
  test("copies checked bounded UTF8 guest buffers", () => {
    const memory = new WebAssembly.Memory({ initial: 2, maximum: 4 });
    new Uint8Array(memory.buffer).set([123, 125]);
    expect(decodeGuestBuffer(memory, 0, 2)).toBe("{}");
    for (const [pointer, length] of [[-1, 2], [0, 65537], [131071, 2], [NaN, 2]]) {
      expect(() => decodeGuestBuffer(memory, pointer!, length!)).toThrow();
    }
    new Uint8Array(memory.buffer).set([0xc0, 0xaf]);
    expect(() => decodeGuestBuffer(memory, 0, 2)).toThrow();
  });

  test("rejects arbitrary imports and unbounded guest memory before instantiation", async () => {
    // Minimal Wasm modules: imported env.secret function, and unbounded local memory.
    const secret = new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,2,14,1,3,101,110,118,6,115,101,99,114,101,116,0,0]);
    expect(() => validateGuestModule(secret)).toThrow();
    const unbounded = new Uint8Array([0,97,115,109,1,0,0,0,5,3,1,0,1]);
    expect(() => validateGuestModule(unbounded)).toThrow();
  });

  test("deep links bootstrap privately then commit the selected page", async () => {
    const f = fixture();
    const reply = await f.runtime.handle({ id: 1, type: "init", route: "/live" });
    expect(reply).toMatchObject({ id: 1, type: "commit", route: "/live", root: { id: "live#root" } });
    expect(f.events()).toBe(1);
  });

  test("queues guest navigation outside guest calls and rejects hidden user targets", async () => {
    const f = fixture();
    await f.runtime.handle({ id: 1, type: "init", route: "/" });
    expect(await f.runtime.handle({ id: 2, type: "input", node_id: "timer-tick", event: "pressed" })).toMatchObject({ type: "error" });
    expect(f.events()).toBe(0);
    f.output([{ type: "navigate", payload: { operation: "push", route: "/live" } }]);
    expect(await f.runtime.handle({ id: 3, type: "input", node_id: "go", event: "pressed" })).toMatchObject({ type: "commit", route: "/live", history_operation: "push" });
    expect(f.events()).toBe(2);
  });

  test("idle polls never call the guest; rejected navigation sends rollback", async () => {
    const f = fixture();
    await f.runtime.handle({ id: 1, type: "init", route: "/" });
    expect(await f.runtime.handle({ id: 2, type: "poll" })).toEqual({ id: 2, type: "idle" });
    expect(f.events()).toBe(0);
    f.output([{ type: "navigate", payload: { operation: "push", route: "/denied" } }]);
    expect(await f.runtime.handle({ id: 3, type: "input", node_id: "go", event: "pressed" })).toMatchObject({ type: "idle" });
    expect(f.events()).toBe(2);
  });

  test("invalid owner response retains the last committed document", async () => {
    const f = fixture();
    const initial = await f.runtime.handle({ id: 1, type: "init", route: "/" });
    expect(initial.type).toBe("commit");
    if (initial.type !== "commit") throw new Error("initial commit missing");
    f.failMount();
    expect(await f.runtime.handle({ id: 2, type: "navigate", command: { operation: "push", route: "/live" } })).toMatchObject({ type: "error" });
    expect(f.runtime.committed).toEqual(initial);
    expect(f.canceled()).toBe(1);
  });

  test("timer callbacks recheck completion after each dispatch and stop on navigation", async () => {
    const f = fixture();
    f.configuration.timers.pages = [{ id: "home", timers: [{ id: "timer", tickNode: "tick", completeNode: "complete", statusNode: "status", intervalMs: 1000, completionDelayMs: 100 }] }];
    await f.runtime.handle({ id: 1, type: "init", route: "/" });
    let polls = 0;
    const observed: string[] = [];
    f.core.observe = (_now, completed) => { observed.push(completed); };
    f.core.poll_one = (_now, completed) => {
      polls++;
      if (polls === 1) return JSON.stringify({ node_id: "tick", event: "pressed" });
      expect(completed).toBe('["timer"]');
      return "null";
    };
    f.core.timer_allowed = () => true;
    const completedRoot = root();
    completedRoot.children.push({ id: "status", kind: "text", props: { text: "complete" }, children: [] });
    f.output([{ type: "patch", payload: { root: completedRoot } }]);
    expect(await f.runtime.handle({ id: 2, type: "poll" })).toMatchObject({ type: "commit" });
    expect(observed).toContain('["timer"]');
    polls = 0;
    f.core.poll_one = () => { polls++; return JSON.stringify({ node_id: "complete", event: "pressed" }); };
    f.output([{ type: "navigate", payload: { operation: "replace", route: "/live" } }]);
    expect(await f.runtime.handle({ id: 3, type: "poll" })).toMatchObject({ type: "commit", route: "/live" });
    expect(polls).toBe(1);
  });

  test("guest queue overflow and unsupported actions never replace the committed document", async () => {
    for (const messages of [Array.from({ length: 33 }, () => ({ type: "patch", payload: {} })), [{ type: "action", payload: { kind: "shell" } }]]) {
      const f = fixture();
      await f.runtime.handle({ id: 1, type: "init", route: "/" });
      const committed = f.runtime.committed;
      f.output(messages);
      expect(await f.runtime.handle({ id: 2, type: "input", node_id: "go", event: "pressed" })).toMatchObject({ type: "error" });
      expect(f.runtime.committed).toEqual(committed);
    }
  });

  test("projected images require exact manifest admission before document commit", async () => {
    const f = fixture();
    await f.runtime.handle({ id: 1, type: "init", route: "/" });
    const committed = f.runtime.committed;
    const unsafe = root();
    unsafe.children.push({ id: "image", kind: "image", props: { asset: "assets/undeclared.svg" }, children: [] });
    f.output([{ type: "patch", payload: { root: unsafe } }]);
    expect(await f.runtime.handle({ id: 2, type: "input", node_id: "go", event: "pressed" })).toMatchObject({ type: "error" });
    expect(f.runtime.committed).toEqual(committed);
  });
});

test('static-host trailing slashes canonicalize before shared Rust route resolution', async () => {
  const f = fixture();
  expect(await f.runtime.handle({id:1,type:'init',route:'/live/'})).toMatchObject({type:'commit',route:'/live'});
});
