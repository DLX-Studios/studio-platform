import { describe, expect, test } from "bun:test";
import { ActionCorrelation } from "../assembly/actions";
import { HostEventRegistry, LifecycleRuntime } from "../assembly/events";
import { NavigationCommand, NavigationCorrelation, NavigationResult } from "../assembly/navigation";

describe("bounded SDK host helpers", () => {
  test("dispatches typed non-secret UI events only to exact registrations", () => {
    const events: string[] = [];
    const registry = new HostEventRegistry();
    registry.on("checkout", "pressed", (payload) => events.push(payload));
    expect(registry.dispatch("checkout", "pressed", "{}")).toBe(true);
    expect(registry.dispatch("other", "pressed", "{}")).toBe(false);
    expect(events).toEqual(["{}"]);
  });

  test("correlates navigation and asynchronous actions and enforces 16 pending requests", () => {
    const navigation = new NavigationCorrelation();
    navigation.begin(NavigationCommand.push("/cart"));
    navigation.resolve(new NavigationResult("/cart", true));
    expect(navigation.currentRoute).toBe("/cart");

    const actions = new ActionCorrelation();
    for (let index = 0; index < 16; index += 1) actions.begin(`request-${index}`);
    expect(() => actions.begin("overflow")).toThrow("pending action limit exceeded");
    expect(actions.resolve("request-4", "approved")).toBe("approved");
    expect(actions.pendingCount).toBe(15);
    expect(() => actions.resolve("missing", "x")).toThrow("unknown action request");
  });

  test("accepts only host lifecycle ordering and clears pending work on termination", () => {
    const runtime = new LifecycleRuntime();
    runtime.receive("loading");
    runtime.receive("running");
    runtime.receive("terminated");
    expect(runtime.state).toBe("terminated");
    expect(() => runtime.receive("running")).toThrow("invalid lifecycle transition");
  });
});
