// Build/cache the Rust browser host. The app guest still comes from .studio.
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, mkdirSync, existsSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = join(root, ".scratch/web-core");
const hash = createHash("sha256");
function fingerprint(path: string): void {
  for (const entry of readdirSync(path, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const file = join(path, entry.name);
    if (entry.isDirectory()) fingerprint(file);
    else if (entry.isFile() && (file.endsWith(".rs") || entry.name === "Cargo.toml")) {
      hash.update(file.slice(root.length)); hash.update(readFileSync(file));
    }
  }
}
for (const name of ["studio-web-core", "studio-navigation", "studio-protocol", "studio-ui"]) fingerprint(join(root, "crates", name));
hash.update(readFileSync(join(root, "Cargo.lock")));
hash.update(readFileSync(fileURLToPath(import.meta.url)));
const digest = hash.digest("hex");
const stamp = join(output, ".source-hash");
if (existsSync(stamp) && readFileSync(stamp, "utf8") === digest && existsSync(join(output, "studio_web_core.js")) && existsSync(join(output, "studio_web_core_bg.wasm"))) {
  console.log(output);
  process.exit(0);
}
const cachedTool = join(root, ".scratch/tools/wasm-bindgen");
const bindgen = process.env.STUDIO_WASM_BINDGEN ?? (existsSync(cachedTool) ? cachedTool : "wasm-bindgen");
async function run(args: string[]): Promise<string> {
  const child = Bun.spawn(args, { cwd: root, env: { ...process.env, CARGO_PROFILE_DEV_DEBUG: "0", CARGO_PROFILE_TEST_DEBUG: "0", CARGO_INCREMENTAL: "0" }, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  if (code !== 0) throw new Error(`${args[0]} failed (${code}): ${stderr}`);
  return stdout;
}
try {
  const version = await run([bindgen, "--version"]);
  if (version.trim() !== "wasm-bindgen 0.2.127") throw new Error("Web builds require wasm-bindgen-cli 0.2.127 (or STUDIO_WASM_BINDGEN pointing to that version) and rustup target add wasm32-unknown-unknown.");
  const cargo = await run(["cargo", "build", "--locked", "-p", "studio-web-core", "--target", "wasm32-unknown-unknown", "--message-format=json"]);
  const artifacts = cargo.split("\n").filter(Boolean).map(line => JSON.parse(line) as { reason?: string; target?: { name: string }; filenames?: string[] });
  const wasm = artifacts.find(item => item.reason === "compiler-artifact" && item.target?.name === "studio_web_core")?.filenames?.find(file => file.endsWith(".wasm"));
  if (!wasm) throw new Error("Cargo did not produce the studio-web-core Wasm artifact.");
  mkdirSync(output, { recursive: true });
  await run([bindgen, "--target", "web", "--out-name", "studio_web_core", "--out-dir", output, wasm]);
  await Bun.write(stamp, digest);
  console.log(output);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
