// Studio toolchain shim: the native `studio` binary owns packaging.
//
// This module is the documented entry (`bun run build:starter`, `build:pos`,
// direct invocation, and the programmatic `buildExample` used by tests). It
// guarantees the `studio` binary is built, delegates to
// `studio build <project>`, and propagates the native exit code verbatim so
// bundles, diagnostics, and exit codes stay identical to the native command.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";

const root = join(import.meta.dir, "..");
const profile = "debug";
const cacheRoot = process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache");
const targetDir =
  process.env.CARGO_TARGET_DIR ?? join(cacheRoot, `studio-platform-shared-${profile}`);
// Prefer the repository-local binary (the same tree being built/tested), then
// an explicitly configured target directory, then the shared cache.
const candidates = [
  join(root, "target", profile, "studio"),
  join(targetDir, profile, "studio"),
];
const binary = candidates.find((candidate) => existsSync(candidate)) ?? candidates[0];

function ensureBinary(): void {
  if (existsSync(binary)) {
    return;
  }
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: targetDir,
    CARGO_PROFILE_DEV_DEBUG: "0",
    CARGO_PROFILE_TEST_DEBUG: "0",
    CARGO_INCREMENTAL: "0",
  };
  const build = spawnSync("cargo", ["build", "--locked", "-p", "studio-cli"], {
    cwd: root,
    env,
    stdio: "inherit",
  });
  if (build.status !== 0) {
    process.exit(build.status ?? 1);
  }
  if (!existsSync(binary)) {
    console.error(`expected binary missing: ${binary}`);
    process.exit(1);
  }
}

function spawnStudioBuild(project: string): number {
  ensureBinary();
  const result = spawnSync(binary, ["build", project], {
    cwd: root,
    stdio: "inherit",
  });
  return result.status ?? 1;
}

/** Output location the native command writes for a name-form example. */
function bundleOutputPath(project: string): string {
  if (existsSync(project) && !existsSync(join(root, "examples", project))) {
    return join(project, "build", `${basename(project)}.studio`);
  }
  return join("examples", project, "build", `${project}.studio`);
}

export interface BundleBuild {
  bytes: Uint8Array;
  outputPath: string;
}

/**
 * Build one example through the native toolchain and return the produced
 * bundle bytes. Throws with the native stderr tail when the build fails.
 */
export async function buildExample(example = "pos-desktop"): Promise<BundleBuild> {
  const project = join(root, "examples", example);
  const nameForm = existsSync(project);
  const target = nameForm ? example : project;
  const result = spawnSync(binary, ["build", target], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
    encoding: "utf8",
  });
  if (result.status !== 0) {
    const tail = (result.stderr ?? "").trim().split("\n").slice(-3).join("\n");
    throw new Error(tail || `studio build ${target} failed with ${result.status}`);
  }
  const outputPath = bundleOutputPath(target);
  return { bytes: new Uint8Array(readFileSync(join(root, outputPath))), outputPath };
}

function main(): void {
  const project = process.argv[2];
  if (!project) {
    console.error("usage: bun run ./scripts/build-example.ts <example|project-path>");
    process.exit(2);
  }
  process.exit(spawnStudioBuild(project));
}

if (import.meta.main) {
  main();
}