import { describe, expect, test } from "bun:test";
import {
  ReactiveCycleError,
  $derived,
  $effect,
  $state,
  batch,
  createOwner,
  flushEffects,
  setFlushLimit,
} from "../assembly/reactivity";

describe("AssemblyScript-compatible fine-grained reactivity", () => {
  test("tracks dynamic state dependencies and memoizes derived values", () => {
    const usePrimary = $state(true);
    const primary = $state(2);
    const secondary = $state(10);
    let computations = 0;
    const selected = $derived(() => {
      computations += 1;
      return usePrimary.value ? primary.value : secondary.value;
    });

    expect(selected.value).toBe(2);
    expect(selected.value).toBe(2);
    expect(computations).toBe(1);
    secondary.value = 11;
    expect(selected.value).toBe(2);
    expect(computations).toBe(1);
    primary.value = 3;
    expect(selected.value).toBe(3);
    expect(computations).toBe(2);
    usePrimary.value = false;
    expect(selected.value).toBe(11);
    expect(computations).toBe(3);
    primary.value = 4;
    expect(selected.value).toBe(11);
    expect(computations).toBe(3);
  });

  test("batches writes, runs effects once, and preserves registration order", () => {
    const quantity = $state(1);
    const unitPrice = $state(3500);
    const total = $derived(() => quantity.value * unitPrice.value);
    const log: string[] = [];
    $effect(() => log.push(`first:${total.value}`));
    $effect(() => log.push(`second:${total.value}`));
    expect(log).toEqual(["first:3500", "second:3500"]);

    batch(() => {
      quantity.value = 2;
      quantity.value = 3;
      unitPrice.value = 3000;
      expect(log).toHaveLength(2);
    });
    expect(log).toEqual(["first:3500", "second:3500", "first:9000", "second:9000"]);
  });

  test("flushes explicit pending work deterministically", () => {
    const value = $state(0);
    const observed: number[] = [];
    $effect(() => observed.push(value.value));
    value.value = 1;
    value.value = 2;
    flushEffects();
    expect(observed).toEqual([0, 2]);
  });

  test("disposes every derived/effect owned by a removed interface", () => {
    const value = $state(1);
    const observed: number[] = [];
    const owner = createOwner(() => {
      const doubled = $derived(() => value.value * 2);
      $effect(() => observed.push(doubled.value));
    });
    expect(observed).toEqual([2]);
    owner.dispose();
    value.value = 2;
    flushEffects();
    expect(observed).toEqual([2]);
  });

  test("fails bounded reactive cycles without freezing later work", () => {
    setFlushLimit(8);
    const looping = $state(0);
    const owner = createOwner(() => {
      $effect(() => {
        looping.value += 1;
      });
    });
    expect(() => flushEffects()).toThrow(ReactiveCycleError);
    owner.dispose();

    const healthy = $state(1);
    const observed: number[] = [];
    $effect(() => observed.push(healthy.value));
    healthy.value = 2;
    flushEffects();
    expect(observed.at(-1)).toBe(2);
    setFlushLimit(100);
  });
});
