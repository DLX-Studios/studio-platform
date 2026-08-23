import {
  OwnedReactive,
  ReactiveSource,
  ReactiveSubscriber,
  registerOwned,
  trackDependency,
  withSubscriber,
} from "./scheduler";

/** Lazily memoized value with dynamically tracked dependencies. */
export class Derived<T> implements ReactiveSource, ReactiveSubscriber, OwnedReactive {
  private compute: () => T;
  private dependencies: ReactiveSource[] = [];
  private subscribers: ReactiveSubscriber[] = [];
  private cached: T;
  private dirty: bool = true;
  private disposed: bool = false;

  constructor(compute: () => T) {
    this.compute = compute;
    registerOwned(this);
  }

  get value(): T {
    trackDependency(this);
    if (this.dirty) this.recompute();
    return this.cached;
  }

  addSubscriber(subscriber: ReactiveSubscriber): void {
    if (this.subscribers.indexOf(subscriber) < 0) this.subscribers.push(subscriber);
  }

  removeSubscriber(subscriber: ReactiveSubscriber): void {
    const index = this.subscribers.indexOf(subscriber);
    if (index >= 0) this.subscribers.splice(index, 1);
  }

  addDependency(source: ReactiveSource): void {
    if (this.dependencies.indexOf(source) < 0) this.dependencies.push(source);
  }

  notify(): void {
    if (this.dirty || this.disposed) return;
    this.dirty = true;
    for (const subscriber of this.subscribers) subscriber.notify();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.clearDependencies();
    this.subscribers.length = 0;
  }

  private recompute(): void {
    if (this.disposed) return;
    this.clearDependencies();
    this.cached = withSubscriber<T>(this, this.compute);
    this.dirty = false;
  }

  private clearDependencies(): void {
    for (let index = 0; index < this.dependencies.length; index += 1) {
      this.dependencies[index].removeSubscriber(this);
    }
    this.dependencies.length = 0;
  }
}

/** Create a lazily memoized derived value. */
export function $derived<T>(compute: () => T): Derived<T> {
  return new Derived(compute);
}
