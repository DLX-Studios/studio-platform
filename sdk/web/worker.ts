import type { NavigationCommand, UiNode, WebConfig, WorkerReply, WorkerRequest } from "./worker-protocol";

const MAX_MESSAGE = 65_536;
const MAX_MESSAGES = 32;
const MAX_MEMORY_PAGES = 256;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

interface Selection { route: string; id: string; params: Record<string, string> }
interface Core {
  current(): string;
  navigate(command: string, now: number): string;
  cancel_navigation(): boolean;
  accept(message: string): string;
  tree(): string;
  can_input(node: string, event: string): boolean;
  timer_allowed(node: string, event: string): boolean;
  observe(now: number, completed: string): void;
  poll_one(now: number, completed: string): string;
}
interface Guest {
  memory: WebAssembly.Memory;
  studio_alloc(length: number): number;
  studio_dealloc(pointer: number, length: number): void;
  studio_init(pointer: number, length: number): number;
  studio_event(pointer: number, length: number): number;
}
interface GuestImports {
  studio_host: { emit(pointer: number, length: number): number };
  env: { abort(message: number, file: number, line: number, column: number): never };
}
export interface WorkerDependencies {
  loadConfig(): Promise<WebConfig>;
  loadCore(config: WebConfig, initial: string, now: number): Promise<Core>;
  loadGuest(imports: GuestImports): Promise<Guest>;
  now(): number;
}

/** Copy before decoding: neither guest mutation nor memory growth can change a queued message. */
export function decodeGuestBuffer(memory: WebAssembly.Memory, pointer: number, length: number): string {
  if (!Number.isInteger(pointer) || !Number.isInteger(length) || pointer < 0 || length < 0 || length > MAX_MESSAGE) {
    throw new Error("Guest message exceeds the bounded buffer contract");
  }
  if (pointer > memory.buffer.byteLength || length > memory.buffer.byteLength - pointer) {
    throw new Error("Guest message lies outside linear memory");
  }
  return decoder.decode(new Uint8Array(memory.buffer, pointer, length).slice());
}

/** Inspect the binary memory declaration as JS reflection omits its maximum. */
export function validateGuestModule(bytes: Uint8Array): WebAssembly.Module {
  if (bytes.byteLength > 16 * 1024 * 1024) throw new Error("Guest module is too large");
  const module = new WebAssembly.Module(bytes as BufferSource);
  for (const entry of WebAssembly.Module.imports(module)) {
    if (entry.kind !== "function" || !((entry.module === "studio_host" && entry.name === "emit") || (entry.module === "env" && entry.name === "abort"))) {
      throw new Error("Unsupported guest import");
    }
  }
  let position = 8;
  const read = (end = bytes.length): number => {
    let value = 0;
    for (let shift = 0; shift < 35 && position < end; shift += 7) {
      const byte = bytes[position++]!;
      value += (byte & 127) * 2 ** shift;
      if ((byte & 128) === 0) {
        if (value > 0xffff_ffff) throw new Error("Invalid Wasm bound");
        return value;
      }
    }
    throw new Error("Invalid Wasm integer");
  };
  let memories = 0;
  while (position < bytes.length) {
    const section = bytes[position++]!;
    const length = read();
    const end = position + length;
    if (end > bytes.length) throw new Error("Invalid Wasm section");
    if (section === 5) {
      memories = read(end);
      if (memories !== 1 || read(end) !== 1) throw new Error("One bounded, unshared guest memory is required");
      const minimum = read(end);
      const maximum = read(end);
      if (minimum > maximum || maximum > MAX_MEMORY_PAGES || position !== end) throw new Error("Guest memory exceeds the web limit");
    }
    position = end;
  }
  if (memories !== 1) throw new Error("Guest memory is missing");
  const exports = WebAssembly.Module.exports(module);
  for (const name of ["studio_init", "studio_event", "studio_alloc", "studio_dealloc", "memory"]) {
    if (!exports.some((entry) => entry.name === name && entry.kind === (name === "memory" ? "memory" : "function"))) {
      throw new Error(`Required guest export ${name} is missing`);
    }
  }
  return module;
}

export class WorkerRuntime {
  committed: Extract<WorkerReply, { type: "commit" }> | undefined;
  private config!: WebConfig;
  private core!: Core;
  private guest!: Guest;
  private queue: string[] = [];
  private calling = false;
  private busy = false;
  private failed = false;
  private changed = false;
  private history: NavigationCommand["operation"] | null = null;
  private now = 0;

  constructor(private readonly dependencies: WorkerDependencies) {}

