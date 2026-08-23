/** A reactive source that can detach a dependent computation. */
export interface ReactiveSource {
  addSubscriber(subscriber: ReactiveSubscriber): void;
  removeSubscriber(subscriber: ReactiveSubscriber): void;
}

/** A computation that can depend on reactive sources. */
export interface ReactiveSubscriber {
  addDependency(source: ReactiveSource): void;
  notify(): void;
}

/** Disposable work registered to the currently active interface owner. */
export interface OwnedReactive {
  dispose(): void;
}

/** Error raised when reactive work exceeds the bounded flush limit. */
export class ReactiveCycleError extends Error {
  constructor() {
    super("reactive flush limit exceeded");
    this.name = "ReactiveCycleError";
  }
}

let activeSubscriber: ReactiveSubscriber | null = null;
let ownerCollector: OwnedReactive[] | null = null;
let batchDepth: i32 = 0;
let flushLimit: i32 = 100;
let flushing: bool = false;
const pending: ReactiveEffect[] = [];
const queued = new Set<ReactiveEffect>();

/** Minimal scheduler-facing shape implemented by effects. */
export interface ReactiveEffect extends ReactiveSubscriber {
  dispose(): void;
  runScheduled(): void;
  isDisposed(): boolean;
}

/** Track a source read by the active computation. */
export function trackDependency(source: ReactiveSource): void {
  if (activeSubscriber !== null) {
    source.addSubscriber(activeSubscriber);
    activeSubscriber.addDependency(source);
  }
}

/** Execute a computation while recording its source reads. */
export function withSubscriber<T>(subscriber: ReactiveSubscriber, run: () => T): T {
  const previous = activeSubscriber;
  activeSubscriber = subscriber;
  const result = run();
  activeSubscriber = previous;
  return result;
}

/** Execute a void computation while recording its source reads. */
export function withSubscriberVoid(subscriber: ReactiveSubscriber, run: () => void): void {
  const previous = activeSubscriber;
  activeSubscriber = subscriber;
  run();
  activeSubscriber = previous;
}

/** Register a reactive computation with the active owner, when present. */
export function registerOwned(value: OwnedReactive): void {
  const collector = ownerCollector;
  if (collector !== null) collector.push(value);
}

/** Capture all reactive computations created by one owner callback. */
export function collectOwned(run: () => void): OwnedReactive[] {
  const previous = ownerCollector;
  const collected: OwnedReactive[] = [];
  ownerCollector = collected;
  run();
  ownerCollector = previous;
  return collected;
}

/** Queue an effect once while retaining registration order. */
export function scheduleEffect(effect: ReactiveEffect): void {
  if (effect.isDisposed() || queued.has(effect)) return;
  queued.add(effect);
  pending.push(effect);
}

/** Flush queued effects deterministically, failing bounded cycles. */
export function flushEffects(): void {
  if (flushing || batchDepth > 0) return;
  flushing = true;
  let executions: i32 = 0;
  while (pending.length > 0) {
    const effect = pending.shift();
    queued.delete(effect);
    if (effect.isDisposed()) continue;
    executions += 1;
    if (executions > flushLimit) {
      pending.length = 0;
      queued.clear();
      flushing = false;
      throw new ReactiveCycleError();
    }
    effect.runScheduled();
  }
  flushing = false;
}

/** Configure the maximum number of effect executions in one flush. */
export function setFlushLimit(limit: i32): void {
  if (limit < 1) throw new RangeError("flush limit must be positive");
  flushLimit = limit;
}

/** Enter a nested batch. */
export function beginBatch(): void {
  batchDepth += 1;
}

/** Leave a nested batch and flush at the outer boundary. */
export function endBatch(): void {
  if (batchDepth < 1) throw new Error("reactive batch underflow");
  batchDepth -= 1;
  if (batchDepth === 0) flushEffects();
}
