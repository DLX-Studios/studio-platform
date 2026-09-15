/**
 * Loader for @digitalocean/dots.
 *
 * The SDK ships its Kiota-generated *TypeScript sources* (no .d.ts), and its
 * sources don't typecheck against the installed Kiota version. Any static
 * import (even subpaths — root index.ts exists, so the .js→.ts substitution
 * always wins) drags 237+ source errors into our typecheck program.
 *
 * Solution: import with a computed specifier. TS ignores non-literal dynamic
 * imports, and the bundler (rolldown) leaves them for runtime resolution from
 * node_modules. Cached after first load.
 */

let cached: Promise<Record<string, any>> | null = null;

export function loadDoSdk(): Promise<Record<string, any>> {
  if (!cached) {
    const specifier = ["@digitalocean", "dots"].join("/");
    cached = import(/* @vite-ignore */ specifier);
  }
  return cached;
}
