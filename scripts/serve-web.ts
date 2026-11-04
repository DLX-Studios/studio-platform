// Local static/SPA preview for a Studio web export.
import { resolve, sep, join } from "node:path";
import { stat } from "node:fs/promises";
const root = resolve(Bun.argv[2] ?? "examples/stream-countdown/build/web");
const port = Number(Bun.argv[3] ?? 5180);
const server = Bun.serve({ hostname: "127.0.0.1", port, async fetch(request) {
  let path: string;
  try { path = decodeURIComponent(new URL(request.url).pathname); } catch { return new Response("Bad path", {status: 400}); }
  const file = resolve(root, `.${path}`);
  if (file !== root && !file.startsWith(root + sep)) return new Response("Not found", {status:404});
  const info = await stat(file).catch(() => undefined);
  const target = info?.isDirectory() ? join(file, "index.html") : file;
  if (await Bun.file(target).exists()) return new Response(Bun.file(target));
  if (!path.split("/").pop()?.includes(".")) return new Response(Bun.file(join(root,"index.html")));
  return new Response("Not found", {status:404});
}});
console.log(`Studio web preview: ${server.url}`);
