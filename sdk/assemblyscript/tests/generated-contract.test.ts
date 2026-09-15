import { describe, expect, test } from "bun:test";
import { join } from "node:path";

const root = join(import.meta.dir, "../../..");
const generatedBindings = join(
  root,
  "sdk/assemblyscript/assembly/generated/protocol.ts",
);
const fixtureDirectory = join(root, "protocol/fixtures/protocol-v1/valid");
const fixtureInventory = [
  "guest-action.json",
  "guest-log.json",
  "guest-mount.json",
  "guest-navigate.json",
  "guest-patch.json",
  "host-action-result-failure.json",
  "host-action-result-success.json",
  "host-lifecycle.json",
  "host-navigation.json",
  "host-ui.json",
] as const;

function containsOwner(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsOwner);
  if (value !== null && typeof value === "object") {
    return Object.entries(value).some(
      ([key, child]) => key === "owner" || containsOwner(child),
    );
  }
  return false;
}

describe("generated AssemblyScript protocol-v1 contract", () => {
  test("contains exactly one representative fixture per closed envelope shape", async () => {
    const files = (
      await Array.fromAsync(new Bun.Glob("*.json").scan(fixtureDirectory))
    ).sort();
    expect(files).toEqual([...fixtureInventory]);

    const fixtures = await Promise.all(
      files.map(async (filename) => ({
        filename,
        text: await Bun.file(join(fixtureDirectory, filename)).text(),
      })),
    );
    const decoded = fixtures.map(({ filename, text }) => ({
      filename,
      text,
      value: JSON.parse(text) as { type: string; payload: Record<string, unknown> },
    }));

    expect(
      decoded.filter(({ filename }) => filename.startsWith("guest-")).map(({ value }) => value.type),
    ).toEqual(["action", "log", "mount", "navigate", "patch"]);
    expect(
      new Set(
        decoded.filter(({ filename }) => filename.startsWith("host-")).map(({ value }) => value.type),
      ),
    ).toEqual(new Set(["action_result", "lifecycle", "navigation", "ui"]));
    expect(
      decoded
        .filter(({ value }) => value.type === "action_result")
        .map(({ value }) => value.payload.status)
        .sort(),
    ).toEqual(["failure", "success"]);

    for (const { text, value } of decoded) {
      expect(text.endsWith("\n")).toBe(true);
      expect(text.endsWith("\n\n")).toBe(false);
      expect(containsOwner(value)).toBe(false);
    }
  });

  test("exports matching closed AssemblyScript discriminants", async () => {
    const file = Bun.file(generatedBindings);
    expect(await file.exists()).toBe(true);
    const source = await file.text();

    for (const discriminant of [
      "mount",
      "patch",
      "navigate",
      "action",
      "log",
      "ui",
      "navigation",
      "action_result",
      "lifecycle",
      "update_prop",
      "insert_child",
      "remove_node",
      "replace_node",
      "push",
      "replace",
      "pop",
      "pop_to",
      "reset",
      "success",
      "failure",
      "loading",
      "running",
      "trapped",
      "stopped",
    ]) {
      expect(source).toContain(`= "${discriminant}";`);
    }
    expect(source).toContain("export const PROTOCOL_VERSION: u16 = 1;");
    expect(source).toContain("Generated from Rust-authoritative protocol-v1 schemas");
  });
});
