import { spawnSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const args = new Set(process.argv.slice(2));
const release = args.has("--release");
const buildRuntime = args.has("--all");
const profile = release ? "release" : "debug";
const cacheRoot =
  process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache");
const sharedTarget =
  process.env.CARGO_TARGET_DIR ?? join(cacheRoot, `studio-platform-shared-${profile}`);

const packages = buildRuntime
  ? ["studio-designer", "studio-app"]
  : ["studio-designer"];

const env = {
  ...process.env,
  CARGO_TARGET_DIR: sharedTarget,
  CARGO_PROFILE_DEV_DEBUG: "0",
  CARGO_PROFILE_TEST_DEBUG: "0",
  CARGO_INCREMENTAL: "0",
  CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "4",
};

const result = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    ...(release ? ["--release"] : []),
    ...packages.flatMap((pkg) => ["-p", pkg]),
  ],
  { cwd: root, env, stdio: "inherit" },
);

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

for (const pkg of packages) {
  const binary = join(sharedTarget, profile, pkg);
  if (!existsSync(binary)) {
    console.error(`expected binary missing: ${binary}`);
    process.exit(1);
  }
  const mebibytes = (statSync(binary).size / 1024 / 1024).toFixed(1);
  console.log(`${pkg}: ${binary} (${mebibytes} MiB)`);
}

console.log("\nRun the Designer from a native Wayland session:");
console.log(`  ${join(sharedTarget, profile, "studio-designer")}`);
