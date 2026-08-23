export type UiEventHandler = (payloadJson: string) => void;
export type StringEventHandler = (value: string) => void;
export type BooleanEventHandler = (value: bool) => void;
export type NumberEventHandler = (value: f64) => void;
export type VoidEventHandler = () => void;

/** Stable callback token emitted into a widget's protocol properties. */
export class CallbackToken {
  readonly id: string;
  constructor(id: string) {
    if (id.length == 0) throw new Error("callback id required");
    this.id = id;
  }
}

/** Exact node/event registration for non-secret host events. */
export class HostEventRegistry {
  private readonly handlers: Map<string, UiEventHandler> = new Map<string, UiEventHandler>();

  on(nodeId: string, event: string, handler: UiEventHandler): void {
    const key = eventKey(nodeId, event);
    if (this.handlers.has(key)) throw new Error("event handler already registered");
    this.handlers.set(key, handler);
  }

  dispatch(nodeId: string, event: string, payloadJson: string): boolean {
    const handler = this.handlers.get(eventKey(nodeId, event));
    if (handler === undefined) return false;
    handler(payloadJson);
    return true;
  }

  clear(): void { this.handlers.clear(); }

  onValueChanged(nodeId: string, handler: StringEventHandler): CallbackToken {
    const id = callbackId(nodeId, "value_changed");
    this.on(nodeId, "value_changed", (payload: string): void => handler(payload));
    return new CallbackToken(id);
  }

  onCheckedChanged(nodeId: string, handler: BooleanEventHandler): CallbackToken {
    const id = callbackId(nodeId, "checked_changed");
    this.on(nodeId, "checked_changed", (payload: string): void => handler(payload == "true"));
    return new CallbackToken(id);
  }

  onNumberChanged(nodeId: string, handler: NumberEventHandler): CallbackToken {
    const id = callbackId(nodeId, "value_changed");
    this.on(nodeId, "value_changed", (payload: string): void => handler(parseFloat(payload)));
    return new CallbackToken(id);
  }

  onPressed(nodeId: string, handler: VoidEventHandler): CallbackToken {
    const id = callbackId(nodeId, "pressed");
    this.on(nodeId, "pressed", (_payload: string): void => handler());
    return new CallbackToken(id);
  }
}

/** Closed host-controlled lifecycle state machine. */
export class LifecycleRuntime {
  state: string = "created";

  receive(next: string): void {
    const valid =
      (this.state == "created" && next == "loading") ||
      (this.state == "loading" && next == "running") ||
      (this.state == "running" && (next == "terminated" || next == "stopped"));
    if (!valid) throw new Error("invalid lifecycle transition");
    this.state = next;
  }
}

function eventKey(nodeId: string, event: string): string {
  if (nodeId.length == 0 || event.length == 0) throw new Error("event identity required");
  return nodeId + "\u0000" + event;
}

function callbackId(nodeId: string, event: string): string {
  if (nodeId.length == 0 || event.length == 0) throw new Error("callback identity required");
  return "cb_" + nodeId + "_" + event;
}
