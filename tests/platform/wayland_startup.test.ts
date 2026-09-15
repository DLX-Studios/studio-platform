import { describe, expect, test } from "bun:test";
import { join } from "node:path";

const root = join(import.meta.dir, "../..");
const harness = join(root, "scripts/test-headless-wayland.sh");

describe("native Wayland startup", () => {
  test("launches under a headless native compositor and rejects no endpoint", async () => {
    expect(await Bun.file(harness).exists()).toBe(true);

    const child = Bun.spawn([harness], {
      cwd: root,
      env: process.env,
      stdout: "pipe",
      stderr: "pipe",
    });
    const [exitCode, stdout, stderr] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ]);

    expect(exitCode, stderr).toBe(0);
    expect(stdout).toContain(
      "native headless Wayland launch with no XWayland process: ok",
    );
    expect(stdout).toContain("missing Wayland endpoint rejection: ok");
  }, 30_000);
});
