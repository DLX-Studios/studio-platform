export interface UiNode {
  id: string;
  kind: string;
  props: Record<string, unknown>;
  children: UiNode[];
}

export interface NavigationCommand {
  operation: "push" | "replace" | "pop" | "pop_to" | "reset";
  route?: string;
}

export interface WebConfig {
  schemaVersion: 1;
  title: string;
  routing: { schemaVersion: number; routes: { id: string; path: string; source: string }[] };
  timers: { schemaVersion: number; pages: TimerPage[] };
  assets: string[];
}

export interface TimerPage {
  id: string;
  timers: { id: string; tickNode: string; completeNode: string; statusNode: string; intervalMs: number; completionDelayMs: number }[];
}

export type WorkerRequest = { id: number } & (
  | { type: "init"; route: string }
  | { type: "input"; node_id: string; event: "pressed" }
  | { type: "navigate"; command: NavigationCommand }
  | { type: "poll" }
);

export type WorkerReply =
  | { id: number; type: "commit"; root: UiNode; route: string; history_operation: NavigationCommand["operation"] | null }
  | { id: number; type: "idle" }
  | { id: number; type: "error"; message: string };
