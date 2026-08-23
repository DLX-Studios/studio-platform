import { describe, expect, test } from "bun:test";
import { $derived, $state, batch } from "../assembly/reactivity";
import { StudioUiRuntime } from "../assembly/runtime";
import { bindProp } from "../assembly/bindings";
import {
  Button,
  Column,
  Text,
} from "../assembly/widgets";

describe("AssemblyScript widget bindings", () => {
  test("emits one mount followed by one minimal dependent property patch", () => {
    const emissions: unknown[] = [];
    const priceMinor = $state(8500);
    const discountBasisPoints = $state(0);
    const formatted = $derived(() => {
      const discounted =
        priceMinor.value - Math.trunc((priceMinor.value * discountBasisPoints.value) / 10_000);
      return `$${(discounted / 100).toFixed(2)}`;
    });
    const price = Text("price", formatted.value);
    bindProp(price, "text", () => formatted.value);
    const root = Column("root", [
      price,
      Button("checkout", "Checkout", "checkout_pressed"),
    ]);
    const runtime = new StudioUiRuntime((message) => emissions.push(message));

    runtime.mount("/catalog", root);
    expect(emissions).toHaveLength(1);
    expect(emissions[0]).toMatchObject({
      type: "mount",
      payload: {
        protocol_version: 1,
        route: "/catalog",
        root: { id: "root", kind: "column" },
      },
    });

    batch(() => {
      discountBasisPoints.value = 1000;
      discountBasisPoints.value = 1500;
    });
    expect(emissions).toHaveLength(2);
    expect(emissions[1]).toEqual({
      type: "patch",
      payload: {
        sequence: 1,
        operations: [
          {
            op: "update_prop",
            node_id: "price",
            property: "text",
            value: "$72.25",
          },
        ],
      },
    });
  });

  test("coalesces bindings by node/property and batches structural operations", () => {
    const emissions: unknown[] = [];
    const label = $state("Cart (0)");
    const cartLabel = Text("cart-count", label.value);
    bindProp(cartLabel, "text", () => label.value);
    const runtime = new StudioUiRuntime((message) => emissions.push(message));
    runtime.mount("/catalog", Column("root", [cartLabel]));

    batch(() => {
      label.value = "Cart (1)";
      label.value = "Cart (2)";
    });
    runtime.transaction(() => {
      runtime.insertChild("root", 1, Text("subtotal", "$70.00"));
      runtime.replaceNode("cart-count", Text("cart-count", "2 items"));
      runtime.removeNode("subtotal");
    });

    expect(emissions).toHaveLength(3);
    expect(emissions[1]).toMatchObject({
      payload: {
        sequence: 1,
        operations: [{ op: "update_prop", node_id: "cart-count", value: "Cart (2)" }],
      },
    });
    expect(emissions[2]).toEqual({
      type: "patch",
      payload: {
        sequence: 2,
        operations: [
          {
            op: "insert_child",
            parent_id: "root",
            index: 1,
            node: {
              id: "subtotal",
              kind: "text",
              props: { text: "$70.00" },
              children: [],
            },
          },
          {
            op: "replace_node",
            node_id: "cart-count",
            node: {
              id: "cart-count",
              kind: "text",
              props: { text: "2 items" },
              children: [],
            },
          },
          { op: "remove_node", node_id: "subtotal" },
        ],
      },
    });
  });

  test("rejects duplicate stable IDs before emitting a mount", () => {
    const emissions: unknown[] = [];
    const runtime = new StudioUiRuntime((message) => emissions.push(message));
    expect(() =>
      runtime.mount("/", Column("root", [Text("same", "A"), Text("same", "B")])),
    ).toThrow("duplicate widget id: same");
    expect(emissions).toEqual([]);
  });
});
