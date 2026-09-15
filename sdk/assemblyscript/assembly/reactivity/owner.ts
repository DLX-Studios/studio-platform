import { OwnedReactive, collectOwned } from "./scheduler";

/** Lifetime boundary for all reactive computations created by one interface. */
export class ReactiveOwner {
  private owned: OwnedReactive[];
  private disposed: bool = false;

  constructor(owned: OwnedReactive[]) {
    this.owned = owned;
  }

  /** Dispose all owned computations in reverse creation order. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (let index = this.owned.length - 1; index >= 0; index -= 1) {
      this.owned[index].dispose();
    }
    this.owned.length = 0;
  }
}

/** Capture reactive computations created by a mounted interface. */
export function createOwner(initialize: () => void): ReactiveOwner {
  return new ReactiveOwner(collectOwned(initialize));
}
