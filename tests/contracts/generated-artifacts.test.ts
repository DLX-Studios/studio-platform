import { describe, expect, test } from "bun:test";
import { join } from "node:path";

const root = join(import.meta.dir, "../..");
const schemaDirectory = join(root, "protocol/schemas/protocol-v1");
const inventory = [
  "action-request.schema.json",
  "action-result.schema.json",
  "guest-message.schema.json",
  "host-event.schema.json",
  "mount-tree.schema.json",
  "navigation-command.schema.json",
  "patch-batch.schema.json",
] as const;

describe("generated Rust-authoritative protocol artifacts", () => {
  test("contains exactly the documented protocol-v1 schema inventory", async () => {
    const files = (
      await Array.fromAsync(
        new Bun.Glob("*.schema.json").scan(schemaDirectory),
      )
    ).sort();
    expect(files).toEqual([...inventory]);
  });

  for (const filename of inventory) {
    test(`${filename} is a stable draft 2020-12 Studio schema`, async () => {
      const file = Bun.file(join(schemaDirectory, filename));
      expect(await file.exists()).toBe(true);
      const text = await file.text();
      const schema = JSON.parse(text);

      expect(schema.$schema).toBe(
        "https://json-schema.org/draft/2020-12/schema",
      );
      expect(schema.$id).toBe(
        `https://studio.local/schemas/protocol-v1/${filename}`,
      );
      expect(text.endsWith("\n")).toBe(true);
      expect(text.endsWith("\n\n")).toBe(false);
    });
  }
});
