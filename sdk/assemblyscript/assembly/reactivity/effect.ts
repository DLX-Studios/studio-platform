import {
  OwnedReactive,
  ReactiveEffect,
  ReactiveSource,
  registerOwned,
  scheduleEffect,
  withSubscriberVoid,
} from "./scheduler";

/** Fine-grained side effect with dynamic dependency tracking. */
export class Effect implements ReactiveEffect, OwnedReactive {
  private callback: () => void;
  private dependencies: ReactiveSource[] = [];
  private disposed: bool = false;

  constructor(callback: () => void) {
    this.callback = callback;
    registerOwned(this);
    this.execute();
  }

  addDependency(source: ReactiveSource): void {
    if (this.dependencies.indexOf(source) < 0) this.dependencies.push(source);
  }

  notify(): void {
    scheduleEffect(this);
  }

  runScheduled(): void {
    this.execute();
  }

  isDisposed(): boolean {
    return this.disposed;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.clearDependencies();
  }

  private execute(): void {
    if (this.disposed) return;
    this.clearDependencies();
    withSubscriberVoid(this, this.callback);
  }

  private clearDependencies(): void {
    for (let index = 0; index < this.dependencies.length; index += 1) {
      this.dependencies[index].removeSubscriber(this);
    }
    this.dependencies.length = 0;
  }
}

/** Register and immediately run a fine-grained effect. */
export function $effect(callback: () => void): Effect {
  return new Effect(callback);
}
