import { $effect, type Effect } from "./reactivity";
import { bindingsFor } from "./bindings";
import { ActionCorrelation } from "./actions";
import { HostEventRegistry, LifecycleRuntime } from "./events";
import { NavigationCorrelation } from "./navigation";

export type WidgetProps = Map<string, unknown>;
export interface Widget {
  id: string;
  kind: string;
  props: WidgetProps;
  children: Widget[];
}

export type PatchOperation =
  | {
      op: "update_prop";
      node_id: string;
      property: string;
      value: unknown;
    }
  | { op: "insert_child"; parent_id: string; index: number; node: Widget }
  | { op: "remove_node"; node_id: string }
  | { op: "replace_node"; node_id: string; node: Widget };

export interface MountMessage {
  type: "mount";
  payload: { protocol_version: 1; route: string; root: Widget };
}

export interface PatchMessage {
  type: "patch";
  payload: { sequence: number; operations: PatchOperation[] };
}

export type UiMessage = MountMessage | PatchMessage;

/** Guest-side retained UI runtime and ordered patch emitter. */
export class StudioUiRuntime {
  readonly events = new HostEventRegistry();
  readonly navigation = new NavigationCorrelation();
  readonly actions = new ActionCorrelation();
  readonly lifecycle = new LifecycleRuntime();
  private readonly emit: (message: UiMessage) => void;
  private readonly nodes = new Map<string, Widget>();
  private readonly bindingEffects: Effect[] = [];
  private sequence = 0;
  private transactionDepth = 0;
  private pendingOperations: PatchOperation[] = [];
  private mounted = false;

  constructor(emit: (message: UiMessage) => void) {
    this.emit = emit;
  }

  /** Validate and emit the interface's one initial retained tree. */
  mount(route: string, root: Widget): void {
    if (this.mounted) throw new Error("runtime is already mounted");
    const discovered = collectUnique(root);
    for (const [id, widget] of discovered) this.nodes.set(id, widget);
    this.emit({
      type: "mount",
      payload: { protocol_version: 1, route, root: cloneWidget(root) },
    });
    this.mounted = true;
    this.installBindings(root);
  }

  /** Batch structural changes into one ordered patch envelope. */
  transaction<T>(run: () => T): T {
    this.requireMounted();
    this.transactionDepth += 1;
    try {
      return run();
    } finally {
      this.transactionDepth -= 1;
      if (this.transactionDepth === 0) this.flushOperations();
    }
  }

  /** Insert a child at a stable parent/index target. */
  insertChild(parentId: string, index: number, node: Widget): void {
    this.requireNode(parentId);
    const discovered = collectUnique(node);
    for (const id of discovered.keys()) {
      if (this.nodes.has(id)) throw new Error(`duplicate widget id: ${id}`);
    }
    for (const [id, widget] of discovered) this.nodes.set(id, widget);
    this.queue({
      op: "insert_child",
      parent_id: parentId,
      index,
      node: cloneWidget(node),
    });
    this.installBindings(node);
  }

  /** Remove a retained node by stable ID. */
  removeNode(nodeId: string): void {
    const node = this.requireNode(nodeId);
    for (const id of collectUnique(node).keys()) this.nodes.delete(id);
    this.queue({ op: "remove_node", node_id: nodeId });
  }

  /** Replace a retained node while preserving the ordered patch target. */
  replaceNode(nodeId: string, node: Widget): void {
    const previous = this.requireNode(nodeId);
    const removed = collectUnique(previous);
    const replacement = collectUnique(node);
    for (const id of replacement.keys()) {
      if (this.nodes.has(id) && !removed.has(id)) throw new Error(`duplicate widget id: ${id}`);
    }
    for (const id of removed.keys()) this.nodes.delete(id);
    for (const [id, widget] of replacement) this.nodes.set(id, widget);
    this.queue({ op: "replace_node", node_id: nodeId, node: cloneWidget(node) });
    this.installBindings(node);
  }

  private installBindings(root: Widget): void {
    walk(root, (widget) => {
      for (const binding of bindingsFor(widget)) {
        let initial = true;
        const effect = $effect(() => {
          const value = binding.read();
          if (initial) {
            initial = false;
            if (Object.is(widget.props.get(binding.property), value)) return;
          }
          widget.props.set(binding.property, value);
          this.queue({
            op: "update_prop",
            node_id: widget.id,
            property: binding.property,
            value,
          });
        });
        this.bindingEffects.push(effect);
      }
    });
  }

  private queue(operation: PatchOperation): void {
    this.pendingOperations.push(operation);
    if (this.transactionDepth === 0) this.flushOperations();
  }

  private flushOperations(): void {
    if (this.pendingOperations.length === 0) return;
    this.sequence += 1;
    const operations = this.pendingOperations;
    this.pendingOperations = [];
    this.emit({ type: "patch", payload: { sequence: this.sequence, operations } });
  }

  private requireNode(nodeId: string): Widget {
    const node = this.nodes.get(nodeId);
    if (node === undefined) throw new Error(`unknown widget id: ${nodeId}`);
    return node;
  }

  private requireMounted(): void {
    if (!this.mounted) throw new Error("runtime is not mounted");
  }
}

function collectUnique(root: Widget): Map<string, Widget> {
  const result = new Map<string, Widget>();
  walk(root, (widget) => {
    if (result.has(widget.id)) throw new Error(`duplicate widget id: ${widget.id}`);
    result.set(widget.id, widget);
  });
  return result;
}

function walk(root: Widget, visit: (widget: Widget) => void): void {
  visit(root);
  for (const child of root.children) walk(child, visit);
}

function cloneWidget(widget: Widget): { id: string; kind: string; props: Record<string, unknown>; children: ReturnType<typeof cloneWidget>[] } {
  const props: Record<string, unknown> = {};
  widget.props.forEach((v, k) => { props[k] = v; });
  return {
    id: widget.id,
    kind: widget.kind,
    props,
    children: widget.children.map(cloneWidget),
  };
}