  async handle(request: WorkerRequest): Promise<WorkerReply> {
    const id = request?.id;
    if (!Number.isSafeInteger(id) || id < 0) return { id, type: "error", message: "Invalid request identifier" };
    if (this.busy) return { id, type: "error", message: "A worker request is already active" };
    this.busy = true;
    this.changed = false;
    this.history = null;
    try {
      if (this.failed) throw new Error("The guest stopped after an invalid operation; reload to restart");
      const time = Math.floor(this.dependencies.now());
      if (!Number.isSafeInteger(time) || time < this.now) throw new Error("Invalid monotonic host time");
      this.now = time;
      switch (request.type) {
        case "init": await this.initialize(request.route); break;
        case "input":
          this.ready();
          if (request.event !== "pressed" || !this.core.can_input(request.node_id, request.event)) throw new Error("Input target is not an enabled visible handler");
          this.dispatch({ type: "ui", payload: { node_id: request.node_id, event: request.event, payload: {} } });
          this.applyMessages();
          this.observe();
          break;
        case "navigate": this.ready(); this.navigate(request.command); break;
        case "poll": this.ready(); this.poll(); break;
        default: throw new Error("Unknown worker request");
      }
      if (!this.changed) return { id, type: "idle" };
      const root = this.readTree();
      const selection: Selection = JSON.parse(this.core.current());
      const reply = { id, type: "commit" as const, root, route: selection.route, history_operation: this.history };
      this.committed = reply;
      return reply;
    } catch (error) {
      return { id, type: "error", message: error instanceof Error ? error.message : String(error) };
    } finally {
      this.busy = false;
    }
  }

  private ready(): void {
    if (!this.guest) throw new Error("Worker is not initialized");
  }

  private async initialize(route: string): Promise<void> {
    route = route === "/" ? "/" : route.replace(/\/+$/, "");
    if (this.guest) throw new Error("Worker is already initialized");
    this.config = await this.dependencies.loadConfig();
    if (this.config.schemaVersion !== 1 || !Array.isArray(this.config.assets)) throw new Error("Invalid web configuration");
    const declarations = this.config.routing.routes;
    const initial = declarations.find((entry) => entry.path === "/")?.path
      ?? declarations.find((entry) => !entry.path.includes(":") && !entry.path.includes("*"))?.path;
    if (!initial) throw new Error("A static startup route is required");
    this.core = await this.dependencies.loadCore(this.config, initial, this.now);
    this.guest = await this.dependencies.loadGuest({
      studio_host: { emit: (pointer, length) => {
        if (!this.calling || !this.guest) throw new Error("Guest emit outside a host invocation");
        if (this.queue.length >= MAX_MESSAGES) throw new Error("Guest message queue exceeded");
        this.queue.push(decodeGuestBuffer(this.guest.memory, pointer, length));
        return 0;
      } },
      env: { abort: (_message, _file, line, column) => { throw new Error(`Guest aborted at ${line}:${column}`); } },
    });
    this.invoke("studio_init", "{}");
    this.applyMessages(true);
    if (route !== initial) this.navigate({ operation: "replace", route }, true);
    this.observe();
  }

  private invoke(method: "studio_init" | "studio_event", json: string): void {
    const bytes = encoder.encode(json);
    if (bytes.length > MAX_MESSAGE || this.calling) throw new Error("Invalid guest invocation");
    this.queue = [];
    this.calling = true;
    let pointer: number | undefined;
    try {
      pointer = this.guest.studio_alloc(bytes.length);
      if (!Number.isInteger(pointer) || pointer <= 0 || pointer > this.guest.memory.buffer.byteLength - bytes.length) throw new Error("Invalid guest allocation");
      new Uint8Array(this.guest.memory.buffer, pointer, bytes.length).set(bytes);
      const status = this.guest[method](pointer, bytes.length);
      if (status !== 0) throw new Error(`Guest rejected ${method}`);
    } catch (error) {
      this.failed = true;
      throw error;
    } finally {
      try {
        if (pointer !== undefined) this.guest.studio_dealloc(pointer, bytes.length);
      } finally { this.calling = false; }
    }
  }

  private dispatch(event: unknown): void {
    this.invoke("studio_event", JSON.stringify(event));
  }

