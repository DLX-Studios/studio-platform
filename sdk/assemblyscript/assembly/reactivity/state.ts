import { ReactiveSource, ReactiveSubscriber, trackDependency } from "./scheduler";

/** Mutable reactive state cell. */
export class State<T> implements ReactiveSource {
  private current: T;
  private subscribers: ReactiveSubscriber[] = [];

  constructor(initial: T) {
    this.current = initial;
  }

  get value(): T {
    trackDependency(this);
    return this.current;
  }

  set value(next: T) {
    this.current = next;
    for (const subscriber of this.subscribers) subscriber.notify();
  }

  addSubscriber(subscriber: ReactiveSubscriber): void {
    if (this.subscribers.indexOf(subscriber) < 0) this.subscribers.push(subscriber);
  }

  removeSubscriber(subscriber: ReactiveSubscriber): void {
    const index = this.subscribers.indexOf(subscriber);
    if (index >= 0) this.subscribers.splice(index, 1);
  }
}

/** Create a mutable reactive state cell. */
export function $state<T>(initial: T): State<T> {
  return new State(initial);
}
