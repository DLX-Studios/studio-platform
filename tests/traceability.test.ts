import { describe, expect, test } from "bun:test";
import { join } from "node:path";

const root = join(import.meta.dir, "..");

describe("milestone requirement traceability", () => {
  test("maps every FR-001..032 and SC-001..012 exactly once to explicit evidence", async () => {
    const specification = await Bun.file(join(root, "specs/001-secure-plugin-runtime/spec.md")).text();
    const matrix = await Bun.file(join(root, "specs/001-secure-plugin-runtime/traceability.md")).text();
    const required = new Set(specification.match(/(?:FR|SC)-\d{3}/g) ?? []);
    const rows = [...matrix.matchAll(/^\| ((?:FR|SC)-\d{3}) \| ([^|]+) \| (Automated|Manual) \| ([^|]+) \|$/gm)];
    const counts = new Map<string, number>();
    for (const [, id, , , evidence] of rows) {
      counts.set(id!, (counts.get(id!) ?? 0) + 1);
      expect(evidence!.trim().length).toBeGreaterThan(3);
    }
    expect([...required].filter((id) => !counts.has(id))).toEqual([]);
    expect([...counts].filter(([, count]) => count !== 1)).toEqual([]);
    expect(required.size).toBe(44);
  });

  test("release checklist retains automated and explicit manual gates", async () => {
    const checklist = await Bun.file(join(root, "docs/RELEASE_CHECKLIST.md")).text();
    for (const section of ["Automated gates", "Manual native gates", "Security and provenance", "Release decision"]) {
      expect(checklist).toContain(`## ${section}`);
    }
  });
});