  private applyMessages(allowMount = false): void {
    const messages = this.queue;
    this.queue = [];
    try {
      if (messages.length === 0 && allowMount) throw new Error("Guest omitted the authorized mount");
      if (allowMount && (messages.length !== 1 || JSON.parse(messages[0]!).type !== "mount")) throw new Error("Guest omitted the authorized mount");
      for (const json of messages) {
        const message = JSON.parse(json);
        if (message.type === "navigate") {
          if (messages.length !== 1 || allowMount) throw new Error("Navigation must be the only guest response");
          this.navigate(message.payload);
          return;
        }
        if ((message.type === "mount" && (!allowMount || messages.length !== 1)) || !["mount", "patch"].includes(message.type)) {
          throw new Error("Unsupported or unsolicited guest message");
        }
        this.core.accept(json);
        this.readTree();
        this.changed = true;
      }
    } catch (error) {
      this.failed = true;
      throw error;
    }
  }

  private navigate(command: NavigationCommand, requireSuccess = false): void {
    let selection: Selection;
    try {
      selection = JSON.parse(this.core.navigate(JSON.stringify(command), this.now));
    } catch (error) {
      const current: Selection = JSON.parse(this.core.current());
      this.dispatch({ type: "navigation", payload: { route: current.route, accepted: false, error_code: "navigation_rejected" } });
      if (this.queue.length) { this.failed = true; throw new Error("Rejected navigation emitted guest mutations"); }
      if (requireSuccess) throw error;
      return;
    }
    try {
      this.dispatch({ type: "navigation", payload: { route: selection.route, accepted: true, error_code: null } });
      this.applyMessages(true);
      this.history = command.operation;
      this.observe();
    } catch (error) {
      this.core.cancel_navigation();
      throw error;
    }
  }

  private readTree(): UiNode {
    const root: UiNode = JSON.parse(this.core.tree());
    const admitted = new Set(this.config.assets);
    const pending = [root];
    while (pending.length) {
      const node = pending.pop()!;
      for (const key of ["asset", "background_asset"]) {
        const path = node.props[key];
        if (path !== undefined && (typeof path !== "string" || !admitted.has(path))) throw new Error("Tree references an asset outside the manifest");
      }
      pending.push(...node.children);
    }
    return root;
  }

  private completed(): string {
    const selection: Selection = JSON.parse(this.core.current());
    const timers = this.config.timers.pages.find((page) => page.id === selection.id)?.timers ?? [];
    const nodes = new Map<string, UiNode>();
    const pending = [this.readTree()];
    while (pending.length) { const node = pending.pop()!; nodes.set(node.id, node); pending.push(...node.children); }
    return JSON.stringify(timers.filter((timer) => nodes.get(timer.statusNode)?.props.text === "complete").map((timer) => timer.id));
  }

  private observe(): void { this.core.observe(this.now, this.completed()); }

  private poll(): void {
    const route = this.core.current();
    for (let count = 0; count < MAX_MESSAGES; count++) {
      const event = JSON.parse(this.core.poll_one(this.now, this.completed()));
      if (event === null) break;
      if (!this.core.timer_allowed(event.node_id, event.event)) throw new Error("Timer target is not owned by the active page");
      this.dispatch({ type: "ui", payload: { ...event, payload: {} } });
      this.applyMessages();
      this.observe();
      if (this.core.current() !== route || this.history !== null) break;
    }
  }
}

async function checkedFetch(path: string): Promise<Response> {
  const response = await fetch(new URL(path, self.location.origin));
  if (!response.ok) throw new Error(`Unable to load ${path}`);
  return response;
}

const browserDependencies: WorkerDependencies = {
  loadConfig: async () => (await checkedFetch("/web-config.json")).json(),
  loadCore: async (config, initial, now) => {
    const coreUrl = new URL("/core/studio_web_core.js", self.location.origin);
    const bridge = await import(coreUrl.href);
    await bridge.default();
    return new bridge.WebHost(JSON.stringify(config.routing), JSON.stringify(config.timers), initial, now) as Core;
  },
  loadGuest: async (imports) => {
    const bytes = new Uint8Array(await (await checkedFetch("/module.wasm")).arrayBuffer());
    const module = validateGuestModule(bytes);
    const instance = await WebAssembly.instantiate(module, imports as unknown as WebAssembly.Imports);
    return instance.exports as unknown as Guest;
  },
  now: () => performance.now(),
};

const workerScopeType = (globalThis as unknown as { WorkerGlobalScope?: new () => object }).WorkerGlobalScope;
if (workerScopeType && globalThis instanceof workerScopeType) {
  const runtime = new WorkerRuntime(browserDependencies);
  self.onmessage = (event: MessageEvent<WorkerRequest>) => { void runtime.handle(event.data).then((reply) => self.postMessage(reply)); };
}
