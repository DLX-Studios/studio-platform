// @bun
import {
  ActionFailure,
  HandledHttpError,
  HttpError,
  Redirect,
  SvelteKitError,
  ValidationError,
  error,
  isRedirect,
  merge_tracing,
  otel,
  record_span,
  text,
  with_request_store
} from "./server/chunks/index-pcpfewry.js";
import {
  afterNavigate,
  noop,
  once
} from "./server/chunks/index-6mtzd7p1.js";
import {
  derived,
  escape_html,
  parse,
  render,
  stringify,
  uneval
} from "./server/chunks/index-e5h3mrsa.js";
import {
  __require
} from "./server/chunks/index-qcep1gwj.js";

// node_modules/@sveltejs/adapter-bun/src/index.js
import fs2 from "fs";
import process2 from "process";

// node_modules/@sveltejs/adapter-bun/src/options.js
var options_default = {};

// .svelte-kit/output/server/manifest.js
var manifest = (() => {
  function __memo(fn) {
    let value;
    return () => value ??= value = fn();
  }
  return {
    appDir: "_app",
    appPath: "_app",
    assets: new Set([]),
    mimeTypes: {},
    _: {
      client: { start: "_app/immutable/entry/start.m1pMgU0L.js", app: "_app/immutable/entry/app.B40Rnt5P.js", imports: ["_app/immutable/entry/start.m1pMgU0L.js", "_app/immutable/entry/payload.DSmR2FwN.js", "_app/immutable/chunks/DuqgYnXA.js", "_app/immutable/chunks/tW-EaAD7.js", "_app/immutable/chunks/DjAkF-xo.js", "_app/immutable/chunks/DPBx5xTx.js", "_app/immutable/chunks/D4UQy-ZY.js", "_app/immutable/chunks/CV8VO5Jt.js", "_app/immutable/entry/app.B40Rnt5P.js"], stylesheets: [], fonts: [], uses_env_dynamic_public: false },
      nodes: [
        __memo(() => import("./server/chunks/0-dfs2pj0r.js")),
        __memo(() => import("./server/chunks/1-s8c78tc3.js")),
        __memo(() => import("./server/chunks/2-1w4mg714.js")),
        __memo(() => import("./server/chunks/3-6dke75ew.js")),
        __memo(() => import("./server/chunks/4-6ywzc1ka.js")),
        __memo(() => import("./server/chunks/5-z5qwk5x0.js")),
        __memo(() => import("./server/chunks/6-eqg7qny7.js")),
        __memo(() => import("./server/chunks/7-tpge7rjq.js")),
        __memo(() => import("./server/chunks/8-tkva5jkd.js")),
        __memo(() => import("./server/chunks/9-mk7jd0ne.js")),
        __memo(() => import("./server/chunks/10-5h9kn5yj.js"))
      ],
      remotes: {},
      routes: [
        {
          id: "/",
          pattern: /^\/$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 2 },
          endpoint: null
        },
        {
          id: "/agents",
          pattern: /^\/agents\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 3 },
          endpoint: null
        },
        {
          id: "/analytics",
          pattern: /^\/analytics\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 4 },
          endpoint: null
        },
        {
          id: "/api/analytics",
          pattern: /^\/api\/analytics\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-tmse6fq2.js"))
        },
        {
          id: "/api/auth/login",
          pattern: /^\/api\/auth\/login\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-wqy4fk04.js"))
        },
        {
          id: "/api/auth/logout",
          pattern: /^\/api\/auth\/logout\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-29rmhgws.js"))
        },
        {
          id: "/api/daemon/hello",
          pattern: /^\/api\/daemon\/hello\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-hqgc0rar.js"))
        },
        {
          id: "/api/do/options",
          pattern: /^\/api\/do\/options\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-y81bnqxg.js"))
        },
        {
          id: "/api/do/sshkeys",
          pattern: /^\/api\/do\/sshkeys\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-sa6a2av3.js"))
        },
        {
          id: "/api/do/sshkeys/generate",
          pattern: /^\/api\/do\/sshkeys\/generate\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-ckbg8g5x.js"))
        },
        {
          id: "/api/machines",
          pattern: /^\/api\/machines\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-xrat70bq.js"))
        },
        {
          id: "/api/machines/[id]",
          pattern: /^\/api\/machines\/([^/]+?)\/?$/,
          params: [{ name: "id", optional: false, rest: false, chained: false }],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-4n3v99yx.js"))
        },
        {
          id: "/api/machines/[id]/agent",
          pattern: /^\/api\/machines\/([^/]+?)\/agent\/?$/,
          params: [{ name: "id", optional: false, rest: false, chained: false }],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-q25pkggj.js"))
        },
        {
          id: "/api/setup",
          pattern: /^\/api\/setup\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-4q4asf7y.js"))
        },
        {
          id: "/api/setup/probe",
          pattern: /^\/api\/setup\/probe\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-c3ergcq9.js"))
        },
        {
          id: "/api/setup/spaces/probe",
          pattern: /^\/api\/setup\/spaces\/probe\/?$/,
          params: [],
          page: null,
          endpoint: __memo(() => import("./server/chunks/_server.ts-9mtvjyz9.js"))
        },
        {
          id: "/login",
          pattern: /^\/login\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 5 },
          endpoint: null
        },
        {
          id: "/machines",
          pattern: /^\/machines\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 7 },
          endpoint: null
        },
        {
          id: "/m/[id]",
          pattern: /^\/m\/([^/]+?)\/?$/,
          params: [{ name: "id", optional: false, rest: false, chained: false }],
          page: { layouts: [0], errors: [1], leaf: 6 },
          endpoint: null
        },
        {
          id: "/providers",
          pattern: /^\/providers\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 8 },
          endpoint: null
        },
        {
          id: "/setup",
          pattern: /^\/setup\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 9 },
          endpoint: null
        },
        {
          id: "/ssh-keys",
          pattern: /^\/ssh-keys\/?$/,
          params: [],
          page: { layouts: [0], errors: [1], leaf: 10 },
          endpoint: null
        }
      ],
      prerendered_routes: new Set([]),
      matchers: async () => {
        return {};
      },
      server_assets: {}
    }
  };
})();
var base = "/";
var embed = false;
var env_prefix = "";
var origin = undefined;

// node_modules/@sveltejs/adapter-bun/src/routes-util.js
import path from "path";
import { fileURLToPath } from "url";
var dir = path.dirname(fileURLToPath(import.meta.url));
function resolve_file(subdir, filename) {
  return embed ? filename : path.resolve(dir, subdir, filename);
}
var CONTENT_ENCODING = { br: "br", gz: "gzip" };
var ESCAPED_PATH_CHAR = /[\u0000-\u001f\u007f-\u{10ffff} "#<>?`{}%\\]/gu;
function encode_pathname(pathname) {
  return pathname.replace(ESCAPED_PATH_CHAR, (char) => encodeURIComponent(char));
}
function route_paths(pathname) {
  const minimal = encode_pathname(pathname);
  const full = pathname.split("/").map(encodeURIComponent).join("/");
  return minimal === full ? [minimal] : [minimal, full];
}
function to_paths(url) {
  return route_paths(path.posix.join(base, url));
}
function to_directory_paths(url) {
  const directory = `${path.posix.join(base, url).replace(/\/$/, "")}/`;
  const paths = route_paths(directory);
  if (directory !== "/") {
    paths.push(...route_paths(directory.slice(0, -1)));
  }
  return paths;
}
function is_fresh(request, etag, mtime) {
  const header = request.headers.get("if-none-match");
  if (header !== null) {
    return header.split(",").some((value) => ["*", etag].includes(value.trim().replace(/^W\//, "")));
  }
  const since = Date.parse(request.headers.get("if-modified-since") ?? "");
  return Number.isFinite(since) && Math.trunc(mtime / 1000) <= Math.trunc(since / 1000);
}
function negotiate(accept, meta) {
  if (accept === null || !meta.br && !meta.gz)
    return null;
  const accepted = new Set;
  for (const part of accept.split(",")) {
    const [name = "", ...params] = part.trim().toLowerCase().split(";");
    if (params.some((param) => /^q=0(\.0*)?$/.test(param.trim())))
      continue;
    accepted.add(name.trim());
  }
  if (meta.br && (accepted.has("br") || accepted.has("*")))
    return "br";
  if (meta.gz && (accepted.has("gzip") || accepted.has("*")))
    return "gz";
  return null;
}
function handlers(handler) {
  return { GET: handler, HEAD: handler };
}
function route_entries(paths, route) {
  return paths.map((route_path) => [route_path, route]);
}
function file_route(file, meta, extra_headers = {}) {
  const content_type = Bun.file(file).type;
  const last_modified = new Date(meta.mtime).toUTCString();
  const handler = (request) => {
    const encoding = request.headers.get("range") === null ? negotiate(request.headers.get("accept-encoding"), meta) : null;
    const etag = encoding === null ? `"${meta.hash}"` : `"${meta.hash}-${encoding}"`;
    const response_headers = {
      "content-type": content_type,
      ...extra_headers,
      etag,
      "last-modified": last_modified
    };
    if (meta.br || meta.gz)
      response_headers["vary"] = "accept-encoding";
    if (is_fresh(request, etag, meta.mtime)) {
      return new Response(null, { status: 304, headers: response_headers });
    }
    let body_file = file;
    if (encoding !== null) {
      response_headers["content-encoding"] = CONTENT_ENCODING[encoding];
      body_file = `${file}.${encoding}`;
    }
    return new Response(Bun.file(body_file), { headers: response_headers });
  };
  return handlers(handler);
}
function client_asset(url, filename = url, meta) {
  const immutable = url.startsWith(`${manifest.appDir}/immutable/`);
  const route = file_route(resolve_file("client", filename), meta, immutable ? { "cache-control": "public,max-age=31536000,immutable" } : {});
  const paths = to_paths(url);
  if (url.endsWith("/index.html") || url === "index.html") {
    paths.push(...to_directory_paths(url.slice(0, -"index.html".length)));
  } else if (url.endsWith(".html")) {
    paths.push(...to_paths(url.slice(0, -".html".length)));
  }
  return route_entries(paths, route);
}

// node_modules/@sveltejs/adapter-bun/src/routes.js
var routes = Object.fromEntries([
  ...client_asset("_app/immutable/nodes/10.DcxKhWqh.js", undefined, { hash: "df5b820c3ef5cc52", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/8.DnbG8dL_.js", undefined, { hash: "b7cfe3fbc4d283b6", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/7.5RGIGWGT.js", undefined, { hash: "f53baa9cc448bdfd", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/6.D5wrDmNj.js", undefined, { hash: "bb008dd0be05df71", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/1.DZhTAPRS.js", undefined, { hash: "3ae592d748627c40", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/4.DgDOe5kj.js", undefined, { hash: "4e788837ca6ad072", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/3.Dx8ooiGh.js", undefined, { hash: "eaa90c10a7c08c17", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/9.BEW2YkBE.js", undefined, { hash: "bbefd0cdd52232da", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/0.rM0EStb_.js", undefined, { hash: "c4dcbbc274052347", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/5.CJ4OhlDZ.js", undefined, { hash: "bef9a27d2d5eaa6d", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/nodes/2.DGaNZwx9.js", undefined, { hash: "7f6baaae13ba2227", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/BTmxmbKa.js", undefined, { hash: "5e26fd37d13c3c58", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/tW-EaAD7.js", undefined, { hash: "73818efb6e5b40da", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/CmwU_1OI.js", undefined, { hash: "1f4cd407aa054f2a", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/Bjy-W4x2.js", undefined, { hash: "566098d43927cea6", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/D4UQy-ZY.js", undefined, { hash: "3355e3c362f4c66c", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/CV8VO5Jt.js", undefined, { hash: "692e876c937b0528", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/DuqgYnXA.js", undefined, { hash: "8e3923d5b420be19", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/DjAkF-xo.js", undefined, { hash: "38278d39ea2bfd57", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/BIRBqmYv.js", undefined, { hash: "c248cb08efa84f7b", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/Cene61J5.js", undefined, { hash: "313e5faba22369e5", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/DPBx5xTx.js", undefined, { hash: "926be05ed6a1874a", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/chunks/StsqyyfY.js", undefined, { hash: "407b527b06aca0be", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/entry/start.m1pMgU0L.js", undefined, { hash: "829dc11edb0b3f28", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/entry/app.B40Rnt5P.js", undefined, { hash: "31e4f76978825ebb", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/entry/payload.DSmR2FwN.js", undefined, { hash: "63f597fe584aedcd", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/assets/OptionCard.I4OQdEuu.css", undefined, { hash: "a401a38272e80109", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/assets/0.5UnA_0Iu.css", undefined, { hash: "92d30c84c1c0f426", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/immutable/assets/6.CAqGrTQO.css", undefined, { hash: "ee92268850e9911c", mtime: 1788652471342, br: true, gz: true }),
  ...client_asset("_app/version.json", undefined, { hash: "9b8a479e8d63b2bc", mtime: 1788652471342, br: true, gz: true })
].reverse());
var server_assets = new Map([]);
// .svelte-kit/output/server/chunks/server.js
var text_encoder = new TextEncoder;
function stream_from_iterable(iterable) {
  if (ReadableStream.from)
    return ReadableStream.from(iterable);
  const iterator = iterable[Symbol.asyncIterator]();
  return new ReadableStream({
    async pull(controller) {
      const { value, done } = await iterator.next();
      if (done)
        controller.close();
      else
        controller.enqueue(value);
    },
    async cancel(reason) {
      await iterator.return?.(reason);
    }
  });
}
function stream_text(head, chunks) {
  return stream_from_iterable(async function* () {
    yield text_encoder.encode(head);
    for await (const chunk of chunks)
      if (chunk)
        yield text_encoder.encode(chunk);
  }());
}
var text_decoder = new TextDecoder;
function get_relative_path(from, to) {
  const from_parts = from.split(/[/\\]/);
  const to_parts = to.split(/[/\\]/);
  from_parts.pop();
  while (from_parts[0] === to_parts[0]) {
    from_parts.shift();
    to_parts.shift();
  }
  let i = from_parts.length;
  while (i--)
    from_parts[i] = "..";
  return from_parts.concat(to_parts).join("/");
}
function base64_encode(bytes) {
  if (globalThis.Buffer)
    return globalThis.Buffer.from(bytes).toString("base64");
  let binary = "";
  for (let i = 0;i < bytes.length; i++)
    binary += String.fromCharCode(bytes[i]);
  return btoa(binary);
}
function base64_decode(encoded) {
  if (globalThis.Buffer) {
    const buffer = globalThis.Buffer.from(encoded, "base64");
    return new Uint8Array(buffer);
  }
  const binary = atob(encoded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0;i < binary.length; i++)
    bytes[i] = binary.charCodeAt(i);
  return bytes;
}
var ENDPOINT_METHODS = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "OPTIONS",
  "HEAD",
  "QUERY"
];
var MUTATIVE_METHODS = [
  "POST",
  "PUT",
  "PATCH",
  "DELETE"
];
var BODY_DEPENDENT_METHODS = [...MUTATIVE_METHODS, "QUERY"];
var PAGE_METHODS = [
  "GET",
  "POST",
  "HEAD"
];
var IN_WEBCONTAINER = !!globalThis.process?.versions?.webcontainer;
var REROUTED_URL_HEADER = "x-sveltekit-rerouted-url";
var assets = "";
var app_dir = "_app";
var escape_sequence_pattern = /\[([ux])\+([^\]]+)\]/;
function run_matcher(matcher, value) {
  const result = matcher["~standard"].validate(value);
  if (result instanceof Promise)
    throw new Error("Async param matchers are not supported");
  if (result.issues)
    return { success: false };
  const parsed = result.value;
  if (typeof parsed !== "string" && typeof parsed !== "number" && typeof parsed !== "boolean" && typeof parsed !== "bigint")
    throw new Error("Param matcher must return a string, number, boolean, or bigint");
  return {
    success: true,
    value: parsed
  };
}
function exec(match, params, matchers) {
  const result = {};
  const values = match.slice(1);
  const values_needing_match = values.filter((value) => value !== undefined);
  let buffered = 0;
  for (let i = 0;i < params.length; i += 1) {
    const param = params[i];
    let value = values[i - buffered];
    if (param.chained && param.rest && buffered) {
      value = values.slice(i - buffered, i + 1).filter((s) => s).join("/");
      buffered = 0;
    }
    if (value === undefined) {
      if (param.rest)
        value = "";
      else
        continue;
    }
    const decoded = decodeURIComponent(value);
    if (param.matcher) {
      const outcome = run_matcher(matchers[param.matcher], decoded);
      if (!outcome.success) {
        if (param.optional && param.chained) {
          buffered++;
          continue;
        }
        return;
      }
      result[param.name] = outcome.value;
    } else
      result[param.name] = decoded;
    const next_param = params[i + 1];
    const next_value = values[i + 1];
    if (next_param && !next_param.rest && next_param.optional && next_value && param.chained)
      buffered = 0;
    if (!next_param && !next_value && Object.keys(result).length === values_needing_match.length)
      buffered = 0;
  }
  if (buffered)
    return;
  return result;
}
new RegExp(`${escape_sequence_pattern.source}|${/\[(\[)?(\.\.\.)?([\w-]+?)(?:=([\w-]+))?\]\]?/g.source}`, "g");
function find_route(path2, routes2, matchers) {
  for (const route of routes2) {
    const match = route.pattern.exec(path2);
    if (!match)
      continue;
    const matched = exec(match, route.params, matchers);
    if (matched)
      return {
        route,
        params: matched
      };
  }
  return null;
}
var internal = new URL("a://");
function resolve(base2, path2) {
  if (path2[0] === "/" && path2[1] === "/")
    return path2;
  let url = new URL(base2, internal);
  url = new URL(path2, url);
  return url.protocol === internal.protocol ? url.pathname + url.search + url.hash : url.href;
}
function relative_pathname(from, to) {
  const segment = to.replace(/\/$/, "").split("/").at(-1);
  return from.endsWith("/") ? `../${segment}` : `${segment}/`;
}
function normalize_path(path2, trailing_slash) {
  if (path2 === "/" || trailing_slash === "ignore")
    return path2;
  if (trailing_slash === "never")
    return path2.endsWith("/") ? path2.slice(0, -1) : path2;
  else if (trailing_slash === "always" && !path2.endsWith("/"))
    return path2 + "/";
  return path2;
}
function decode_pathname(pathname) {
  return pathname.split("%25").map(decodeURI).join("%25");
}
function make_trackable(url, callback, search_params_callback, allow_hash = false) {
  const tracked = new URL(url);
  Object.defineProperty(tracked, "searchParams", {
    value: new Proxy(tracked.searchParams, { get(obj, key) {
      if (key === "get" || key === "getAll" || key === "has")
        return (param, ...rest) => {
          search_params_callback(param);
          return obj[key](param, ...rest);
        };
      callback();
      const value = Reflect.get(obj, key);
      return typeof value === "function" ? value.bind(obj) : value;
    } }),
    enumerable: true,
    configurable: true
  });
  const tracked_url_properties = [
    "href",
    "pathname",
    "search",
    "toString",
    "toJSON"
  ];
  if (allow_hash)
    tracked_url_properties.push("hash");
  for (const property of tracked_url_properties)
    Object.defineProperty(tracked, property, {
      get() {
        callback();
        return url[property];
      },
      enumerable: true,
      configurable: true
    });
  tracked[Symbol.for("nodejs.util.inspect.custom")] = (_depth, opts, inspect) => {
    return inspect(url, opts);
  };
  tracked.searchParams[Symbol.for("nodejs.util.inspect.custom")] = (_depth, opts, inspect) => {
    return inspect(url.searchParams, opts);
  };
  if (!allow_hash)
    disable_hash(tracked);
  return tracked;
}
function disable_hash(url) {
  allow_nodejs_console_log(url);
  Object.defineProperty(url, "hash", { get() {
    throw new Error("Cannot access event.url.hash. Consider using `page.url.hash` inside a component instead");
  } });
}
function disable_search(url) {
  allow_nodejs_console_log(url);
  for (const property of ["search", "searchParams"])
    Object.defineProperty(url, property, { get() {
      throw new Error(`Cannot access url.${property} on a page with prerendering enabled`);
    } });
}
function allow_nodejs_console_log(url) {
  url[Symbol.for("nodejs.util.inspect.custom")] = (_depth, opts, inspect) => {
    return inspect(new URL(url), opts);
  };
}
var DATA_SUFFIX = "/__data.json";
var HTML_DATA_SUFFIX = ".html__data.json";
function has_data_suffix(pathname) {
  return pathname.endsWith(DATA_SUFFIX) || pathname.endsWith(HTML_DATA_SUFFIX);
}
function add_data_suffix(pathname) {
  if (pathname.endsWith(".html"))
    return pathname.replace(/\.html$/, HTML_DATA_SUFFIX);
  return pathname.replace(/\/$/, "") + DATA_SUFFIX;
}
function strip_data_suffix(pathname) {
  if (pathname.endsWith(HTML_DATA_SUFFIX))
    return pathname.slice(0, -16) + ".html";
  return pathname.slice(0, -12);
}
var ROUTE_SUFFIX = "/__route.js";
var HTML_ROUTE_SUFFIX = ".html__route.js";
function has_resolution_suffix(pathname) {
  return pathname.endsWith(ROUTE_SUFFIX) || pathname.endsWith(HTML_ROUTE_SUFFIX);
}
function add_resolution_suffix(pathname) {
  if (pathname.endsWith(".html"))
    return pathname.replace(/\.html$/, HTML_ROUTE_SUFFIX);
  return pathname.replace(/\/$/, "") + ROUTE_SUFFIX;
}
function strip_resolution_suffix(pathname) {
  if (pathname.endsWith(HTML_ROUTE_SUFFIX))
    return pathname.slice(0, -15) + ".html";
  return pathname.slice(0, -11);
}
var symbol = Symbol.for("sveltekit.global_state");
globalThis[symbol] ??= {};
function set_nested_value(object, field, value) {
  deep_set(object, split_path(field.name), value);
}
function parse_form_key(form_id, key) {
  const suffix = "/" + form_id;
  let name = key;
  if (!name.endsWith(suffix))
    throw new Error(`Form contained a field that wasn't created with form.fields.as(...): ${name}`);
  name = name.slice(0, -suffix.length);
  let type = null;
  if (name.startsWith("n:")) {
    name = name.slice(2);
    type = "number";
  } else if (name.startsWith("b:")) {
    name = name.slice(2);
    type = "boolean";
  }
  const is_array = name.endsWith("[]");
  if (is_array)
    name = name.slice(0, -2);
  return {
    name,
    type,
    is_array
  };
}
function coerce_form_value(type, value) {
  if (Array.isArray(value))
    return value.map((value2) => coerce_form_value(type, value2));
  if (type === "number")
    return value === "" ? undefined : parseFloat(value);
  if (type === "boolean")
    return value === "on";
  return value;
}
var DELETE_KEY = {};
function convert_formdata(form_id, data) {
  const result = {};
  for (const field_name of data.keys()) {
    const values = data.getAll(field_name);
    const field = parse_form_key(form_id, field_name);
    const entries = values.filter((entry) => typeof entry === "string" || entry.name !== "" || entry.size > 0);
    if (entries.length === 0 && !field.is_array)
      continue;
    if (entries.length > 1 && !field.is_array)
      throw new Error(`Form cannot contain duplicated keys \u2014 "${field.name}" has ${entries.length} values`);
    set_nested_value(result, field, coerce_form_value(field.type, field.is_array ? entries : entries[0]));
  }
  return result;
}
var BINARY_FORM_CONTENT_TYPE = "application/x-sveltekit-formdata";
var BINARY_FORM_VERSION = 0;
var HEADER_BYTES = 7;
async function deserialize_binary_form(request, form_id) {
  if (request.headers.get("content-type") !== "application/x-sveltekit-formdata") {
    const form_data = await request.formData();
    return {
      data: convert_formdata(form_id, form_data),
      meta: {},
      form_data
    };
  }
  if (!request.body)
    throw deserialize_error("no body");
  const reader = request.body.getReader();
  const chunks = [];
  function get_chunk(index) {
    if (index in chunks)
      return chunks[index];
    let i = chunks.length;
    while (i <= index) {
      const previous = chunks[i - 1] ?? Promise.resolve(undefined);
      chunks[i] = previous.then(() => reader.read()).then((chunk) => chunk.value);
      i++;
    }
    return chunks[index];
  }
  async function get_buffer(offset, length) {
    const parts = [];
    let total = 0;
    for await (const part of read_range(get_chunk, offset, length)) {
      parts.push(part);
      total += part.byteLength;
    }
    if (total < length || parts.length === 0)
      return null;
    if (parts.length === 1)
      return parts[0];
    const buffer = new Uint8Array(length);
    let cursor = 0;
    for (const part of parts) {
      buffer.set(part, cursor);
      cursor += part.byteLength;
    }
    return buffer;
  }
  const header = await get_buffer(0, HEADER_BYTES);
  if (!header)
    throw deserialize_error("too short");
  if (header[0] !== BINARY_FORM_VERSION)
    throw deserialize_error(`got version ${header[0]}, expected version ${BINARY_FORM_VERSION}`);
  const header_view = new DataView(header.buffer, header.byteOffset, header.byteLength);
  const data_length = header_view.getUint32(1, true);
  const file_offsets_length = header_view.getUint16(5, true);
  const data_buffer = await get_buffer(HEADER_BYTES, data_length);
  if (!data_buffer)
    throw deserialize_error("data too short");
  let file_offsets;
  let files_start_offset;
  if (file_offsets_length > 0) {
    const file_offsets_buffer = await get_buffer(HEADER_BYTES + data_length, file_offsets_length);
    if (!file_offsets_buffer)
      throw deserialize_error("file offset table too short");
    const parsed_offsets = JSON.parse(text_decoder.decode(file_offsets_buffer));
    if (!Array.isArray(parsed_offsets) || parsed_offsets.some((n) => typeof n !== "number" || !Number.isInteger(n) || n < 0))
      throw deserialize_error("invalid file offset table");
    file_offsets = parsed_offsets;
    files_start_offset = HEADER_BYTES + data_length + file_offsets_length;
  }
  const file_spans = [];
  const [data, meta] = parse(text_decoder.decode(data_buffer), { File: ([name, type, size, last_modified, index]) => {
    if (typeof name !== "string" || typeof type !== "string" || typeof size !== "number" || typeof last_modified !== "number" || typeof index !== "number")
      throw deserialize_error("invalid file metadata");
    let offset = file_offsets[index];
    if (offset === undefined)
      throw deserialize_error("duplicate file offset table index");
    file_offsets[index] = undefined;
    offset += files_start_offset;
    file_spans.push({
      offset,
      size
    });
    return new Proxy(new LazyFile(name, type, size, last_modified, get_chunk, offset), { getPrototypeOf() {
      return File.prototype;
    } });
  } });
  file_spans.sort((a, b) => a.offset - b.offset || a.size - b.size);
  for (let i = 1;i < file_spans.length; i++) {
    const previous = file_spans[i - 1];
    const current = file_spans[i];
    const previous_end = previous.offset + previous.size;
    if (previous_end < current.offset)
      throw deserialize_error("gaps in file data");
    if (previous_end > current.offset)
      throw deserialize_error("overlapping file data");
  }
  (async () => {
    let has_more = true;
    while (has_more)
      has_more = !!await get_chunk(chunks.length);
  })().catch(noop);
  return {
    data,
    meta,
    form_data: null
  };
}
function deserialize_error(message) {
  return new SvelteKitError(400, "Bad Request", `Could not deserialize binary form: ${message}`);
}
async function* read_range(get_chunk, offset, length) {
  let chunk_start = 0;
  for (let index = 0;; index++) {
    const chunk = await get_chunk(index);
    if (!chunk)
      return;
    const chunk_end = chunk_start + chunk.byteLength;
    if (chunk_end > offset) {
      yield chunk.subarray(Math.max(0, offset - chunk_start), Math.min(chunk.byteLength, offset + length - chunk_start));
      if (offset + length <= chunk_end)
        return;
    }
    chunk_start = chunk_end;
  }
}
var LazyFile = class LazyFile2 {
  #get_chunk;
  #offset;
  constructor(name, type, size, last_modified, get_chunk, offset) {
    this.name = name;
    this.type = type;
    this.size = size;
    this.lastModified = last_modified;
    this.webkitRelativePath = "";
    this.#get_chunk = get_chunk;
    this.#offset = offset;
    this.arrayBuffer = this.arrayBuffer.bind(this);
    this.bytes = this.bytes.bind(this);
    this.slice = this.slice.bind(this);
    this.stream = this.stream.bind(this);
    this.text = this.text.bind(this);
  }
  #buffer;
  async arrayBuffer() {
    this.#buffer ??= await new Response(this.stream()).arrayBuffer();
    return this.#buffer;
  }
  async bytes() {
    return new Uint8Array(await this.arrayBuffer());
  }
  slice(start = 0, end = this.size, contentType = this.type) {
    if (start < 0)
      start = Math.max(this.size + start, 0);
    else
      start = Math.min(start, this.size);
    if (end < 0)
      end = Math.max(this.size + end, 0);
    else
      end = Math.min(end, this.size);
    const size = Math.max(end - start, 0);
    return new LazyFile2(this.name, contentType, size, this.lastModified, this.#get_chunk, this.#offset + start);
  }
  stream() {
    const range = read_range(this.#get_chunk, this.#offset, this.size);
    const size = this.size;
    return stream_from_iterable(async function* () {
      let cursor = 0;
      for await (const chunk of range) {
        cursor += chunk.byteLength;
        yield chunk;
      }
      if (cursor < size)
        throw new Error("incomplete file data");
    }());
  }
  async text() {
    return text_decoder.decode(await this.arrayBuffer());
  }
};
var path_regex = /^[a-zA-Z_$]\w*(\.[a-zA-Z_$]\w*|\[\d+\])*$/;
function split_path(path2) {
  if (!path_regex.test(path2))
    throw new Error(`Invalid path ${path2}`);
  return path2.split(/\.|\[|\]/).filter(Boolean);
}
function check_prototype_pollution(key) {
  if (key === "__proto__" || key === "constructor" || key === "prototype")
    throw new Error(`Invalid key "${key}"`);
}
function deep_set(object, keys, value) {
  let current = object;
  for (let i = 0;i < keys.length - 1; i += 1) {
    const key = keys[i];
    check_prototype_pollution(key);
    const is_array = /^\d+$/.test(keys[i + 1]);
    const inner = Object.hasOwn(current, key) ? current[key] : undefined;
    const exists = inner != null;
    if (exists && is_array !== Array.isArray(inner))
      throw new Error(`Invalid array key ${keys[i + 1]}`);
    if (!exists) {
      if (value === DELETE_KEY)
        return;
      current[key] = is_array ? [] : {};
    }
    current = current[key];
  }
  const final_key = keys[keys.length - 1];
  check_prototype_pollution(final_key);
  if (value === DELETE_KEY)
    delete current[final_key];
  else
    current[final_key] = value;
}
function negotiate2(accept, types) {
  const parts = [];
  accept.split(",").forEach((str, i) => {
    const match = /^[ \t]*([^/ \t]+)\/([^; \t]+)[ \t]*(?:;[ \t]*q=([0-9.]+))?/.exec(str);
    if (match) {
      const [, type, subtype, q = "1"] = match;
      parts.push({
        type,
        subtype,
        q: +q,
        i
      });
    }
  });
  parts.sort((a, b) => {
    if (a.q !== b.q)
      return b.q - a.q;
    if (a.subtype === "*" !== (b.subtype === "*"))
      return a.subtype === "*" ? 1 : -1;
    if (a.type === "*" !== (b.type === "*"))
      return a.type === "*" ? 1 : -1;
    return a.i - b.i;
  });
  let accepted;
  let min_priority = Infinity;
  for (const mimetype of types) {
    const [type, subtype] = mimetype.split("/");
    const priority = parts.findIndex((part) => (part.type === type || part.type === "*") && (part.subtype === subtype || part.subtype === "*"));
    if (priority !== -1 && priority < min_priority) {
      accepted = mimetype;
      min_priority = priority;
    }
  }
  return accepted;
}
function matches_content_type(header, ...types) {
  const type = header?.split(";", 1)[0].trim() ?? "";
  return types.includes(type.toLowerCase());
}
function is_form_content_type(request) {
  return matches_content_type(request.headers.get("content-type"), "application/x-www-form-urlencoded", "multipart/form-data", "text/plain", BINARY_FORM_CONTENT_TYPE);
}
var uneval2 = () => {
  throw new Error("");
};
var stringify2 = () => {
  throw new Error("");
};
var parse2 = () => {
  throw new Error("");
};
var encoders = {};
var decoders = {};
var has_custom_transporters = false;
function init_transport(transport) {
  const transporters = Object.entries(transport);
  has_custom_transporters = transporters.length > 0;
  const replacer = (thing) => {
    for (const key of Object.keys(transport)) {
      const encoded = transport[key].encode(thing);
      if (encoded)
        return `app.decode('${key}', ${uneval(encoded, replacer)})`;
    }
  };
  encoders = Object.fromEntries(transporters.map(([k, v]) => [k, v.encode]));
  decoders = Object.fromEntries(transporters.map(([k, v]) => [k, v.decode]));
  uneval2 = (data) => uneval(data, replacer);
  stringify2 = (data) => stringify(data, encoders);
  parse2 = (data) => parse(data, decoders);
}
var INVALIDATED_PARAM = "x-sveltekit-invalidated";
var TRAILING_SLASH_PARAM = "x-sveltekit-trailing-slash";
var remote_object = "__skrao";
var remote_map = "__skram";
var remote_set = "__skras";
var remote_file = "__skraf";
var remote_arg_marker = Symbol(remote_object);
function create_remote_arg_revivers() {
  const remote_fns_revivers = {
    [remote_object]: (value) => value,
    [remote_map]: (value) => {
      if (!Array.isArray(value))
        throw new Error("Invalid data for Map reviver");
      const map = /* @__PURE__ */ new Map;
      for (const item of value) {
        if (!Array.isArray(item) || item.length !== 2 || typeof item[0] !== "string" || typeof item[1] !== "string")
          throw new Error("Invalid data for Map reviver");
        const [key, val] = item;
        map.set(parse3(key), parse3(val));
      }
      return map;
    },
    [remote_set]: (value) => {
      if (!Array.isArray(value))
        throw new Error("Invalid data for Set reviver");
      const set = /* @__PURE__ */ new Set;
      for (const item of value) {
        if (typeof item !== "string")
          throw new Error("Invalid data for Set reviver");
        set.add(parse3(item));
      }
      return set;
    },
    [remote_file]: (value) => {
      if (!value || typeof value !== "object" || typeof value.name !== "string" || typeof value.type !== "string" || typeof value.size !== "number" || typeof value.lastModified !== "number" || !(value.data instanceof ArrayBuffer))
        throw new Error("Invalid data for File reviver");
      const { data, name, ...meta } = value;
      return new File([data], name, meta);
    }
  };
  const all_revivers = {
    ...decoders,
    ...remote_fns_revivers
  };
  const parse3 = (data) => parse(data, all_revivers);
  return all_revivers;
}
function parse_remote_arg(string) {
  if (!string)
    return;
  const json_string = text_decoder.decode(base64_decode(string.replaceAll("-", "+").replaceAll("_", "/")));
  return parse(json_string, create_remote_arg_revivers());
}
function create_remote_key(id, payload) {
  return id + "/" + payload;
}
function split_remote_key(key) {
  const i = key.lastIndexOf("/");
  if (i === -1)
    throw new Error(`Invalid remote key: ${key}`);
  return {
    id: key.slice(0, i),
    payload: key.slice(i + 1)
  };
}
function coalesce_to_error(err) {
  return err instanceof Error || err && err.name && err.message ? err : new Error(JSON.stringify(err));
}
function normalize_error(error2) {
  return error2;
}
function get_status(error2) {
  return error2 instanceof HttpError || error2 instanceof SvelteKitError ? error2.status : 500;
}
var escape_html_attr_dict = {
  "&": "&amp;",
  '"': "&quot;"
};
var escape_html_dict = {
  "&": "&amp;",
  "<": "&lt;"
};
var escape_regex = (dict) => new RegExp(`[${Object.keys(dict).join("")}]|\\p{Surrogate}`, "gu");
var escape_html_attr_regex = escape_regex(escape_html_attr_dict);
var escape_html_regex = escape_regex(escape_html_dict);
function escape_html2(str, is_attr) {
  const dict = is_attr ? escape_html_attr_dict : escape_html_dict;
  return str.replace(is_attr ? escape_html_attr_regex : escape_html_regex, (match) => dict[match] ?? `&#${match.charCodeAt(0)};`);
}
async function handle_fatal_error(event, state, error2) {
  const body = await handle_error_and_jsonify(event, state, error2);
  const status = body.status;
  const type = negotiate2(event.request.headers.get("accept") || "text/html", ["application/json", "text/html"]);
  if (event.isDataRequest || type === "application/json")
    return Response.json(body, { status });
  return static_error_page(status, body.message);
}
function handle_error_and_jsonify(event, state, error2) {
  if (error2 instanceof HandledHttpError)
    return error2.body;
  let caught;
  if (error2 instanceof HttpError)
    caught = {
      kind: "app",
      error: error2.body
    };
  else if (error2 instanceof SvelteKitError)
    caught = {
      kind: "framework",
      error: {
        status: error2.status,
        message: error2.text
      }
    };
  else if (error2 instanceof ValidationError)
    caught = {
      kind: "validation",
      error: {
        status: 400,
        message: "Bad Request"
      },
      issues: error2.issues
    };
  else {
    caught = {
      kind: "unknown",
      error: error2
    };
    let e = error2;
    while (e instanceof Error) {
      fix_stack_trace(e);
      e = e.cause;
    }
  }
  const fallback = caught.kind === "unknown" ? {
    status: 500,
    message: "Internal Error"
  } : caught.error;
  function merge(body) {
    return {
      ...fallback,
      ...body
    };
  }
  let result;
  try {
    const input = {
      ...caught,
      event
    };
    result = with_request_store({
      event,
      state
    }, () => hooks.handleError(input));
  } catch (hook_error) {
    log_handle_error_hook_failure(error2, hook_error);
    return {
      status: fallback.status,
      message: "Internal Error"
    };
  }
  if (result instanceof Promise) {
    if (state.is_in_render) {
      console.warn(`To use an async \`handleError\` hook to handle errors that occur during rendering, you must enable \`compilerOptions.experimental.async\` in the SvelteKit plugin of your Vite config. The returned error has been replaced with a generic object`);
      result.catch((hook_error) => log_handle_error_hook_failure(error2, hook_error));
      return {
        status: fallback.status,
        message: "Internal Error"
      };
    }
    return result.then(merge, (hook_error) => {
      log_handle_error_hook_failure(error2, hook_error);
      return {
        status: fallback.status,
        message: "Internal Error"
      };
    });
  }
  return merge(result);
}
function log_handle_error_hook_failure(error2, hook_error) {
  const failure = new Error("The `handleError` hook failed", { cause: coalesce_to_error(hook_error) });
  failure.stack = failure.message;
  console.error(failure);
  if (error2 instanceof SvelteKitError)
    console.error(`Original error: ${error2.status} ${error2.text}: ${error2.message}`);
  else
    console.error("Original error:", error2);
}
function static_error_page(status, message) {
  let page = options$1.templates.error({
    status,
    message: escape_html2(message)
  });
  return text(page, {
    headers: { "content-type": "text/html; charset=utf-8" },
    status
  });
}
function method_not_allowed(mod, method) {
  return text(`${method} method not allowed`, {
    status: 405,
    headers: { allow: allowed_methods(mod).join(", ") }
  });
}
function allowed_methods(mod) {
  const allowed = ENDPOINT_METHODS.filter((method) => (method in mod));
  if ("GET" in mod && !("HEAD" in mod))
    allowed.push("HEAD");
  return allowed;
}
function redirect_response(status, location) {
  return new Response(undefined, {
    status,
    headers: { location }
  });
}
function with_version_header(response) {
  response.headers.set("x-sveltekit-version", "1788652438838");
  return response;
}
function clarify_devalue_error(event, error2) {
  if (error2.path)
    return `Data returned from \`load\` while rendering ${event.route.id} is not serializable: ${error2.message} (${error2.path}). If you need to serialize/deserialize custom types, use transport hooks: https://svelte.dev/docs/kit/hooks#transport.`;
  if (error2.path === "")
    return `Data returned from \`load\` while rendering ${event.route.id} is not a plain object`;
  return error2.message;
}
function serialize_uses(node) {
  const uses = {};
  if (node.uses && node.uses.dependencies.size > 0)
    uses.dependencies = Array.from(node.uses.dependencies);
  if (node.uses && node.uses.search_params.size > 0)
    uses.search_params = Array.from(node.uses.search_params);
  if (node.uses && node.uses.params.size > 0)
    uses.params = Array.from(node.uses.params);
  if (node.uses?.parent)
    uses.parent = 1;
  if (node.uses?.route)
    uses.route = 1;
  if (node.uses?.url)
    uses.url = 1;
  return uses;
}
function has_prerendered_path(manifest2, pathname) {
  return manifest2._.prerendered_routes.has(pathname) || pathname.at(-1) === "/" && manifest2._.prerendered_routes.has(pathname.slice(0, -1));
}
function get_node_type(node_id) {
  const filename = node_id?.split("/")?.at(-1);
  if (!filename)
    return "unknown";
  return filename.split(".").slice(0, -1).join(".");
}
function is_action_json_request(event) {
  return negotiate2(event.request.headers.get("accept") ?? "*/*", ["application/json", "text/html"]) === "application/json" && event.request.method === "POST";
}
async function handle_action_json_request(event, state, server) {
  return action_result_json(event, state, await handle_action_request(event, state, server));
}
async function action_result_json(event, state, result) {
  if (result.type === "redirect")
    return action_json(result);
  if (result.type === "error") {
    const error2 = await handle_error_and_jsonify(event, state, result.error);
    return action_json({
      ...result,
      error: error2
    }, { status: error2.status });
  }
  if (result.type === "success" && !result.data)
    return action_json({
      ...result,
      status: 204,
      data: undefined
    });
  try {
    return action_json({
      ...result,
      data: try_serialize(result.data, stringify2, event.route.id)
    }, { status: result.status });
  } catch (e) {
    return action_result_json(event, state, action_error_result(e, result.location));
  }
}
function get_action_location(url) {
  const location = new URL(url);
  for (const key of location.searchParams.keys())
    if (key.startsWith("/")) {
      location.searchParams.delete(key);
      break;
    }
  return location.pathname + location.search;
}
function method_not_allowed_result(event, location) {
  event.setHeaders({ allow: "GET" });
  return {
    type: "error",
    location,
    error: new SvelteKitError(405, "Method Not Allowed", `POST method not allowed. No form actions exist for this page`)
  };
}
function action_error_result(e, location) {
  const err = normalize_error(e);
  if (err instanceof Redirect)
    return {
      type: "redirect",
      status: err.status,
      location: err.location
    };
  return {
    type: "error",
    location,
    error: err
  };
}
function action_json_redirect(redirect) {
  return action_json({
    type: "redirect",
    status: redirect.status,
    location: redirect.location
  });
}
function action_json(data, init) {
  return with_version_header(Response.json(data, init));
}
function is_action_request(event) {
  return event.request.method === "POST";
}
async function handle_action_request(event, state, server) {
  const actions = server?.actions;
  const location = get_action_location(event.url);
  if (!actions)
    return method_not_allowed_result(event, location);
  check_named_default_separate(actions);
  try {
    const data = await call_action(event, state, actions);
    if (data instanceof ActionFailure)
      return {
        type: "failure",
        status: data.status,
        location,
        data: data.data
      };
    else
      return {
        type: "success",
        status: 200,
        location,
        data
      };
  } catch (e) {
    return action_error_result(e instanceof ActionFailure ? /* @__PURE__ */ new Error('Cannot "throw fail()". Use "return fail()"') : e, location);
  }
}
function check_named_default_separate(actions) {
  if (actions.default && Object.keys(actions).length > 1)
    throw new Error("When using named actions, the default action cannot be used. See the docs for more info: https://svelte.dev/docs/kit/form-actions#named-actions");
}
async function call_action(event, state, actions) {
  const url = new URL(event.request.url);
  let name = "default";
  for (const param of url.searchParams)
    if (param[0].startsWith("/")) {
      name = param[0].slice(1);
      if (name === "default")
        throw new Error('Cannot use reserved action name "default"');
      break;
    }
  if (!Object.hasOwn(actions, name))
    throw new SvelteKitError(404, "Not Found", `No action with name '${name}' found`);
  const action = actions[name];
  if (!is_form_content_type(event.request))
    throw new SvelteKitError(415, "Unsupported Media Type", `Form actions expect form-encoded data \u2014 received ${event.request.headers.get("content-type")}`);
  return record_span({
    name: "sveltekit.form_action",
    attributes: {
      "sveltekit.form_action.name": name,
      "http.route": event.route.id || "unknown"
    },
    fn: async (current) => {
      const traced_event = merge_tracing(event, current);
      const result = await with_request_store({
        event: traced_event,
        state
      }, () => action(traced_event));
      if (result instanceof ActionFailure)
        current.setAttributes({
          "sveltekit.form_action.result.type": "failure",
          "sveltekit.form_action.result.status": result.status
        });
      return result;
    }
  });
}
function uneval_action_response(data, route_id) {
  return try_serialize(data, uneval2, route_id);
}
function try_serialize(data, fn, route_id) {
  try {
    return fn(data);
  } catch (e) {
    const error2 = e;
    if (data instanceof Response)
      throw new Error(`Data returned from action inside ${route_id} is not serializable. Form actions need to return plain objects or fail(). E.g. return { success: true } or return fail(400, { message: "invalid" });`, { cause: e });
    if ("path" in error2) {
      let message = `Data returned from action inside ${route_id} is not serializable: ${error2.message}`;
      if (error2.path !== "")
        message += ` (data.${error2.path})`;
      throw new Error(message, { cause: e });
    }
    throw error2;
  }
}
var KEEP_ALIVE_INTERVAL = 30000;
function create_live_query_response(event, state, internals, arg) {
  const cancellation = new AbortController;
  const live_event = {
    ...event,
    request: new Request(event.request, { signal: AbortSignal.any([event.request.signal, cancellation.signal]) })
  };
  const generator = internals.run(live_event, state, arg);
  let open = true;
  let pulling = false;
  let stream_controller;
  let keep_alive;
  let result;
  function schedule_keep_alive() {
    clearTimeout(keep_alive);
    keep_alive = setTimeout(() => {
      if (!open)
        return;
      if ((stream_controller.desiredSize ?? 0) > 0)
        stream_controller.enqueue(text_encoder.encode(`: keep-alive

`));
      schedule_keep_alive();
    }, KEEP_ALIVE_INTERVAL);
  }
  function send(data) {
    if (!open)
      return;
    stream_controller.enqueue(text_encoder.encode("data: " + JSON.stringify(data) + `

`));
    schedule_keep_alive();
  }
  function teardown(cancelled) {
    if (!open)
      return;
    open = false;
    clearTimeout(keep_alive);
    cancellation.abort();
    if (!cancelled)
      stream_controller.close();
    generator.return(undefined).catch(() => {});
  }
  event.request.signal.addEventListener("abort", () => teardown(true), { once: true });
  return new Response(new ReadableStream({
    start(controller) {
      stream_controller = controller;
      schedule_keep_alive();
    },
    async pull() {
      if (!open || pulling)
        return;
      pulling = true;
      try {
        while (open) {
          const { value, done } = await generator.next();
          if (!open)
            return;
          if (done) {
            teardown(false);
            return;
          }
          if (result !== (result = stringify2(value))) {
            send({
              type: "result",
              result
            });
            return;
          }
        }
      } catch (error2) {
        if (!open)
          return;
        if (error2 instanceof Redirect)
          send({
            type: "redirect",
            location: error2.location
          });
        else
          send({
            type: "error",
            error: await handle_error_and_jsonify(event, state, error2)
          });
        teardown(false);
      } finally {
        pulling = false;
      }
    },
    cancel() {
      teardown(true);
    }
  }), { headers: {
    "cache-control": "private, no-store",
    "content-type": "text/event-stream"
  } });
}
async function handle_remote_call(event, state, manifest2, id) {
  return record_span({
    name: "sveltekit.remote.call",
    attributes: { "sveltekit.remote.call.id": id },
    fn: async (current) => {
      const traced_event = merge_tracing(event, current);
      return with_version_header(await with_request_store({
        event: traced_event,
        state
      }, () => handle_remote_call_internal(traced_event, state, manifest2, id)));
    }
  });
}
async function handle_remote_call_internal(event, state, manifest2, id) {
  const [hash, name, additional_args] = id.split("/");
  const remotes = manifest2._.remotes;
  if (!Object.hasOwn(remotes, hash))
    error(404);
  const module = await remotes[hash]();
  const fn = Object.hasOwn(module.default, name) ? module.default[name] : undefined;
  if (!fn)
    error(404);
  const internals = fn.__;
  event.tracing.current.setAttributes({
    "sveltekit.remote.call.type": internals.type,
    "sveltekit.remote.call.name": internals.name
  });
  const headers = state.prerendering ? undefined : { "cache-control": "private, no-store" };
  try {
    const data = {};
    switch (internals.type) {
      case "query_live":
        if (event.request.method !== "GET")
          throw new SvelteKitError(405, "Method Not Allowed", `\`query.live\` functions must be invoked via GET request, not ${event.request.method}`);
        return create_live_query_response(event, state, internals, parse_remote_arg(new URL(event.request.url).searchParams.get("payload")));
      case "query_batch": {
        if (event.request.method !== "POST")
          throw new SvelteKitError(405, "Method Not Allowed", `\`query.batch\` functions must be invoked via POST request, not ${event.request.method}`);
        const { payloads } = await event.request.json();
        const args = await Promise.all(payloads.map((payload) => parse_remote_arg(payload)));
        data._ = await with_request_store({
          event,
          state
        }, () => internals.run(args));
        break;
      }
      case "form": {
        if (event.request.method !== "POST")
          throw new SvelteKitError(405, "Method Not Allowed", `\`form\` functions must be invoked via POST request, not ${event.request.method}`);
        if (!is_form_content_type(event.request))
          throw new SvelteKitError(415, "Unsupported Media Type", `\`form\` functions expect form-encoded data \u2014 received ${event.request.headers.get("content-type")}`);
        const { data: input, meta, form_data } = await deserialize_binary_form(event.request, internals.id);
        state.remote.requested = create_requested_map(meta.remote_refreshes);
        if (additional_args && !("id" in input))
          input.id = JSON.parse(decodeURIComponent(additional_args));
        const fn2 = internals.fn;
        data._ = await with_request_store({
          event,
          state: {
            ...state,
            is_in_remote_form_or_command: true
          }
        }, () => fn2(input, meta, form_data));
        if (data._.issues)
          return Response.json({
            type: "result",
            data: stringify2(data)
          }, { headers });
        break;
      }
      case "command": {
        const { payload, refreshes } = await event.request.json();
        state.remote.requested = create_requested_map(refreshes);
        const arg = parse_remote_arg(payload);
        data._ = await with_request_store({
          event,
          state: {
            ...state,
            is_in_remote_form_or_command: true
          }
        }, () => fn(arg));
        break;
      }
      case "prerender":
        data._ = await with_request_store({
          event,
          state
        }, () => fn(parse_remote_arg(additional_args)));
        break;
      case "query": {
        const payload = new URL(event.request.url).searchParams.get("payload");
        data._ = await with_request_store({
          event,
          state
        }, () => fn(parse_remote_arg(payload)));
        break;
      }
    }
    await collect_remote_data(data, event, state);
    return Response.json({
      type: "result",
      data: stringify2(data)
    }, { headers });
  } catch (error2) {
    if (error2 instanceof Redirect) {
      const data = await collect_remote_data({ redirect: error2.location }, event, state);
      return Response.json({
        type: "result",
        data: stringify2(data)
      }, { headers });
    }
    const transformed = await handle_error_and_jsonify(event, state, error2);
    return Response.json({
      type: "error",
      error: transformed
    }, {
      status: state.prerendering ? transformed.status : undefined,
      headers: { "cache-control": "private, no-store" }
    });
  }
}
async function collect_remote_data(data, event, state) {
  function convert_error(error2) {
    return Promise.resolve(handle_error_and_jsonify(event, state, error2));
  }
  const promises = [];
  const processed = /* @__PURE__ */ new Set;
  if (state.remote.explicit) {
    const { explicit } = state.remote;
    const inflight = [];
    const drain = () => {
      for (const [remote_key, { internals, fn }] of explicit) {
        explicit.delete(remote_key);
        if (processed.has(remote_key))
          continue;
        processed.add(remote_key);
        data.r = true;
        const type = internals.type === "query_live" ? "l" : internals.type[0];
        inflight.push(fn().then((v) => {
          (data[type] ??= {})[remote_key] = { v };
          drain();
        }, async (e) => {
          if (!(e instanceof Redirect))
            (data[type] ??= {})[remote_key] = { e: await convert_error(e) };
          drain();
        }));
      }
    };
    drain();
    for (const promise of inflight)
      await promise;
  }
  if (state.remote.implicit)
    for (const [internals, record] of state.remote.implicit) {
      if (!internals.id)
        continue;
      for (const key in record) {
        const remote_key = internals.type === "form" ? key : create_remote_key(internals.id, key);
        if (processed.has(remote_key))
          continue;
        const type = internals.type === "query_live" ? "l" : internals.type[0];
        const promise = state.remote.data?.get(internals)?.[key] ?? record[key]();
        let resolved = true;
        await Promise.race([Promise.resolve(promise).then((v) => {
          if (resolved)
            ((data[type] ??= {})[remote_key] ??= {}).v = v;
        }, (e) => {
          if (e instanceof Redirect)
            return;
          if (resolved)
            promises.push(convert_error(e).then((e2) => {
              ((data[type] ??= {})[remote_key] ??= {}).e = e2;
            }));
        }), Promise.resolve().then(() => resolved = false)]);
      }
    }
  await Promise.all(promises);
  return data;
}
function create_requested_map(refreshes) {
  const requested = /* @__PURE__ */ new Map;
  for (const key of refreshes ?? []) {
    const parts = split_remote_key(key);
    const existing = requested.get(parts.id);
    if (existing)
      existing.push(parts.payload);
    else
      requested.set(parts.id, [parts.payload]);
  }
  return requested;
}
async function handle_remote_form_post(event, state, manifest2, id) {
  return record_span({
    name: "sveltekit.remote.form.post",
    attributes: { "sveltekit.remote.form.post.id": id },
    fn: (current) => {
      const traced_event = merge_tracing(event, current);
      return with_request_store({
        event: traced_event,
        state
      }, () => handle_remote_form_post_internal(traced_event, state, manifest2, id));
    }
  });
}
async function handle_remote_form_post_internal(event, state, manifest2, id) {
  const location = get_action_location(event.url);
  const [hash, name, ...rest] = id.split("/");
  const action_id = rest.join("/");
  const remotes = manifest2._.remotes;
  const module = Object.hasOwn(remotes, hash) ? await remotes[hash]() : undefined;
  let form = module && Object.hasOwn(module.default, name) ? module.default[name] : undefined;
  if (!form)
    return method_not_allowed_result(event, location);
  if (action_id)
    form = with_request_store({
      event,
      state
    }, () => form.for(JSON.parse(action_id)));
  try {
    const __ = form.__;
    const { data, meta, form_data } = await deserialize_binary_form(event.request, __.id);
    if (action_id && !("id" in data))
      data.id = JSON.parse(decodeURIComponent(action_id));
    await with_request_store({
      event,
      state: {
        ...state,
        is_in_remote_form_or_command: true
      }
    }, () => __.fn(data, meta, form_data));
    return {
      type: "success",
      status: 200,
      location
    };
  } catch (e) {
    return action_error_result(e, location);
  }
}
function has_remote_prefix(url) {
  return url.pathname.startsWith(`/${app_dir}/remote/`);
}
function strip_remote_prefix(url) {
  return url.pathname.replace(`/${app_dir}/remote/`, "");
}
function get_remote_id(url) {
  return has_remote_prefix(url) && strip_remote_prefix(url);
}
function get_remote_action(url) {
  return url.searchParams.get("/remote");
}
var fs = globalThis.process?.getBuiltinModule?.("node:fs");
var url = globalThis.process?.getBuiltinModule?.("node:url");
var path2 = globalThis.process?.getBuiltinModule?.("node:path");
var module = globalThis.process?.getBuiltinModule?.("node:module");
var cwd = globalThis.process?.cwd?.();
var relative = cwd ? (file) => path2.relative(cwd, file) : (file) => file;
var fix_stack_trace = (error2) => {
  if (!error2.stack || !fs)
    return;
  let end = 0;
  error2.stack = error2.stack.split(`
`).map((line, i) => {
    const match = line.match(/^ {4}at.+(file:\/\/\/.*):(\d+):(\d+)(\)?)$/);
    if (!match) {
      if (!line.includes("node:internal/"))
        end = i + 1;
      return line;
    }
    const file = url.fileURLToPath(match[1]);
    const traced = trace(file, Number(match[2]) - 1, Number(match[3]) - 1);
    if (!/[\\/]node_modules[\\/]/.test(traced?.file ?? file))
      end = i + 1;
    if (traced?.line) {
      const location = `${match[1]}:${match[2]}:${match[3]}`;
      const original = `${relative(traced.file)}:${traced.line}:${traced.column}`;
      return line.replace(location, original);
    }
    if (traced)
      return `${line.replace(match[1], relative(file))} [${traced.file}]`;
    return line;
  }).slice(0, end).join(`
`);
};
var source_maps = /* @__PURE__ */ new Map;
var source_regions = /* @__PURE__ */ new Map;
function get_source_map(file) {
  if (source_maps.has(file))
    return source_maps.get(file);
  try {
    let source;
    let directory = path2.dirname(file);
    const code = fs.readFileSync(file, "utf8");
    const url2 = Array.from(code.matchAll(/\/\/[#@]\s*sourceMappingURL=(\S+)/g)).at(-1)?.[1];
    if (url2?.startsWith("data:")) {
      const comma = url2.indexOf(",");
      const metadata = url2.slice(5, comma);
      const data = url2.slice(comma + 1);
      source = metadata.endsWith(";base64") ? Buffer.from(data, "base64").toString() : decodeURIComponent(data);
    } else {
      const map_file = url2 ? path2.resolve(path2.dirname(file), decodeURIComponent(url2)) : `${file}.map`;
      if (fs.existsSync(map_file)) {
        directory = path2.dirname(map_file);
        source = fs.readFileSync(map_file, "utf8");
      }
    }
    if (source) {
      const source_map = {
        map: new module.SourceMap(JSON.parse(source)),
        directory
      };
      source_maps.set(file, source_map);
      return source_map;
    }
  } catch {}
  source_maps.set(file, null);
  return null;
}
function trace(file, line, column) {
  const source_map = get_source_map(file);
  if (!source_map)
    return null;
  const entry = source_map.map.findEntry(line, column);
  if (entry && "originalSource" in entry && entry.originalSource && typeof entry.originalLine === "number" && typeof entry.originalColumn === "number") {
    const traced = {
      file: entry.originalSource.startsWith("file:") ? url.fileURLToPath(entry.originalSource) : path2.resolve(source_map.directory, entry.originalSource),
      line: entry.originalLine + 1,
      column: entry.originalColumn + 1
    };
    return trace(traced.file, traced.line - 1, traced.column - 1) ?? traced;
  }
  let regions = source_regions.get(file);
  if (!regions) {
    let source2;
    regions = fs.readFileSync(file, "utf8").split(`
`).map((line2) => {
      const start = line2.match(/^\/\/#region (.+)$/);
      if (start)
        source2 = start[1];
      if (line2 === "//#endregion")
        source2 = undefined;
      return source2;
    });
    source_regions.set(file, regions);
  }
  const source = regions[line];
  if (source)
    return { file: source };
  return null;
}
var styleText = globalThis.process?.getBuiltinModule?.("node:util")?.styleText ?? ((_format, text2) => text2);
var read_implementation = false;
var options$1 = false;
var hooks = false;
function set_read_implementation(fn) {
  read_implementation = fn;
}
function set_manifest(value) {}
function set_options(value) {
  options$1 = value;
}
function set_hooks(value) {
  hooks = value;
}
var error_template_default = ({ status, message }) => `<!doctype html>
<html lang="en">
	<head>
		<meta charset="utf-8" />
		<title>` + message + `</title>

		<style>
			body {
				--bg: white;
				--fg: #222;
				--divider: #ccc;
				background: var(--bg);
				color: var(--fg);
				font-family:
					system-ui,
					-apple-system,
					BlinkMacSystemFont,
					'Segoe UI',
					Roboto,
					Oxygen,
					Ubuntu,
					Cantarell,
					'Open Sans',
					'Helvetica Neue',
					sans-serif;
				display: flex;
				align-items: center;
				justify-content: center;
				height: 100vh;
				margin: 0;
			}

			.error {
				display: flex;
				align-items: center;
				max-width: 32rem;
				margin: 0 1rem;
			}

			.status {
				font-weight: 200;
				font-size: 3rem;
				line-height: 1;
				position: relative;
				top: -0.05rem;
			}

			.message {
				border-left: 1px solid var(--divider);
				padding: 0 0 0 1rem;
				margin: 0 0 0 1rem;
				min-height: 2.5rem;
				display: flex;
				align-items: center;
			}

			.message h1 {
				font-weight: 400;
				font-size: 1em;
				margin: 0;
			}

			@media (prefers-color-scheme: dark) {
				body {
					--bg: #222;
					--fg: #ddd;
					--divider: #666;
				}
			}
		</style>
	</head>
	<body>
		<div class="error">
			<span class="status">` + status + `</span>
			<div class="message">
				<h1>` + message + `</h1>
			</div>
		</div>
	</body>
</html>
`;
var options = {
  app_template_contains_nonce: false,
  csp: {
    mode: "auto",
    directives: {
      "upgrade-insecure-requests": false,
      "block-all-mixed-content": false
    },
    reportOnly: {
      "upgrade-insecure-requests": false,
      "block-all-mixed-content": false
    }
  },
  csrf_trusted_origins: [],
  service_worker_options: undefined,
  templates: {
    app: ({ head, body, assets: assets2, nonce, env }) => `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link
      href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;600&display=swap"
      rel="stylesheet"
    />
    ` + head + `
  </head>
  <body data-sveltekit-preload-data="hover">
    <div style="display: contents">` + body + `</div>
  </body>
</html>
`,
    error: error_template_default
  }
};
async function get_hooks() {
  let handle;
  let handleFetch;
  let handleError;
  let init;
  ({ handle, handleFetch, handleError, init } = await import("./server/chunks/hooks.server-vs0f4esk.js"));
  let reroute;
  let transport;
  return {
    handle,
    handleFetch,
    handleError,
    init,
    reroute,
    transport
  };
}

// .svelte-kit/output/server/env.js
var explicit_public_env = {};
var rendered_env = {};

// node_modules/cookie/dist/index.js
var cookieNameRegExp = /^[\u0021-\u003A\u003C\u003E-\u007E]+$/;
var cookieValueRegExp = /^[\u0021-\u003A\u003C-\u007E]*$/;
var domainValueRegExp = /^([.]?[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)([.][a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*$/i;
var pathValueRegExp = /^[\u0020-\u003A\u003D-\u007E]*$/;
var maxAgeRegExp = /^-?\d+$/;
var cookieOctetRegExp = /^[!#$&'()*+\-./0-9:<=>?@A-Z[\]^_`a-z{|}~]*$/;
var NullObject = /* @__PURE__ */ (() => {
  const C = function() {};
  C.prototype = Object.create(null);
  return C;
})();
function parseCookie(str, options2) {
  const obj = new NullObject;
  const len = str.length;
  if (len < 2)
    return obj;
  const dec = options2?.decode || decode;
  let index = 0;
  do {
    const eqIdx = eqIndex(str, index, len);
    if (eqIdx === len)
      break;
    const endIdx = endIndex(str, index, len);
    if (eqIdx > endIdx) {
      index = str.lastIndexOf(";", eqIdx - 1) + 1;
      continue;
    }
    const key = valueSlice(str, index, eqIdx);
    if (obj[key] === undefined) {
      obj[key] = dec(valueSlice(str, eqIdx + 1, endIdx));
    }
    index = endIdx + 1;
  } while (index < len);
  return obj;
}
function stringifySetCookie(cookie, options2) {
  const enc = options2?.encode || defaultEncode;
  if (!cookieNameRegExp.test(cookie.name)) {
    throw new TypeError(`argument name is invalid: ${cookie.name}`);
  }
  const value = cookie.value == null ? "" : enc(cookie.value);
  if (!cookieValueRegExp.test(value)) {
    throw new TypeError(`argument val is invalid: ${cookie.value}`);
  }
  let str = cookie.name + "=" + value;
  if (cookie.maxAge !== undefined) {
    if (!Number.isInteger(cookie.maxAge)) {
      throw new TypeError(`option maxAge is invalid: ${cookie.maxAge}`);
    }
    str += "; Max-Age=" + cookie.maxAge;
  }
  if (cookie.domain) {
    if (!domainValueRegExp.test(cookie.domain)) {
      throw new TypeError(`option domain is invalid: ${cookie.domain}`);
    }
    str += "; Domain=" + cookie.domain;
  }
  if (cookie.path) {
    if (!pathValueRegExp.test(cookie.path)) {
      throw new TypeError(`option path is invalid: ${cookie.path}`);
    }
    str += "; Path=" + cookie.path;
  }
  if (cookie.expires) {
    if (!Number.isFinite(cookie.expires.valueOf())) {
      throw new TypeError(`option expires is invalid: ${cookie.expires}`);
    }
    str += "; Expires=" + cookie.expires.toUTCString();
  }
  if (cookie.httpOnly) {
    str += "; HttpOnly";
  }
  if (cookie.secure) {
    str += "; Secure";
  }
  if (cookie.partitioned) {
    str += "; Partitioned";
  }
  if (cookie.priority) {
    const priority = typeof cookie.priority === "string" ? cookie.priority.toLowerCase() : undefined;
    switch (priority) {
      case "low":
        str += "; Priority=Low";
        break;
      case "medium":
        str += "; Priority=Medium";
        break;
      case "high":
        str += "; Priority=High";
        break;
      default:
        throw new TypeError(`option priority is invalid: ${cookie.priority}`);
    }
  }
  if (cookie.sameSite) {
    const sameSite = typeof cookie.sameSite === "string" ? cookie.sameSite.toLowerCase() : cookie.sameSite;
    switch (sameSite) {
      case true:
      case "strict":
        str += "; SameSite=Strict";
        break;
      case "lax":
        str += "; SameSite=Lax";
        break;
      case "none":
        str += "; SameSite=None";
        break;
      default:
        throw new TypeError(`option sameSite is invalid: ${cookie.sameSite}`);
    }
  }
  return str;
}
function parseSetCookie(str, options2) {
  const dec = options2?.decode || decode;
  const len = str.length;
  const endIdx = endIndex(str, 0, len);
  let eqIdx = eqIndex(str, 0, len);
  const setCookie = eqIdx < endIdx ? {
    name: valueSlice(str, 0, eqIdx),
    value: dec(valueSlice(str, eqIdx + 1, endIdx))
  } : { name: "", value: dec(valueSlice(str, 0, endIdx)) };
  let index = endIdx + 1;
  while (index < len) {
    const endIdx2 = endIndex(str, index, len);
    if (eqIdx < index)
      eqIdx = eqIndex(str, index, len);
    const attr = eqIdx < endIdx2 ? valueSlice(str, index, eqIdx) : valueSlice(str, index, endIdx2);
    const val = eqIdx < endIdx2 ? valueSlice(str, eqIdx + 1, endIdx2) : undefined;
    switch (attr.toLowerCase()) {
      case "httponly":
        setCookie.httpOnly = true;
        break;
      case "secure":
        setCookie.secure = true;
        break;
      case "partitioned":
        setCookie.partitioned = true;
        break;
      case "domain":
        setCookie.domain = val;
        break;
      case "path":
        setCookie.path = val;
        break;
      case "max-age":
        if (val && maxAgeRegExp.test(val))
          setCookie.maxAge = Number(val);
        break;
      case "expires":
        if (!val)
          break;
        const date = new Date(val);
        if (Number.isFinite(date.valueOf()))
          setCookie.expires = date;
        break;
      case "priority":
        if (!val)
          break;
        const priority = val.toLowerCase();
        if (priority === "low" || priority === "medium" || priority === "high") {
          setCookie.priority = priority;
        }
        break;
      case "samesite":
        if (!val)
          break;
        const sameSite = val.toLowerCase();
        if (sameSite === "lax" || sameSite === "strict" || sameSite === "none") {
          setCookie.sameSite = sameSite;
        }
        break;
    }
    index = endIdx2 + 1;
  }
  return setCookie;
}
function endIndex(str, min, len) {
  const index = str.indexOf(";", min);
  return index === -1 ? len : index;
}
function eqIndex(str, min, len) {
  const index = str.indexOf("=", min);
  return index === -1 ? len : index;
}
function valueSlice(str, min, max) {
  if (min === max)
    return "";
  let start = min;
  let end = max;
  do {
    const code = str.charCodeAt(start);
    if (code !== 32 && code !== 9)
      break;
  } while (++start < end);
  while (end > start) {
    const code = str.charCodeAt(end - 1);
    if (code !== 32 && code !== 9)
      break;
    end--;
  }
  return str.slice(start, end);
}
function decode(str) {
  if (str.indexOf("%") === -1)
    return str;
  try {
    return decodeURIComponent(str);
  } catch (e) {
    return str;
  }
}
function defaultEncode(str) {
  return cookieOctetRegExp.test(str) ? str : encodeURIComponent(str);
}

// .svelte-kit/output/server/index.js
var s = JSON.stringify;
async function render_endpoint(event, state, mod) {
  const method = event.request.method;
  let handler = mod[method] || mod.fallback;
  if (method === "HEAD" && !mod.HEAD && mod.GET)
    handler = mod.GET;
  if (!handler)
    return method_not_allowed(mod, method);
  const prerender = mod.prerender ?? state.prerender_default;
  if (prerender && BODY_DEPENDENT_METHODS.some((method2) => mod[method2]))
    throw new Error("Cannot prerender endpoints with body-dependent methods");
  if (state.prerendering && !state.prerendering.inside_reroute && !prerender) {
    if (state.depth > 0)
      throw new Error(`${event.route.id} is not prerenderable`);
    else
      return new Response(undefined, { status: 204 });
  }
  try {
    const response = await with_request_store({
      event,
      state
    }, () => handler(event));
    if (!(response instanceof Response))
      throw new Error(`Invalid response from route ${event.url.pathname}: handler should return a Response object`);
    if (state.prerendering && (!state.prerendering.inside_reroute || prerender)) {
      const cloned = new Response(response.clone().body, {
        status: response.status,
        statusText: response.statusText,
        headers: new Headers(response.headers)
      });
      cloned.headers.set("x-sveltekit-prerender", String(prerender));
      if (state.prerendering.inside_reroute && prerender) {
        cloned.headers.set("x-sveltekit-routeid", encodeURI(event.route.id));
        state.prerendering.dependencies.set(event.url.pathname, {
          response: cloned,
          body: null
        });
      } else
        return cloned;
    }
    return response;
  } catch (e) {
    if (e instanceof Redirect)
      return new Response(undefined, {
        status: e.status,
        headers: { location: e.location }
      });
    throw e;
  }
}
function is_endpoint_request(event) {
  const { method, headers } = event.request;
  if (ENDPOINT_METHODS.includes(method) && !PAGE_METHODS.includes(method))
    return true;
  if (method === "POST" && headers.get("x-sveltekit-action") === "true")
    return false;
  const accept = event.request.headers.get("accept") ?? "*/*";
  return negotiate2(accept, ["*", "text/html"]) !== "text/html";
}
function compact(arr) {
  return arr.filter((val) => val != null);
}
var ROUTES_PREFIX = "/routes";
function route_id_resolution_pathname(route_id) {
  return add_resolution_suffix(`/${app_dir}${ROUTES_PREFIX}${route_id === "/" ? "" : route_id}`);
}
function is_route_id_resolution_path(pathname) {
  const prefix = `/${app_dir}${ROUTES_PREFIX}`;
  return pathname === prefix || pathname.startsWith(prefix + "/");
}
function extract_route_id(pathname) {
  return pathname.slice(`/_app${ROUTES_PREFIX}`.length) || "/";
}
function build_error_chain(branch, errors, load) {
  const chain = [undefined];
  let last_idx = -1;
  for (let i = 1;i < branch.length; i += 1) {
    if (!branch[i])
      continue;
    let j = i - 1;
    while (j > last_idx + 1 && errors[j] == null)
      j -= 1;
    last_idx = j;
    const error2 = errors[j];
    chain.push(error2 == null ? undefined : load(error2)?.catch(() => {
      return;
    }));
  }
  return Promise.all(chain);
}
function* nearest_error_pages(i, branch, errors) {
  while (i--) {
    const error2 = errors[i];
    if (error2 != null) {
      let j = i;
      while (!branch[j])
        j -= 1;
      yield {
        error: error2,
        idx: j + 1
      };
    }
  }
}
function create_async_iterator() {
  let resolved = -1;
  const deferred = [];
  return {
    async* iterate(transform = (x) => x) {
      for (let i = 0;i < deferred.length; i += 1)
        yield transform(await deferred[i].promise);
    },
    add: (promise) => {
      const next = Promise.withResolvers();
      next.promise.catch(noop);
      deferred.push(next);
      promise.then((value) => {
        deferred[++resolved].resolve(value);
      }, (error2) => {
        deferred[++resolved].reject(error2);
      });
    }
  };
}
function server_data_serializer(event, state) {
  let promise_id = 1;
  let max_nodes = -1;
  const iterator = create_async_iterator();
  const global = "__sveltekit_1mr5upq";
  function get_replacer(index) {
    return function replacer(thing) {
      if (typeof thing?.then === "function") {
        const id = promise_id++;
        const promise = thing.then((data) => ({ data })).catch(async (error2) => ({ error: await handle_error_and_jsonify(event, state, error2) })).then(async ({ data, error: error2 }) => {
          let str;
          try {
            str = uneval(error2 ? [, error2] : [data], replacer);
          } catch (e) {
            error2 = await handle_error_and_jsonify(event, state, new Error(`Failed to serialize promise while rendering ${event.route.id}`, { cause: e }));
            str = uneval([, error2], replacer);
          }
          return {
            index,
            str: `${global}.resolve(${id}, ${str.includes("app.decode") ? `(app) => ${str}` : `() => ${str}`})`
          };
        });
        iterator.add(promise);
        return `${global}.defer(${id})`;
      } else
        for (const key in encoders) {
          const encoded = encoders[key](thing);
          if (encoded)
            return `app.decode('${key}', ${uneval(encoded, replacer)})`;
        }
    };
  }
  const strings = [];
  return {
    set_max_nodes(i) {
      max_nodes = i;
    },
    add_node(i, node) {
      try {
        if (!node) {
          strings[i] = "null";
          return;
        }
        const payload = {
          type: "data",
          data: node.data,
          uses: serialize_uses(node)
        };
        if (node.slash)
          payload.slash = node.slash;
        strings[i] = uneval(payload, get_replacer(i));
      } catch (e) {
        e.path = e.path.slice(1);
        throw new Error(clarify_devalue_error(event, e), { cause: e });
      }
    },
    get_data(csp) {
      const open = `<script${csp.script_needs_nonce ? ` nonce="${csp.nonce}"` : ""}>`;
      const close = `</script>
`;
      return {
        data: `[${compact(max_nodes > -1 ? strings.slice(0, max_nodes) : strings).join(",")}]`,
        chunks: promise_id > 1 ? iterator.iterate(({ index, str }) => {
          if (max_nodes > -1 && index >= max_nodes)
            return "";
          return open + str + close;
        }) : null
      };
    }
  };
}
function server_data_serializer_json(event, state) {
  let promise_id = 1;
  const iterator = create_async_iterator();
  const reducers = {
    ...encoders,
    Promise: (thing) => {
      if (typeof thing?.then !== "function")
        return;
      const id = promise_id++;
      let key = "data";
      const promise = thing.catch(async (e) => {
        key = "error";
        return handle_error_and_jsonify(event, state, e);
      }).then(async (value) => {
        let str;
        try {
          str = stringify(value, reducers);
        } catch (e) {
          const error2 = await handle_error_and_jsonify(event, state, new Error(`Failed to serialize promise while rendering ${event.route.id}`, { cause: e }));
          key = "error";
          str = stringify(error2, reducers);
        }
        return `{"type":"chunk","id":${id},"${key}":${str}}
`;
      });
      iterator.add(promise);
      return id;
    }
  };
  const strings = [];
  return {
    add_node(i, node) {
      try {
        if (!node) {
          strings[i] = "null";
          return;
        }
        if (node.type === "error" || node.type === "skip") {
          strings[i] = JSON.stringify(node);
          return;
        }
        strings[i] = `{"type":"data","data":${stringify(node.data, reducers)},"uses":${JSON.stringify(serialize_uses(node))}${node.slash ? `,"slash":${JSON.stringify(node.slash)}` : ""}}`;
      } catch (e) {
        e.path = "data" + e.path;
        throw new Error(clarify_devalue_error(event, e), { cause: e });
      }
    },
    get_data() {
      return {
        data: `{"type":"data","nodes":[${strings.join(",")}]}
`,
        chunks: promise_id > 1 ? iterator.iterate() : null
      };
    }
  };
}
var NULL_BODY_STATUS = [
  101,
  103,
  204,
  205,
  304
];
async function load_server_data({ event, state, node, parent }) {
  if (!node?.server)
    return null;
  let is_tracking = true;
  const uses = {
    dependencies: /* @__PURE__ */ new Set,
    params: /* @__PURE__ */ new Set,
    parent: false,
    route: false,
    url: false,
    search_params: /* @__PURE__ */ new Set
  };
  const load = node.server.load;
  const slash = node.server.trailingSlash;
  if (!load)
    return {
      type: "data",
      data: null,
      uses,
      slash
    };
  const url2 = make_trackable(event.url, () => {
    if (is_tracking)
      uses.url = true;
  }, (param) => {
    if (is_tracking)
      uses.search_params.add(param);
  });
  if (state.prerendering || state.prerender_default === true)
    disable_search(url2);
  return {
    type: "data",
    data: await record_span({
      name: "sveltekit.load",
      attributes: {
        "sveltekit.load.node_id": node.server_id || "unknown",
        "sveltekit.load.node_type": get_node_type(node.server_id),
        "sveltekit.load.environment": "server",
        "http.route": event.route.id || "unknown"
      },
      fn: async (current) => {
        const traced_event = merge_tracing(event, current);
        return await with_request_store({
          event: traced_event,
          state
        }, () => load.call(null, {
          ...traced_event,
          fetch: (info, init) => {
            new URL(info instanceof Request ? info.url : info, event.url);
            return event.fetch(info, init);
          },
          depends: (...deps) => {
            for (const dep of deps) {
              const { href } = new URL(dep, event.url);
              uses.dependencies.add(href);
            }
          },
          params: new Proxy(event.params, { get: (target, key) => {
            if (is_tracking)
              uses.params.add(key);
            return target[key];
          } }),
          parent: async () => {
            if (is_tracking)
              uses.parent = true;
            return parent();
          },
          route: new Proxy(event.route, { get: (target, key) => {
            if (is_tracking)
              uses.route = true;
            return target[key];
          } }),
          url: url2,
          untrack(fn) {
            is_tracking = false;
            try {
              return fn();
            } finally {
              is_tracking = true;
            }
          }
        }));
      }
    }) ?? null,
    uses,
    slash
  };
}
async function load_data({ event, state, fetched, node, parent, server_data_promise, resolve_opts, csr }) {
  const server_data_node = await server_data_promise;
  const load = node?.universal?.load;
  if (!load)
    return server_data_node?.data ?? null;
  return await record_span({
    name: "sveltekit.load",
    attributes: {
      "sveltekit.load.node_id": node.universal_id || "unknown",
      "sveltekit.load.node_type": get_node_type(node.universal_id),
      "sveltekit.load.environment": "server",
      "http.route": event.route.id || "unknown"
    },
    fn: async (current) => {
      const traced_event = merge_tracing(event, current);
      return await with_request_store({
        event: traced_event,
        state
      }, () => load.call(null, {
        url: event.url,
        params: event.params,
        data: server_data_node?.data ?? null,
        route: event.route,
        fetch: create_universal_fetch(event, state.prerendering, fetched, csr, resolve_opts),
        setHeaders: event.setHeaders,
        depends: noop,
        parent,
        untrack: (fn) => fn(),
        tracing: traced_event.tracing
      }));
    }
  }) ?? null;
}
function create_universal_fetch(event, prerendering, fetched, csr, resolve_opts) {
  const universal_fetch = async (input, init) => {
    const cloned_body = input instanceof Request && input.body ? input.clone().body : null;
    const cloned_headers = input instanceof Request && [...input.headers].length ? new Headers(input.headers) : init?.headers;
    let response = await event.fetch(input, init);
    const url2 = new URL(input instanceof Request ? input.url : input, event.url);
    const same_origin = url2.origin === event.url.origin;
    let dependency;
    if (same_origin) {
      if (prerendering) {
        dependency = {
          response,
          body: null
        };
        prerendering.dependencies.set(url2.pathname, dependency);
      }
    } else if (url2.protocol === "https:" || url2.protocol === "http:") {
      if ((input instanceof Request ? input.mode : init?.mode ?? "cors") === "no-cors")
        response = new Response("", {
          status: response.status,
          statusText: response.statusText,
          headers: response.headers
        });
      else {
        const acao = response.headers.get("access-control-allow-origin");
        if (!acao || acao !== event.url.origin && acao !== "*")
          throw new Error(`CORS error: ${acao ? "Incorrect" : "No"} 'Access-Control-Allow-Origin' header is present on the requested resource`);
      }
    }
    let teed_body;
    const proxy = new Proxy(response, { get(response2, key, receiver) {
      async function push_fetched(body, is_b64) {
        const status_number = Number(response2.status);
        if (isNaN(status_number))
          throw new Error(`response.status is not a number. value: "${response2.status}" type: ${typeof response2.status}`);
        const request_body = input instanceof Request && cloned_body ? await new Response(cloned_body).text() : init?.body;
        if (request_body && typeof request_body !== "string" && !ArrayBuffer.isView(request_body))
          return;
        fetched.push({
          url: same_origin ? url2.href.slice(event.url.origin.length) : url2.href,
          method: event.request.method,
          request_body,
          request_headers: cloned_headers,
          response_body: body,
          response: response2,
          is_b64
        });
      }
      if (key === "body") {
        if (response2.body === null)
          return null;
        if (teed_body)
          return teed_body;
        const [a, b] = response2.body.tee();
        (async () => {
          const result = new Uint8Array(await new Response(a).arrayBuffer());
          if (dependency)
            dependency.body = new Uint8Array(result);
          push_fetched(base64_encode(result), true);
        })().catch(noop);
        return teed_body = b;
      }
      if (key === "arrayBuffer")
        return async () => {
          const buffer = await response2.arrayBuffer();
          const bytes = new Uint8Array(buffer);
          if (dependency)
            dependency.body = bytes;
          if (buffer instanceof ArrayBuffer)
            await push_fetched(base64_encode(bytes), true);
          return buffer;
        };
      async function text2() {
        const body = await response2.text();
        if (body === "" && NULL_BODY_STATUS.includes(response2.status)) {
          await push_fetched(undefined, false);
          return;
        }
        if (!body || typeof body === "string")
          await push_fetched(body, false);
        if (dependency)
          dependency.body = body;
        return body;
      }
      if (key === "text")
        return text2;
      if (key === "json")
        return async () => {
          const body = await text2();
          return body ? JSON.parse(body) : undefined;
        };
      const value = Reflect.get(response2, key, response2);
      if (value instanceof Function)
        return Object.defineProperties(function() {
          return Reflect.apply(value, this === receiver ? response2 : this, arguments);
        }, {
          name: { value: value.name },
          length: { value: value.length }
        });
      return value;
    } });
    if (csr) {
      const get = response.headers.get;
      response.headers.get = (key) => {
        const lower = key.toLowerCase();
        const value = get.call(response.headers, lower);
        if (value && !lower.startsWith("x-sveltekit-")) {
          if (!resolve_opts.filterSerializedResponseHeaders(lower, value))
            throw new Error(`Failed to get response header "${lower}" \u2014 it must be included by the \`filterSerializedResponseHeaders\` option: https://svelte.dev/docs/kit/hooks#handle (at ${event.route.id})`);
        }
        return value;
      };
      const get_set_cookie = response.headers.getSetCookie;
      response.headers.getSetCookie = () => {
        const values = get_set_cookie.call(response.headers);
        for (const value of values)
          if (!resolve_opts.filterSerializedResponseHeaders("set-cookie", value))
            throw new Error(`Failed to get response header "set-cookie" \u2014 it must be included by the \`filterSerializedResponseHeaders\` option: https://svelte.dev/docs/kit/hooks#handle (at ${event.route.id})`);
        return values;
      };
    }
    return proxy;
  };
  return (input, init) => {
    const response = universal_fetch(input, init);
    response.catch(noop);
    return response;
  };
}
function hash(...values) {
  let hash2 = 5381;
  for (const value of values)
    if (typeof value === "string") {
      let i = value.length;
      while (i)
        hash2 = hash2 * 33 ^ value.charCodeAt(--i);
    } else if (ArrayBuffer.isView(value)) {
      const buffer = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
      let i = buffer.length;
      while (i)
        hash2 = hash2 * 33 ^ buffer[--i];
    } else
      throw new TypeError("value must be a string or TypedArray");
  return (hash2 >>> 0).toString(36);
}
function hash_request(headers, body) {
  const values = [];
  if (headers)
    values.push([...new Headers(headers)].join(","));
  if (body)
    values.push(body);
  return hash(...values);
}
var replacements = {
  "<": "\\u003C",
  "\u2028": "\\u2028",
  "\u2029": "\\u2029"
};
var pattern = new RegExp(`[${Object.keys(replacements).join("")}]`, "g");
function serialize_data(fetched, filter, prerendering = false) {
  const headers = {};
  let cache_control = null;
  let age = null;
  let varyAny = false;
  for (const [key, value] of fetched.response.headers) {
    if (filter(key, value))
      headers[key] = value;
    if (key === "cache-control")
      cache_control = value;
    else if (key === "age")
      age = value;
    else if (key === "vary" && value.trim() === "*")
      varyAny = true;
  }
  const payload = {
    status: fetched.response.status,
    statusText: fetched.response.statusText,
    headers,
    body: fetched.response_body
  };
  const safe_payload = JSON.stringify(payload).replace(pattern, (match) => replacements[match]);
  const attrs = [
    'type="application/json"',
    "data-sveltekit-fetched",
    `data-url="${escape_html2(fetched.url, true)}"`
  ];
  if (fetched.is_b64)
    attrs.push("data-b64");
  if (fetched.request_headers || fetched.request_body)
    attrs.push(`data-hash="${hash_request(fetched.request_headers, fetched.request_body)}"`);
  if (!prerendering && fetched.method === "GET" && cache_control && !varyAny) {
    const match = /s-maxage=(\d+)/g.exec(cache_control) ?? /max-age=(\d+)/g.exec(cache_control);
    if (match) {
      const ttl = +match[1] - +(age ?? "0");
      attrs.push(`data-ttl="${ttl}"`);
    }
  }
  return `<script ${attrs.join(" ")}>${safe_payload}</script>`;
}
function sha256(data) {
  if (!key[0])
    precompute();
  const out = init.slice(0);
  const array = encode(data);
  for (let i = 0;i < array.length; i += 16) {
    const w = array.subarray(i, i + 16);
    let tmp;
    let a;
    let b;
    let out0 = out[0];
    let out1 = out[1];
    let out2 = out[2];
    let out3 = out[3];
    let out4 = out[4];
    let out5 = out[5];
    let out6 = out[6];
    let out7 = out[7];
    for (let i2 = 0;i2 < 64; i2++) {
      if (i2 < 16)
        tmp = w[i2];
      else {
        a = w[i2 + 1 & 15];
        b = w[i2 + 14 & 15];
        tmp = w[i2 & 15] = (a >>> 7 ^ a >>> 18 ^ a >>> 3 ^ a << 25 ^ a << 14) + (b >>> 17 ^ b >>> 19 ^ b >>> 10 ^ b << 15 ^ b << 13) + w[i2 & 15] + w[i2 + 9 & 15] | 0;
      }
      tmp = tmp + out7 + (out4 >>> 6 ^ out4 >>> 11 ^ out4 >>> 25 ^ out4 << 26 ^ out4 << 21 ^ out4 << 7) + (out6 ^ out4 & (out5 ^ out6)) + key[i2];
      out7 = out6;
      out6 = out5;
      out5 = out4;
      out4 = out3 + tmp | 0;
      out3 = out2;
      out2 = out1;
      out1 = out0;
      out0 = tmp + (out1 & out2 ^ out3 & (out1 ^ out2)) + (out1 >>> 2 ^ out1 >>> 13 ^ out1 >>> 22 ^ out1 << 30 ^ out1 << 19 ^ out1 << 10) | 0;
    }
    out[0] = out[0] + out0 | 0;
    out[1] = out[1] + out1 | 0;
    out[2] = out[2] + out2 | 0;
    out[3] = out[3] + out3 | 0;
    out[4] = out[4] + out4 | 0;
    out[5] = out[5] + out5 | 0;
    out[6] = out[6] + out6 | 0;
    out[7] = out[7] + out7 | 0;
  }
  const bytes = new Uint8Array(out.buffer);
  reverse_endianness(bytes);
  return base64_encode(bytes);
}
var init = /* @__PURE__ */ new Uint32Array(8);
var key = /* @__PURE__ */ new Uint32Array(64);
function precompute() {
  function frac(x) {
    return (x - Math.floor(x)) * 4294967296;
  }
  let prime = 2;
  for (let i = 0;i < 64; prime++) {
    let is_prime = true;
    for (let factor = 2;factor * factor <= prime; factor++)
      if (prime % factor === 0) {
        is_prime = false;
        break;
      }
    if (is_prime) {
      if (i < 8)
        init[i] = frac(prime ** (1 / 2));
      key[i] = frac(prime ** (1 / 3));
      i++;
    }
  }
}
function reverse_endianness(bytes) {
  for (let i = 0;i < bytes.length; i += 4) {
    const a = bytes[i + 0];
    const b = bytes[i + 1];
    const c = bytes[i + 2];
    const d = bytes[i + 3];
    bytes[i + 0] = d;
    bytes[i + 1] = c;
    bytes[i + 2] = b;
    bytes[i + 3] = a;
  }
}
function encode(str) {
  const encoded = text_encoder.encode(str);
  const length = encoded.length * 8;
  const size = 512 * Math.ceil((length + 65) / 512);
  const bytes = new Uint8Array(size / 8);
  bytes.set(encoded);
  bytes[encoded.length] = 128;
  reverse_endianness(bytes);
  const words = new Uint32Array(bytes.buffer);
  words[words.length - 2] = Math.floor(length / 4294967296);
  words[words.length - 1] = length;
  return words;
}
var array = /* @__PURE__ */ new Uint8Array(16);
function generate_nonce() {
  crypto.getRandomValues(array);
  return base64_encode(array);
}
var quoted = /* @__PURE__ */ new Set([
  "self",
  "unsafe-eval",
  "unsafe-hashes",
  "unsafe-inline",
  "none",
  "strict-dynamic",
  "report-sample",
  "wasm-unsafe-eval",
  "script"
]);
var crypto_pattern = /^(nonce|sha\d\d\d)-/;
var BaseProvider = class {
  #use_hashes;
  #script_needs_csp;
  #script_src_needs_csp;
  #script_src_elem_needs_csp;
  #style_needs_csp;
  #style_src_needs_csp;
  #style_src_attr_needs_csp;
  #style_src_elem_needs_csp;
  #directives;
  #script_src = /* @__PURE__ */ new Set;
  #script_src_elem = /* @__PURE__ */ new Set;
  #style_src = /* @__PURE__ */ new Set;
  #style_src_attr = /* @__PURE__ */ new Set;
  #style_src_elem = /* @__PURE__ */ new Set;
  script_needs_nonce;
  style_needs_nonce;
  script_needs_hash;
  #nonce;
  constructor(use_hashes, directives, nonce) {
    this.#use_hashes = use_hashes;
    this.#directives = directives;
    const d = this.#directives;
    const effective_script_src = d["script-src"] || d["default-src"];
    const script_src_elem = d["script-src-elem"];
    const effective_style_src = d["style-src"] || d["default-src"];
    const style_src_attr = d["style-src-attr"];
    const style_src_elem = d["style-src-elem"];
    const style_needs_csp = (directive) => !!directive && !directive.some((value) => value === "unsafe-inline");
    const script_needs_csp = (directive) => !!directive && (!directive.some((value) => value === "unsafe-inline") || directive.some((value) => value === "strict-dynamic"));
    this.#script_src_needs_csp = script_needs_csp(effective_script_src);
    this.#script_src_elem_needs_csp = script_needs_csp(script_src_elem);
    this.#style_src_needs_csp = style_needs_csp(effective_style_src);
    this.#style_src_attr_needs_csp = style_needs_csp(style_src_attr);
    this.#style_src_elem_needs_csp = style_needs_csp(style_src_elem);
    this.#script_needs_csp = this.#script_src_needs_csp || this.#script_src_elem_needs_csp;
    this.#style_needs_csp = this.#style_src_needs_csp || this.#style_src_attr_needs_csp || this.#style_src_elem_needs_csp;
    this.script_needs_nonce = this.#script_needs_csp && !this.#use_hashes;
    this.style_needs_nonce = this.#style_needs_csp && !this.#use_hashes;
    this.script_needs_hash = this.#script_needs_csp && this.#use_hashes;
    this.#nonce = nonce;
  }
  #get_source(content) {
    return this.#use_hashes ? `sha256-${sha256(content)}` : `nonce-${this.#nonce}`;
  }
  #add_script_source(source) {
    if (this.#script_src_needs_csp)
      this.#script_src.add(source);
    if (this.#script_src_elem_needs_csp)
      this.#script_src_elem.add(source);
  }
  add_script(content) {
    if (!this.#script_needs_csp)
      return;
    this.#add_script_source(this.#get_source(content));
  }
  add_script_hashes(hashes) {
    for (const hash2 of hashes)
      this.#add_script_source(hash2);
  }
  add_style(content) {
    if (!this.#style_needs_csp)
      return;
    const source = this.#get_source(content);
    if (this.#style_src_needs_csp)
      this.#style_src.add(source);
    if (this.#style_src_attr_needs_csp)
      this.#style_src_attr.add(source);
    if (this.#style_src_elem_needs_csp) {
      const sha256_empty_comment_hash = "sha256-9OlNO0DNEeaVzHL4RZwCLsBHA8WBQ8toBp/4F5XV2nc=";
      const d = this.#directives;
      if (d["style-src-elem"] && !d["style-src-elem"].includes(sha256_empty_comment_hash) && !this.#style_src_elem.has(sha256_empty_comment_hash))
        this.#style_src_elem.add(sha256_empty_comment_hash);
      if (source !== sha256_empty_comment_hash)
        this.#style_src_elem.add(source);
    }
  }
  get_header(is_meta = false) {
    const header = [];
    const directives = { ...this.#directives };
    const merge_sources = (key2, sources, base2) => {
      if (sources.size > 0)
        directives[key2] = [...base2 || [], ...sources];
    };
    merge_sources("style-src", this.#style_src, directives["style-src"] || directives["default-src"]);
    merge_sources("style-src-attr", this.#style_src_attr, directives["style-src-attr"]);
    merge_sources("style-src-elem", this.#style_src_elem, directives["style-src-elem"]);
    merge_sources("script-src", this.#script_src, directives["script-src"] || directives["default-src"]);
    merge_sources("script-src-elem", this.#script_src_elem, directives["script-src-elem"]);
    for (const key2 in directives) {
      if (is_meta && (key2 === "frame-ancestors" || key2 === "report-uri" || key2 === "sandbox"))
        continue;
      const value = directives[key2];
      if (!value)
        continue;
      const directive = [key2];
      if (Array.isArray(value))
        for (const source of value)
          directive.push(quoted.has(source) || crypto_pattern.test(source) ? `'${source}'` : source);
      header.push(directive.join(" "));
    }
    return header.join("; ");
  }
};
var CspProvider = class extends BaseProvider {
  get_meta() {
    const content = this.get_header(true);
    if (!content)
      return;
    return `<meta http-equiv="content-security-policy" content="${escape_html2(content, true)}">`;
  }
};
var CspReportOnlyProvider = class extends BaseProvider {
  constructor(use_hashes, directives, nonce) {
    super(use_hashes, directives, nonce);
    if (Object.values(directives).some((v) => !!v) && !directives["report-to"]?.length && !directives["report-uri"]?.length)
      throw Error("`content-security-policy-report-only` must be specified with either the `report-to` or `report-uri` directives, or both");
  }
};
var Csp = class {
  nonce = generate_nonce();
  csp_provider;
  report_only_provider;
  constructor({ mode, directives, reportOnly }, { prerender }) {
    const use_hashes = mode === "hash" || mode === "auto" && prerender;
    this.csp_provider = new CspProvider(use_hashes, directives, this.nonce);
    this.report_only_provider = new CspReportOnlyProvider(use_hashes, reportOnly, this.nonce);
  }
  get script_needs_hash() {
    return this.csp_provider.script_needs_hash || this.report_only_provider.script_needs_hash;
  }
  get script_needs_nonce() {
    return this.csp_provider.script_needs_nonce || this.report_only_provider.script_needs_nonce;
  }
  get style_needs_nonce() {
    return this.csp_provider.style_needs_nonce || this.report_only_provider.style_needs_nonce;
  }
  add_script(content) {
    this.csp_provider.add_script(content);
    this.report_only_provider.add_script(content);
  }
  add_script_hashes(hashes) {
    this.csp_provider.add_script_hashes(hashes);
    this.report_only_provider.add_script_hashes(hashes);
  }
  add_style(content) {
    this.csp_provider.add_style(content);
    this.report_only_provider.add_style(content);
  }
};
function generate_route_object(route, url2, client) {
  const { errors, layouts, leaf } = route;
  const nodes = [
    ...errors,
    ...layouts.map((l) => l?.[1]),
    leaf[1]
  ].filter((n) => typeof n === "number").map((n) => `'${n}': () => ${create_client_import(client.nodes?.[n], url2)}`).join(`,
		`);
  return [
    `{
	id: ${s(route.id)}`,
    `errors: ${s(route.errors)}`,
    `layouts: ${s(route.layouts)}`,
    `leaf: ${s(route.leaf)}`,
    `nodes: {
		${nodes}
	}
}`
  ].join(`,
	`);
}
function create_client_import(import_path, url2) {
  if (!import_path)
    return "Promise.resolve({})";
  if (import_path[0] === "/")
    return `import('${import_path}')`;
  if (assets !== "")
    return `import('${assets}/${import_path}')`;
  let path3 = get_relative_path(url2.pathname, `/${import_path}`);
  if (path3[0] !== ".")
    path3 = `./${path3}`;
  return `import('${path3}')`;
}
async function resolve_route(resolved_path, url2, manifest2) {
  if (!manifest2._.client?.routes)
    return text("Server-side route resolution disabled", { status: 400 });
  try {
    const matchers = await manifest2._.matchers();
    const result = find_route(resolved_path, manifest2._.client.routes, matchers);
    return create_server_routing_response(result?.route ?? null, result?.params ?? {}, url2, manifest2._.client).response;
  } catch {
    return text("Error resolving route", { status: 500 });
  }
}
function resolve_route_by_id(route_id, url2, manifest2) {
  if (!manifest2._.client?.routes)
    return text("Server-side route resolution disabled", { status: 400 });
  try {
    const route = manifest2._.client.routes.find((r) => r.id === route_id);
    if (route)
      return create_server_routing_response(route, null, url2, manifest2._.client).response;
    if (manifest2._.routes.some((r) => r.id === route_id && !r.page))
      return text("export const endpoint_only = true;", { headers: js_headers() });
    return create_server_routing_response(null, null, url2, manifest2._.client).response;
  } catch {
    return text("Error resolving route", { status: 500 });
  }
}
function js_headers() {
  return new Headers({ "content-type": "application/javascript; charset=utf-8" });
}
function create_server_routing_response(route, params, url2, client) {
  const headers = js_headers();
  let body = "";
  if (route) {
    const csr_route = generate_route_object(route, url2, client);
    body = `${create_css_import(route, url2, client)}export const route = ${csr_route};`;
    if (params !== null)
      body += `
export const params = ${JSON.stringify(params)}`;
  }
  return {
    response: text(body, { headers }),
    body
  };
}
function create_css_import(route, url2, client) {
  const { errors, layouts, leaf } = route;
  let css = "";
  for (const node of [
    ...errors,
    ...layouts.map((l) => l?.[1]),
    leaf[1]
  ]) {
    if (typeof node !== "number")
      continue;
    const node_css = client.css?.[node];
    for (const css_path of node_css ?? [])
      css += `'${assets || ""}/${css_path}',`;
  }
  if (!css)
    return "";
  return `${create_client_import(client.start, url2)}.then(x => x.load_css([${css}]));
`;
}
function Root($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const { page, components, onerror, tree, form, error: error2 } = $$props;
    let mounted = false;
    let navigated = false;
    let title = "";
    afterNavigate(() => {
      if (mounted) {
        navigated = true;
        title = document.title || "untitled page";
      } else
        mounted = true;
    });
    function node($$renderer3, n, depth) {
      const Component = derived(() => n.component);
      const Error2 = derived(() => n.error);
      const data = derived(() => n.data);
      function failed($$renderer4, error3) {
        if (Error2()) {
          $$renderer4.push("<!--[-->");
          Error2()($$renderer4, { error: error3 });
          $$renderer4.push("<!--]-->");
        } else {
          $$renderer4.push("<!--[!-->");
          $$renderer4.push("<!--]-->");
        }
      }
      $$renderer3.boundary({ failed: Error2() ? failed : undefined }, ($$renderer4) => {
        $$renderer4.push(`<!--[-->`);
        if (n.child) {
          $$renderer4.push("<!--[0-->");
          if (Component()) {
            $$renderer4.push("<!--[-->");
            Component()($$renderer4, {
              data: data(),
              form,
              params: page.params,
              children: ($$renderer5) => {
                node($$renderer5, n.child, depth + 1);
              },
              $$slots: { default: true }
            });
            $$renderer4.push("<!--]-->");
          } else {
            $$renderer4.push("<!--[!-->");
            $$renderer4.push("<!--]-->");
          }
        } else {
          $$renderer4.push("<!--[-1-->");
          if (Component()) {
            $$renderer4.push("<!--[-->");
            Component()($$renderer4, {
              data: data(),
              form,
              params: page.params,
              error: error2
            });
            $$renderer4.push("<!--]-->");
          } else {
            $$renderer4.push("<!--[!-->");
            $$renderer4.push("<!--]-->");
          }
        }
        $$renderer4.push(`<!--]-->`);
        $$renderer4.push(`<!--]-->`);
      });
    }
    node($$renderer2, tree, 0);
    $$renderer2.push(`<!----> `);
    if (mounted) {
      $$renderer2.push(`<!--[0--><div id="svelte-announcer" aria-live="assertive" aria-atomic="true" style="position: absolute; left: 0; top: 0; clip: rect(0 0 0 0); clip-path: inset(50%); overflow: hidden; white-space: nowrap; width: 1px; height: 1px">`);
      if (navigated)
        $$renderer2.push(`<!--[0-->${escape_html(title)}`);
      else
        $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<!--]--></div>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]-->`);
  });
}
var Props = class {
  page;
  components = [];
  form;
  error;
  tree;
  onerror;
  constructor({ page, tree, form, error: error2, onerror = noop }) {
    this.page = page;
    this.tree = tree;
    this.onerror = onerror;
    this.form = form;
    this.error = error2;
  }
};
var RenderNode = class {
  component;
  error;
  data = {};
  child;
  constructor(component, error2) {
    this.component = component;
    this.error = error2;
  }
};
async function render_response({ branch, fetched, manifest: manifest2, page_config, status, error: error2 = null, event, state, resolve_opts, action_result, data_serializer, error_components }) {
  if (state.prerendering || state.prerender_default === true) {
    if (options$1.csp.mode === "nonce")
      throw new Error('Cannot use prerendering if config.csp.mode === "nonce"');
    if (options$1.app_template_contains_nonce)
      throw new Error("Cannot use prerendering if page template contains %sveltekit.nonce%");
  }
  const { client } = manifest2._;
  const modulepreloads = new Set(client?.imports);
  const stylesheets = new Set(client?.stylesheets);
  const fonts = new Map(client?.fonts.map((font) => [font.file, font]));
  const inline_styles = /* @__PURE__ */ new Map;
  let rendered;
  const form_value = action_result?.type === "success" || action_result?.type === "failure" ? action_result.data ?? null : null;
  let base2 = "";
  let assets$1 = assets;
  let base_expression = s("");
  const csp = new Csp(options$1.csp, { prerender: !!(state.prerendering || state.prerender_default === true) });
  if (!state.prerendering?.fallback) {
    base2 = (event.isDataRequest ? add_data_suffix(event.url.pathname) : event.url.pathname).slice(0).split("/").slice(2).map(() => "..").join("/") || ".";
    base_expression = `new URL(${s(base2)}, location).pathname.slice(0, -1)`;
    if (!assets || assets[0] === "/" && assets !== "/_svelte_kit_assets")
      assets$1 = base2;
  }
  if (page_config.ssr) {
    const props = new Props({
      page: {
        error: error2,
        params: event.params,
        route: event.route,
        status,
        url: event.url,
        data: {},
        form: form_value,
        shallow: null,
        state: {}
      },
      tree: new RenderNode(await branch[0].node.component?.(), undefined),
      form: form_value,
      error: error2 ?? undefined
    });
    let current_node = props.tree;
    let data2 = props.page.data;
    for (let i = 0;i < branch.length; i += 1) {
      const node = branch[i];
      data2 = {
        ...data2,
        ...node.data
      };
      current_node.data = data2;
      if (i < branch.length - 1)
        current_node = current_node.child = new RenderNode(await branch[i + 1].node.component?.(), error_components?.[i + 1]);
    }
    props.page.data = data2;
    const render_state = {
      ...state,
      is_in_render: true
    };
    const render_opts = {
      context: /* @__PURE__ */ new Map([["__request__", { page: props.page }]]),
      csp: csp.script_needs_nonce ? { nonce: csp.nonce } : { hash: csp.script_needs_hash },
      transformError: error_components ? (e) => {
        if (isRedirect(e))
          throw e;
        const handled = handle_error_and_jsonify(event, render_state, e);
        if (handled instanceof Promise)
          return handled.then((e2) => {
            error2 = e2;
            props.page.error = error2;
            props.page.status = status = error2.status;
            return error2;
          });
        error2 = handled;
        props.page.error = error2;
        props.page.status = status = error2.status;
        return error2;
      } : undefined
    };
    globalThis.fetch;
    try {
      rendered = await with_request_store({
        event,
        state: render_state
      }, async () => {
        return render(Root, {
          ...render_opts,
          props
        });
      });
      if (rendered.hashes)
        csp.add_script_hashes(rendered.hashes.script);
    } finally {}
  } else
    rendered = {
      head: "",
      body: "",
      hashes: { script: [] }
    };
  for (const { node } of branch) {
    for (const url2 of node.imports)
      modulepreloads.add(url2);
    for (const url2 of node.stylesheets)
      stylesheets.add(url2);
    for (const font of node.fonts)
      fonts.set(font.file, font);
    if (node.inline_styles && !client?.inline)
      Object.entries(await node.inline_styles()).forEach(([filename, css]) => {
        if (typeof css === "string") {
          inline_styles.set(filename, css);
          return;
        }
        inline_styles.set(filename, css(`${assets$1}/${app_dir}/immutable/assets`, assets$1));
      });
  }
  const head = new Head(rendered.head);
  let body = rendered.body;
  const prefixed = (path3) => {
    if (path3.startsWith("/"))
      return "" + path3;
    return `${assets$1}/${path3}`;
  };
  const style = client?.inline ? client.inline?.style : Array.from(inline_styles.values()).join(`
`);
  if (style) {
    const attributes = [];
    if (csp.style_needs_nonce)
      attributes.push(`nonce="${csp.nonce}"`);
    csp.add_style(style);
    head.add_style(style, attributes);
  }
  const add_preload = (path3, attributes) => {
    head.add_link_tag(path3, attributes);
  };
  for (const dep of stylesheets) {
    const path3 = prefixed(dep);
    const attributes = ['rel="stylesheet"'];
    if (inline_styles.has(dep))
      attributes.push("disabled", 'media="(max-width: 0)"');
    head.add_stylesheet(path3, attributes);
  }
  for (const { file, filename } of fonts.values()) {
    const path3 = prefixed(file);
    if (resolve_opts.preload({
      type: "font",
      path: path3,
      filename
    }))
      add_preload(path3, [
        'rel="preload"',
        'as="font"',
        `type="font/${file.slice(file.lastIndexOf(".") + 1)}"`,
        "crossorigin"
      ]);
  }
  const global = "__sveltekit_1mr5upq";
  const { data, chunks } = data_serializer.get_data(csp);
  if (page_config.ssr && page_config.csr)
    body += `
			${fetched.map((item) => serialize_data(item, resolve_opts.filterSerializedResponseHeaders, !!(state.prerendering || state.prerender_default === true))).join(`
			`)}`;
  if (page_config.csr && client) {
    const route = client.routes?.find((r) => r.id === event.route.id) ?? null;
    const load_env_eagerly = client.uses_env_dynamic_public && (state.prerendering || state.prerender_default === true);
    if (load_env_eagerly)
      modulepreloads.add(`${app_dir}/env.js`);
    if (!client.inline)
      for (const dep of modulepreloads) {
        const path3 = prefixed(dep);
        if (resolve_opts.preload({
          type: "js",
          path: path3
        }))
          add_preload(path3, ['rel="modulepreload"']);
      }
    if (client.routes && state.prerendering && !state.prerendering.fallback) {
      const pathname = add_resolution_suffix(event.url.pathname);
      state.prerendering.dependencies.set(pathname, create_server_routing_response(route, event.params, new URL(pathname, event.url), client));
      if (route && !state.prerendering.resolved_route_ids.has(route.id)) {
        state.prerendering.resolved_route_ids.add(route.id);
        const id_pathname = "" + route_id_resolution_pathname(route.id);
        state.prerendering.dependencies.set(id_pathname, create_server_routing_response(route, null, new URL(id_pathname, event.url), client));
      }
    }
    const blocks = [];
    const properties = [`base: ${base_expression}`, `version: ${s("1788652438838")}`];
    if (assets)
      properties.push(`assets: ${s(assets)}`);
    if (client.uses_env_dynamic_public)
      properties.push(`env: ${load_env_eagerly ? "null" : uneval(rendered_env)}`);
    if (chunks) {
      blocks.push("const deferred = new Map();");
      properties.push(`defer: (id) => new Promise((fulfil, reject) => {
							deferred.set(id, { fulfil, reject });
						})`);
      let app_declaration = "";
      if (has_custom_transporters) {
        if (client.inline)
          app_declaration = `const app = ${global}.app.app;`;
        else if (client.app)
          app_declaration = `const kit = await import(${s(prefixed(client.start))});
							kit.init(${global});
							const app = await import(${s(prefixed(client.app))});`;
        else
          app_declaration = `const { app } = await import(${s(prefixed(client.start))});`;
      }
      const prelude = app_declaration ? `${app_declaration}
							const [data, error] = fn(app);` : `const [data, error] = fn();`;
      properties.push(`resolve: async (id, fn) => {
							${prelude}

							const try_to_resolve = () => {
								if (!deferred.has(id)) {
									setTimeout(try_to_resolve, 0);
									return;
								}
								const { fulfil, reject } = deferred.get(id);
								deferred.delete(id);
								if (error) reject(error);
								else fulfil(data);
							}
							try_to_resolve();
						}`);
    }
    blocks.push(`${global} = {
						${properties.join(`,
						`)}
					};`);
    const args = ["element"];
    blocks.push("const element = document.currentScript.parentElement;");
    if (page_config.ssr) {
      const serialized = {
        form: "null",
        error: "null"
      };
      if (form_value)
        serialized.form = uneval_action_response(form_value, event.route.id);
      if (error2)
        serialized.error = uneval(error2);
      const hydrate = [
        `node_ids: [${branch.map(({ node }) => node.index).join(", ")}]`,
        `data: ${data}`,
        `form: ${serialized.form}`,
        `error: ${serialized.error}`
      ];
      if (status !== 200 && !error2)
        hydrate.push(`status: ${status}`);
      if (client.routes) {
        if (route) {
          const stringified = generate_route_object(route, event.url, client).replaceAll(`
`, `
							`);
          hydrate.push(`params: ${uneval(event.params)}`, `server_route: ${stringified}`);
        }
      }
      const indent = "\t".repeat(load_env_eagerly ? 7 : 6);
      args.push(`{
${indent}	${hydrate.join(`,
${indent}	`)}
${indent}}`);
    }
    const remote_data = await collect_remote_data({}, event, state);
    const serialized_data = Object.keys(remote_data).length > 0 ? `${global}.data = ${uneval2(remote_data)};

						` : "";
    const boot = client.inline ? `${client.inline.script}

					${serialized_data}${global}.app.start(${args.join(", ")});` : client.app ? `import(${s(prefixed(client.start))}).then(async (kit) => {
						kit.init(${global});
						const app = await import(${s(prefixed(client.app))});
						${serialized_data}kit.start(app, ${args.join(", ")});
					});` : `import(${s(prefixed(client.start))}).then((app) => {
						${serialized_data}app.start(${args.join(", ")})
					});`;
    if (load_env_eagerly)
      blocks.push(`import(${s(`${base2}/${app_dir}/env.js`)}).then(({ env }) => {
						${global}.env = env;

						${boot.replace(/\n/g, `
	`)}
					});`);
    else
      blocks.push(boot);
    const init_app = `
				{
					${blocks.join(`

					`)}
				}
			`;
    csp.add_script(init_app);
    body += `
			<script${csp.script_needs_nonce ? ` nonce="${csp.nonce}"` : ""}>${init_app}</script>
		`;
  }
  const headers = new Headers({
    "x-sveltekit-page": "true",
    "content-type": "text/html"
  });
  if (state.prerendering || state.prerender_default === true) {
    const csp_headers = csp.csp_provider.get_meta();
    if (csp_headers)
      head.add_http_equiv(csp_headers);
    if (state.prerendering?.cache)
      head.add_http_equiv(`<meta http-equiv="cache-control" content="${state.prerendering.cache}">`);
  } else {
    const csp_header = csp.csp_provider.get_header();
    if (csp_header)
      headers.set("content-security-policy", csp_header);
    const report_only_header = csp.report_only_provider.get_header();
    if (report_only_header)
      headers.set("content-security-policy-report-only", report_only_header);
  }
  const html = options$1.templates.app({
    head: head.build(),
    body,
    assets: assets$1,
    nonce: csp.nonce,
    env: explicit_public_env
  });
  const transformed = await resolve_opts.transformPageChunk({
    html,
    done: true
  }) || "";
  if (!chunks)
    headers.set("etag", `"${hash(transformed)}"`);
  return !chunks ? text(transformed, {
    status,
    headers
  }) : new Response(stream_text(transformed + `
`, chunks), { headers });
}
var Head = class {
  #rendered;
  #http_equiv = [];
  #link_tags = [];
  #style_tags = [];
  #stylesheet_links = [];
  constructor(rendered) {
    this.#rendered = rendered;
  }
  build() {
    return [
      ...this.#http_equiv,
      ...this.#link_tags,
      this.#rendered,
      ...this.#style_tags,
      ...this.#stylesheet_links
    ].join(`
		`);
  }
  add_style(style, attributes) {
    this.#style_tags.push(`<style${attributes.length ? " " + attributes.join(" ") : ""}>${style}</style>`);
  }
  add_stylesheet(href, attributes) {
    this.#stylesheet_links.push(`<link href="${href}" ${attributes.join(" ")}>`);
  }
  add_link_tag(href, attributes) {
    this.#link_tags.push(`<link href="${href}" ${attributes.join(" ")}>`);
  }
  add_http_equiv(tag) {
    this.#http_equiv.push(tag);
  }
};
function validator(expected) {
  function validate(module2, file) {
    if (!module2)
      return;
    for (const key2 in module2) {
      if (key2[0] === "_" || expected.has(key2))
        continue;
      const values = [...expected.values()];
      const hint = hint_for_supported_files(key2, file?.slice(file.lastIndexOf("."))) ?? `valid exports are ${values.join(", ")}, or anything with a '_' prefix`;
      throw new Error(`Invalid export '${key2}'${file ? ` in ${file}` : ""} (${hint})`);
    }
  }
  return validate;
}
function hint_for_supported_files(key2, ext = ".js") {
  const supported_files = [];
  if (valid_layout_exports.has(key2))
    supported_files.push(`+layout${ext}`);
  if (valid_page_exports.has(key2))
    supported_files.push(`+page${ext}`);
  if (valid_layout_server_exports.has(key2))
    supported_files.push(`+layout.server${ext}`);
  if (valid_page_server_exports.has(key2))
    supported_files.push(`+page.server${ext}`);
  if (valid_server_exports.has(key2))
    supported_files.push(`+server${ext}`);
  if (supported_files.length > 0)
    return `'${key2}' is a valid export in ${supported_files.slice(0, -1).join(", ")}${supported_files.length > 1 ? " or " : ""}${supported_files.at(-1)}`;
}
var valid_layout_exports = /* @__PURE__ */ new Set([
  "load",
  "prerender",
  "csr",
  "ssr",
  "trailingSlash",
  "config"
]);
var valid_page_exports = /* @__PURE__ */ new Set([...valid_layout_exports, "entries"]);
var valid_layout_server_exports = new Set(valid_layout_exports);
var valid_page_server_exports = /* @__PURE__ */ new Set([
  ...valid_layout_server_exports,
  "actions",
  "entries"
]);
var valid_server_exports = /* @__PURE__ */ new Set([
  "GET",
  "POST",
  "PATCH",
  "PUT",
  "DELETE",
  "OPTIONS",
  "HEAD",
  "QUERY",
  "fallback",
  "prerender",
  "trailingSlash",
  "config",
  "entries"
]);
var validate_layout_exports = validator(valid_layout_exports);
var validate_page_exports = validator(valid_page_exports);
var validate_layout_server_exports = validator(valid_layout_server_exports);
var validate_page_server_exports = validator(valid_page_server_exports);
var PageNodes = class {
  data;
  constructor(nodes) {
    this.data = nodes;
  }
  layouts() {
    return this.data.slice(0, -1);
  }
  page() {
    return this.data.at(-1);
  }
  validate() {
    for (const layout of this.layouts())
      if (layout) {
        validate_layout_server_exports(layout.server, layout.server_id);
        validate_layout_exports(layout.universal, layout.universal_id);
      }
    const page = this.page();
    if (page) {
      validate_page_server_exports(page.server, page.server_id);
      validate_page_exports(page.universal, page.universal_id);
    }
  }
  #get_option(option) {
    return this.data.reduce((value, node) => {
      return node?.universal?.[option] ?? node?.server?.[option] ?? value;
    }, undefined);
  }
  csr() {
    return this.#get_option("csr") ?? true;
  }
  ssr() {
    return this.#get_option("ssr") ?? true;
  }
  prerender() {
    return this.#get_option("prerender") ?? false;
  }
  trailing_slash() {
    return this.#get_option("trailingSlash") ?? "never";
  }
  get_config() {
    let current = {};
    for (const node of this.data) {
      if (!node?.universal?.config && !node?.server?.config)
        continue;
      current = {
        ...current,
        ...node?.server?.config,
        ...node?.universal?.config
      };
    }
    return Object.keys(current).length ? current : undefined;
  }
  should_prerender_data() {
    return this.data.some((node) => node?.server?.load || node?.server?.trailingSlash !== undefined);
  }
};
async function respond_with_error({ event, state, manifest: manifest2, error: error2, resolve_opts }) {
  if (event.request.headers.get("x-sveltekit-error")) {
    const transformed = await handle_error_and_jsonify(event, state, error2);
    return static_error_page(transformed.status, transformed.message);
  }
  const fetched = [];
  try {
    const branch = [];
    const default_layout = await manifest2._.nodes[0]();
    const nodes = new PageNodes([default_layout]);
    const ssr = nodes.ssr();
    const csr = nodes.csr();
    const data_serializer = server_data_serializer(event, state);
    const transformed = await handle_error_and_jsonify(event, state, error2);
    if (ssr) {
      state.error = true;
      const server_data_promise = load_server_data({
        event,
        state,
        node: default_layout,
        parent: async () => ({})
      });
      const server_data = await server_data_promise;
      data_serializer.add_node(0, server_data);
      const data = await load_data({
        event,
        state,
        fetched,
        node: default_layout,
        parent: async () => ({}),
        resolve_opts,
        server_data_promise,
        csr
      });
      branch.push({
        node: default_layout,
        server_data,
        data
      }, {
        node: await manifest2._.nodes[1](),
        data: null,
        server_data: null
      });
    }
    return await render_response({
      manifest: manifest2,
      page_config: {
        ssr,
        csr
      },
      status: transformed.status,
      error: transformed,
      branch,
      error_components: [],
      fetched,
      event,
      state,
      resolve_opts,
      data_serializer
    });
  } catch (e) {
    if (e instanceof Redirect)
      return redirect_response(e.status, e.location);
    const transformed = await handle_error_and_jsonify(event, state, e);
    return static_error_page(transformed.status, transformed.message);
  }
}
var MAX_DEPTH = 10;
async function render_page(event, state, page, manifest2, nodes, resolve_opts) {
  if (state.depth > MAX_DEPTH)
    return text(`Not found: ${event.url.pathname}`, { status: 404 });
  if (is_action_json_request(event)) {
    const node = await manifest2._.nodes[page.leaf]();
    return handle_action_json_request(event, state, node?.server);
  }
  try {
    const leaf_node = nodes.page();
    let status = 200;
    let action_result = undefined;
    if (is_action_request(event)) {
      const remote_id = get_remote_action(event.url);
      if (remote_id)
        action_result = await handle_remote_form_post(event, state, manifest2, remote_id);
      else
        action_result = await handle_action_request(event, state, leaf_node.server);
      if (action_result?.type === "redirect")
        return redirect_response(action_result.status, action_result.location);
      if (action_result?.type === "error")
        status = get_status(action_result.error);
      if (action_result?.type === "failure")
        status = action_result.status;
    }
    const should_prerender = nodes.prerender();
    if (should_prerender) {
      if (leaf_node.server?.actions)
        throw new Error("Cannot prerender pages with actions");
    } else if (state.prerendering)
      return new Response(undefined, { status: 204 });
    state.prerender_default = should_prerender;
    const should_prerender_data = nodes.should_prerender_data();
    const data_pathname = add_data_suffix(event.url.pathname);
    const fetched = [];
    const ssr = nodes.ssr();
    const csr = nodes.csr();
    if (ssr === false && !((state.prerendering || state.prerender_default === true) && should_prerender_data))
      return await render_response({
        branch: compact(nodes.data).map((node) => {
          return {
            node,
            data: null,
            server_data: null
          };
        }),
        fetched,
        page_config: {
          ssr: false,
          csr
        },
        status,
        error: null,
        event,
        state,
        manifest: manifest2,
        resolve_opts,
        data_serializer: server_data_serializer(event, state)
      });
    const branch = [];
    let load_error = null;
    const data_serializer = server_data_serializer(event, state);
    const data_serializer_json = (state.prerendering || state.prerender_default === true) && should_prerender_data ? server_data_serializer_json(event, state) : null;
    const server_promises = nodes.data.map((node, i) => {
      if (load_error)
        throw load_error;
      return Promise.resolve().then(async () => {
        try {
          if (node === leaf_node && action_result?.type === "error")
            throw action_result.error;
          const server_data = await load_server_data({
            event,
            state,
            node,
            parent: async () => {
              const data = {};
              for (let j = 0;j < i; j += 1) {
                const parent = await server_promises[j];
                if (parent)
                  Object.assign(data, parent.data);
              }
              return data;
            }
          });
          if (node)
            data_serializer.add_node(i, server_data);
          data_serializer_json?.add_node(i, server_data);
          return server_data;
        } catch (e) {
          load_error = e;
          throw load_error;
        }
      });
    });
    const load_promises = nodes.data.map((node, i) => {
      if (load_error)
        throw load_error;
      return Promise.resolve().then(async () => {
        try {
          return await load_data({
            event,
            state,
            fetched,
            node,
            parent: async () => {
              const data = {};
              for (let j = 0;j < i; j += 1)
                Object.assign(data, await load_promises[j]);
              return data;
            },
            resolve_opts,
            server_data_promise: server_promises[i],
            csr
          });
        } catch (e) {
          load_error = e;
          throw load_error;
        }
      });
    });
    for (const p of server_promises)
      p.catch(noop);
    for (const p of load_promises)
      p.catch(noop);
    for (let i = 0;i < nodes.data.length; i += 1) {
      const node = nodes.data[i];
      if (node)
        try {
          const server_data = await server_promises[i];
          const data = await load_promises[i];
          branch.push({
            node,
            server_data,
            data
          });
        } catch (e) {
          const err = normalize_error(e);
          if (err instanceof Redirect) {
            if (state.prerendering && should_prerender_data) {
              const body = JSON.stringify({
                type: "redirect",
                status: err.status,
                location: err.location
              });
              state.prerendering.dependencies.set(data_pathname, {
                response: text(body),
                body
              });
            }
            return redirect_response(err.status, err.location);
          }
          const error2 = await handle_error_and_jsonify(event, state, err);
          const status2 = error2.status;
          for (const { error: index, idx } of nearest_error_pages(i, branch, page.errors)) {
            const node2 = await manifest2._.nodes[index]();
            data_serializer.set_max_nodes(idx);
            const layouts = compact(branch.slice(0, idx));
            const nodes2 = new PageNodes(layouts.map((layout) => layout.node));
            const error_branch = layouts.concat({
              node: node2,
              data: null,
              server_data: null
            });
            return await render_response({
              event,
              state,
              manifest: manifest2,
              resolve_opts,
              page_config: {
                ssr: nodes2.ssr(),
                csr: nodes2.csr()
              },
              status: status2,
              error: error2,
              error_components: await load_error_components(ssr, error_branch, page, manifest2),
              branch: error_branch,
              fetched,
              data_serializer
            });
          }
          return static_error_page(status2, error2.message);
        }
      else
        branch.push(null);
    }
    if (state.prerendering && data_serializer_json) {
      let { data, chunks } = data_serializer_json.get_data();
      if (chunks)
        for await (const chunk of chunks)
          data += chunk;
      state.prerendering.dependencies.set(data_pathname, {
        response: text(data),
        body: data
      });
    }
    return await render_response({
      event,
      state,
      manifest: manifest2,
      resolve_opts,
      page_config: {
        csr,
        ssr
      },
      status,
      error: null,
      branch: compact(branch),
      action_result,
      fetched,
      data_serializer: !ssr ? server_data_serializer(event, state) : data_serializer,
      error_components: await load_error_components(ssr, branch, page, manifest2)
    });
  } catch (e) {
    if (e instanceof Redirect)
      return redirect_response(e.status, e.location);
    return await respond_with_error({
      event,
      state,
      manifest: manifest2,
      error: e,
      resolve_opts
    });
  }
}
function load_error_components(ssr, branch, page, manifest2) {
  if (!ssr)
    return;
  return build_error_chain(branch, page.errors, (idx) => manifest2._.nodes[idx]?.().then((e) => e.component?.()));
}
var mutating_form_methods = /* @__PURE__ */ new Set([
  "POST",
  "PUT",
  "PATCH",
  "DELETE"
]);
function get_self_origin(paths_origin, url_origin) {
  return paths_origin || url_origin;
}
function is_csrf_forbidden({ request, request_origin, self_origin, trusted_origins }) {
  return (!request.headers.get("content-type") || is_form_content_type(request)) && mutating_form_methods.has(request.method) && request_origin !== self_origin && (!request_origin || !trusted_origins.includes(request_origin));
}
function is_remote_forbidden({ request, request_origin, self_origin }) {
  return request.method !== "GET" && request_origin !== self_origin;
}
async function render_data(event, state, route, manifest2, invalidated_data_nodes, trailing_slash) {
  if (!route.page)
    return with_version_header(new Response(undefined, { status: 404 }));
  try {
    const node_ids = [...route.page.layouts, route.page.leaf];
    const invalidated = invalidated_data_nodes ?? node_ids.map(() => true);
    let aborted = false;
    const url2 = new URL(event.url);
    url2.pathname = normalize_path(url2.pathname, trailing_slash);
    const new_event = {
      ...event,
      url: url2
    };
    const functions = node_ids.map((n, i) => {
      return once(async () => {
        try {
          if (aborted)
            return { type: "skip" };
          const node = n == undefined ? n : await manifest2._.nodes[n]();
          return load_server_data({
            event: new_event,
            state,
            node,
            parent: async () => {
              const data2 = {};
              for (let j = 0;j < i; j += 1) {
                const parent = await functions[j]();
                if (parent)
                  Object.assign(data2, parent.data);
              }
              return data2;
            }
          });
        } catch (e) {
          aborted = true;
          throw e;
        }
      });
    });
    const promises = functions.map(async (fn, i) => {
      if (!invalidated[i])
        return { type: "skip" };
      return fn();
    });
    const data_serializer = server_data_serializer_json(event, state);
    await Promise.all(promises.map(async (p, i) => {
      const node = await p.catch(async (error2) => {
        if (error2 instanceof Redirect)
          throw error2;
        return {
          type: "error",
          error: await handle_error_and_jsonify(event, state, error2)
        };
      });
      data_serializer.add_node(i, node);
    }));
    const { data, chunks } = data_serializer.get_data();
    if (!chunks)
      return json_response(data);
    return with_version_header(new Response(stream_text(data, chunks), { headers: {
      "content-type": "text/sveltekit-data",
      "cache-control": "private, no-store"
    } }));
  } catch (e) {
    const error2 = normalize_error(e);
    if (error2 instanceof Redirect)
      return redirect_json_response(error2);
    else {
      const transformed = await handle_error_and_jsonify(event, state, error2);
      return json_response(transformed, transformed.status);
    }
  }
}
function json_response(json, status = 200) {
  return with_version_header(text(typeof json === "string" ? json : JSON.stringify(json), {
    status,
    headers: {
      "content-type": "application/json",
      "cache-control": "private, no-store"
    }
  }));
}
function redirect_json_response(redirect) {
  return json_response({
    type: "redirect",
    status: redirect.status,
    location: redirect.location
  });
}
function generate_cookie_key(domain, path3, name) {
  return `${domain || ""}${path3}?${encodeURIComponent(name)}`;
}
function get_cookies(request, url2) {
  const header = request.headers.get("cookie") ?? "";
  const initial_cookies = parseCookie(header, { decode: (value) => value });
  let default_cookies;
  function parse_header(opts) {
    return opts?.decode ? parseCookie(header, opts) : default_cookies ??= parseCookie(header);
  }
  function matches_url(cookie) {
    return domain_matches(url2.hostname, cookie.options.domain) && path_matches(url2.pathname, cookie.options.path);
  }
  let normalized_url;
  const new_cookies = /* @__PURE__ */ new Map;
  const defaults = {
    httpOnly: true,
    path: "/",
    sameSite: "lax",
    secure: !(url2.hostname === "localhost" && url2.protocol === "http:")
  };
  const cookies = {
    get(name, opts) {
      let best_match;
      for (const c of new_cookies.values())
        if (c.name === name && matches_url(c) && (!best_match || c.options.path.length > best_match.options.path.length))
          best_match = c;
      if (best_match)
        return best_match.options.maxAge === 0 ? undefined : best_match.value;
      return parse_header(opts)[name];
    },
    getAll(opts) {
      const cookies2 = { ...parse_header(opts) };
      const lookup = /* @__PURE__ */ new Map;
      for (const c of new_cookies.values())
        if (matches_url(c)) {
          const existing = lookup.get(c.name);
          if (!existing || c.options.path.length > existing.options.path.length)
            lookup.set(c.name, c);
        }
      for (const c of lookup.values())
        if (c.options.maxAge === 0)
          delete cookies2[c.name];
        else
          cookies2[c.name] = c.value;
      return Object.entries(cookies2).filter(([, value]) => value != null).map(([name, value]) => ({
        name,
        value
      }));
    },
    set(name, value, options2) {
      set_internal(name, value, {
        ...defaults,
        ...options2
      });
    },
    delete(name, options2) {
      cookies.set(name, "", {
        ...options2,
        maxAge: 0
      });
    },
    parse: parseSetCookie,
    serialize(name, value, { encode: encode2, ...options2 }) {
      let path3 = options2.path ?? "/";
      if (!options2.domain || options2.domain === url2.hostname) {
        if (!normalized_url)
          throw new Error("Cannot serialize cookies until after the route is determined");
        path3 = resolve(normalized_url, path3);
      }
      return stringifySetCookie({
        name,
        value,
        ...defaults,
        ...options2,
        path: path3
      }, { encode: encode2 });
    }
  };
  function get_cookie_header(destination, header2) {
    const combined_cookies = { ...initial_cookies };
    for (const cookie of new_cookies.values()) {
      if (!domain_matches(destination.hostname, cookie.options.domain))
        continue;
      if (!path_matches(destination.pathname, cookie.options.path))
        continue;
      const encoder = cookie.options.encode || encodeURIComponent;
      combined_cookies[cookie.name] = encoder(cookie.value);
    }
    if (header2) {
      const parsed = parseCookie(header2, { decode: (value) => value });
      for (const name in parsed)
        combined_cookies[name] = parsed[name];
    }
    return Object.entries(combined_cookies).map(([name, value]) => `${name}=${value}`).join("; ");
  }
  const internal_queue = [];
  function set_internal(name, value, options2) {
    if (!normalized_url) {
      internal_queue.push(() => set_internal(name, value, options2));
      return;
    }
    let path3 = options2.path ?? "/";
    if (!options2.domain || options2.domain === url2.hostname)
      path3 = resolve(normalized_url, path3);
    const cookie_key = generate_cookie_key(options2.domain, path3, name);
    const cookie = {
      name,
      value,
      options: {
        ...options2,
        path: path3
      }
    };
    new_cookies.set(cookie_key, cookie);
  }
  function set_trailing_slash(trailing_slash) {
    normalized_url = normalize_path(url2.pathname, trailing_slash);
    internal_queue.forEach((fn) => fn());
  }
  return {
    cookies,
    new_cookies,
    get_cookie_header,
    set_internal,
    set_trailing_slash
  };
}
function domain_matches(hostname, constraint) {
  if (!constraint)
    return true;
  const normalized = constraint[0] === "." ? constraint.slice(1) : constraint;
  if (hostname === normalized)
    return true;
  return hostname.endsWith("." + normalized);
}
function path_matches(path3, constraint) {
  if (!constraint)
    return true;
  const normalized = constraint.endsWith("/") ? constraint.slice(0, -1) : constraint;
  if (path3 === normalized)
    return true;
  return path3.startsWith(normalized + "/");
}
function add_cookies_to_headers(headers, cookies) {
  for (const new_cookie of cookies) {
    const { name, value, options: { encode: encode2, ...options2 } } = new_cookie;
    headers.append("set-cookie", stringifySetCookie({
      name,
      value,
      ...options2
    }, { encode: encode2 }));
    if (options2.path.endsWith(".html")) {
      const path3 = add_data_suffix(options2.path);
      headers.append("set-cookie", stringifySetCookie({
        name,
        value,
        ...options2,
        path: path3
      }, { encode: encode2 }));
    }
  }
}
function transient_fields() {
  return {
    remote: {
      data: null,
      explicit: null,
      implicit: null,
      forms: null,
      requested: null,
      batches: null,
      live_iterators: null
    },
    is_in_remote_function: false,
    is_in_remote_form_or_command: false,
    is_in_remote_query: false,
    is_in_remote_prerender: false,
    is_in_render: false
  };
}
function create_request_state(options2) {
  return {
    getClientAddress: options2.getClientAddress,
    platform: options2.platform,
    read: options2.read,
    before_handle: options2.before_handle,
    emulator: options2.emulator,
    prerendering: options2.prerendering,
    prerender_default: undefined,
    error: false,
    depth: 0,
    rerouted_url: null,
    ...transient_fields()
  };
}
function fork_state_for_subrequest(state) {
  return {
    ...state,
    ...transient_fields(),
    depth: state.depth + 1
  };
}
function create_fetch({ event, manifest: manifest2, state, get_cookie_header, set_internal }) {
  const server_fetch = async (info, init2) => {
    const original_request = normalize_fetch_input(info, init2, event.url);
    let mode = (info instanceof Request ? info.mode : init2?.mode) ?? "cors";
    let credentials = (info instanceof Request ? info.credentials : init2?.credentials) ?? "same-origin";
    return hooks.handleFetch({
      event,
      request: original_request,
      fetch: async (info2, init3) => {
        const request = normalize_fetch_input(info2, init3, event.url);
        const url2 = new URL(request.url);
        if (!request.headers.has("origin"))
          request.headers.set("origin", event.url.origin);
        if (info2 !== original_request) {
          mode = (info2 instanceof Request ? info2.mode : init3?.mode) ?? "cors";
          credentials = (info2 instanceof Request ? info2.credentials : init3?.credentials) ?? "same-origin";
        }
        if ((request.method === "GET" || request.method === "HEAD") && (mode === "no-cors" && url2.origin !== event.url.origin || url2.origin === event.url.origin))
          request.headers.delete("origin");
        const decoded = decodeURIComponent(url2.pathname);
        if (url2.origin !== event.url.origin || "") {
          if (`.${url2.hostname}`.endsWith(`.${event.url.hostname}`) && credentials !== "omit") {
            const cookie = get_cookie_header(url2, request.headers.get("cookie"));
            if (cookie)
              request.headers.set("cookie", cookie);
          }
          return fetch(request);
        }
        const filename = (decoded.startsWith(assets) ? decoded.slice(assets.length) : decoded).slice(1);
        const filename_html = `${filename}/index.html`;
        const is_asset = manifest2.assets.has(filename) || filename in manifest2._.server_assets;
        const is_asset_html = manifest2.assets.has(filename_html) || filename_html in manifest2._.server_assets;
        if (is_asset || is_asset_html) {
          const file = is_asset ? filename : filename_html;
          if (state.read) {
            const type = is_asset ? manifest2.mimeTypes[filename.slice(filename.lastIndexOf("."))] : "text/html";
            return new Response(state.read(file), { headers: type ? { "content-type": type } : {} });
          } else if (read_implementation && file in manifest2._.server_assets) {
            const length = manifest2._.server_assets[file];
            const type = manifest2.mimeTypes[file.slice(file.lastIndexOf("."))];
            return new Response(read_implementation(file), { headers: {
              "Content-Length": "" + length,
              "Content-Type": type
            } });
          }
          return await fetch(request);
        }
        if (has_prerendered_path(manifest2, decoded))
          return await fetch(request);
        if (credentials !== "omit") {
          const cookie = get_cookie_header(url2, request.headers.get("cookie"));
          if (cookie)
            request.headers.set("cookie", cookie);
          const authorization = event.request.headers.get("authorization");
          if (authorization && !request.headers.has("authorization"))
            request.headers.set("authorization", authorization);
        }
        if (!request.headers.has("accept"))
          request.headers.set("accept", "*/*");
        const accept_language = event.request.headers.get("accept-language");
        if (accept_language && !request.headers.has("accept-language"))
          request.headers.set("accept-language", accept_language);
        const response = await internal_fetch(request, manifest2, state);
        for (const str of response.headers.getSetCookie()) {
          const { name, value, ...cookie_options } = parseSetCookie(str, { decode: (v) => v });
          set_internal(name, value, {
            path: cookie_options.path ?? (url2.pathname.split("/").slice(0, -1).join("/") || "/"),
            encode: (value2) => value2,
            ...cookie_options
          });
        }
        return response;
      }
    });
  };
  return (input, init2) => {
    const response = server_fetch(input, init2);
    response.catch(noop);
    return response;
  };
}
function normalize_fetch_input(info, init2, url2) {
  if (info instanceof Request)
    return info;
  return new Request(typeof info === "string" ? new URL(info, url2) : info, init2);
}
async function internal_fetch(request, manifest2, state) {
  if (request.signal?.aborted)
    throw new DOMException("The operation was aborted.", "AbortError");
  const subrequest_state = fork_state_for_subrequest(state);
  if (!request.signal)
    return await respond(request, manifest2, subrequest_state);
  let remove_abort_listener = noop;
  const abort_promise = new Promise((_, reject) => {
    const on_abort = () => {
      reject(new DOMException("The operation was aborted.", "AbortError"));
    };
    request.signal.addEventListener("abort", on_abort, { once: true });
    remove_abort_listener = () => request.signal.removeEventListener("abort", on_abort);
  });
  return Promise.race([respond(request, manifest2, subrequest_state), abort_promise]).finally(remove_abort_listener);
}
var payload;
var etag;
var headers;
function get_public_env(request) {
  const env = rendered_env;
  payload ??= uneval(env);
  etag ??= `W/${Date.now()}`;
  headers ??= new Headers({
    "content-type": "application/javascript; charset=utf-8",
    etag
  });
  if (request.headers.get("if-none-match") === etag)
    return new Response(undefined, {
      status: 304,
      headers
    });
  return new Response(`export const env=${payload}`, { headers });
}
var default_transform = ({ html }) => html;
var default_filter = () => false;
var default_preload = ({ type }) => type === "js" || type === "css";
var non_html_fetch_destinations = /* @__PURE__ */ new Set([
  "audio",
  "audioworklet",
  "font",
  "image",
  "json",
  "manifest",
  "paintworklet",
  "report",
  "script",
  "serviceworker",
  "sharedworker",
  "style",
  "track",
  "video",
  "webidentity",
  "worker",
  "xslt"
]);
var page_methods = /* @__PURE__ */ new Set([
  "GET",
  "HEAD",
  "POST"
]);
var allowed_page_methods = /* @__PURE__ */ new Set([
  "GET",
  "HEAD",
  "OPTIONS"
]);
var respond = propagate_context(internal_respond);
async function internal_respond(request, manifest2, state) {
  const url2 = new URL(request.url);
  const is_route_resolution_request = has_resolution_suffix(url2.pathname);
  const is_data_request = has_data_suffix(url2.pathname);
  const remote_id = get_remote_id(url2);
  {
    const request_origin = request.headers.get("origin");
    const self_origin = get_self_origin(undefined, url2.origin);
    if (remote_id) {
      if (is_remote_forbidden({
        request,
        request_origin,
        self_origin
      }))
        return Response.json({ message: "Cross-site remote requests are forbidden" }, { status: 403 });
    } else if (is_csrf_forbidden({
      request,
      request_origin,
      self_origin,
      trusted_origins: options$1.csrf_trusted_origins
    })) {
      const message = `Cross-site ${request.method} form submissions are forbidden`;
      const opts = { status: 403 };
      if (request.headers.get("accept") === "application/json")
        return Response.json({ message }, opts);
      return text(message, opts);
    }
  }
  let invalidated_data_nodes;
  let skip_route_resolution = false;
  let is_route_id_resolution_request = false;
  if (is_route_resolution_request) {
    url2.pathname = strip_resolution_suffix(url2.pathname);
    is_route_id_resolution_request = is_route_id_resolution_path(url2.pathname);
  } else if (is_data_request) {
    url2.pathname = strip_data_suffix(url2.pathname) + (url2.searchParams.get("x-sveltekit-trailing-slash") === "1" ? "/" : "") || "/";
    url2.searchParams.delete(TRAILING_SLASH_PARAM);
    invalidated_data_nodes = url2.searchParams.get(INVALIDATED_PARAM)?.split("").map((node) => node === "1");
    url2.searchParams.delete(INVALIDATED_PARAM);
  } else if (remote_id) {
    const pathname = request.headers.get("x-sveltekit-pathname");
    if (pathname === null)
      skip_route_resolution = true;
    else {
      url2.pathname = pathname;
      url2.search = request.headers.get("x-sveltekit-search") ?? "";
    }
  }
  const headers2 = {};
  const { cookies, new_cookies, get_cookie_header, set_internal, set_trailing_slash } = get_cookies(request, url2);
  const event = {
    cookies,
    fetch: null,
    getClientAddress: state.getClientAddress || (() => {
      throw new Error(`@sveltejs/adapter-bun does not specify getClientAddress. Please raise an issue`);
    }),
    locals: {},
    params: {},
    platform: state.emulator?.platform ? await state.emulator.platform({
      config: {},
      prerender: !!state.prerendering?.fallback
    }) : state.platform,
    request,
    route: { id: null },
    setHeaders: (new_headers) => {
      for (const key2 in new_headers) {
        const lower = key2.toLowerCase();
        const value = new_headers[key2];
        if (lower === "set-cookie")
          throw new Error("Use `event.cookies.set(name, value, options)` instead of `event.setHeaders` to set cookies");
        else if (lower in headers2) {
          if (lower === "server-timing")
            headers2[lower] += ", " + value;
          else
            throw new Error(`"${key2}" header is already set`);
        } else {
          headers2[lower] = value;
          if (state.prerendering && lower === "cache-control")
            state.prerendering.cache = value;
        }
      }
    },
    url: url2,
    isDataRequest: is_data_request,
    isSubRequest: state.depth > 0,
    isRemoteRequest: !!remote_id
  };
  event.fetch = create_fetch({
    event,
    manifest: manifest2,
    state,
    get_cookie_header,
    set_internal
  });
  let resolved_path = url2.pathname;
  if (!remote_id && !is_route_id_resolution_request) {
    const prerendering_reroute_state = state.prerendering?.inside_reroute;
    try {
      if (state.prerendering)
        state.prerendering.inside_reroute = true;
      resolved_path = await hooks.reroute({
        url: new URL(url2),
        fetch: event.fetch
      }) ?? url2.pathname;
      if (!manifest2._.routes.length && resolved_path !== url2.pathname)
        state.rerouted_url = denormalise_url({
          request_url: request.url,
          resolved_path,
          is_data_request,
          is_route_resolution_request
        }).toString();
    } catch {
      return text("Internal Server Error", { status: 500 });
    } finally {
      if (state.prerendering)
        state.prerendering.inside_reroute = prerendering_reroute_state;
    }
  }
  let resolve_opts = {
    transformPageChunk: default_transform,
    filterSerializedResponseHeaders: default_filter,
    preload: default_preload
  };
  let trailing_slash = "never";
  let page_nodes;
  try {
    resolved_path = decode_pathname(resolved_path);
  } catch {
    resolved_path = null;
    return await handle();
  }
  if (resolved_path !== decode_pathname(url2.pathname) && !state.prerendering?.fallback && has_prerendered_path(manifest2, resolved_path)) {
    const url3 = denormalise_url({
      request_url: request.url,
      resolved_path,
      is_data_request,
      is_route_resolution_request
    });
    try {
      const response = await fetch(url3, request);
      const headers3 = new Headers(response.headers);
      if (headers3.has("content-encoding")) {
        headers3.delete("content-encoding");
        headers3.delete("content-length");
      }
      return new Response(response.body, {
        headers: headers3,
        status: response.status,
        statusText: response.statusText
      });
    } catch (error2) {
      return await handle_fatal_error(event, state, error2);
    }
  }
  let route = null;
  if (is_route_resolution_request) {
    if (is_route_id_resolution_request)
      return resolve_route_by_id(extract_route_id(resolved_path), new URL(request.url), manifest2);
    return resolve_route(resolved_path, new URL(request.url), manifest2);
  }
  if (resolved_path === `/_app/env.js`)
    return get_public_env(request);
  if (!remote_id && resolved_path.startsWith(`/_app`)) {
    const headers3 = new Headers;
    headers3.set("cache-control", "public, max-age=0, must-revalidate");
    return text("Not found", {
      status: 404,
      headers: headers3
    });
  }
  if (!state.prerendering?.fallback && !skip_route_resolution)
    try {
      const matchers = await manifest2._.matchers();
      const result = find_route(resolved_path, manifest2._.routes, matchers);
      if (result) {
        route = result.route;
        event.route = { id: route.id };
        event.params = result.params;
      }
    } catch (e) {
      return await handle_fatal_error(event, state, e);
    }
  try {
    page_nodes = route?.page ? new PageNodes(await load_page_nodes(route.page, manifest2)) : undefined;
    if (route && !remote_id) {
      if (url2.pathname === "" || url2.pathname === "/")
        trailing_slash = "always";
      else if (page_nodes)
        trailing_slash = page_nodes.trailing_slash();
      else if (route.endpoint)
        trailing_slash = (await route.endpoint()).trailingSlash ?? "never";
      if (!is_data_request) {
        const normalized = normalize_path(url2.pathname, trailing_slash);
        if (normalized !== url2.pathname && !state.prerendering?.fallback)
          return new Response(undefined, {
            status: 308,
            headers: {
              "x-sveltekit-normalize": "1",
              location: relative_pathname(url2.pathname, normalized) + (url2.search === "?" ? "" : url2.search)
            }
          });
      }
      if (state.before_handle || state.emulator?.platform) {
        let config = {};
        let prerender = false;
        if (route.endpoint) {
          const node = await route.endpoint();
          config = node.config ?? config;
          prerender = node.prerender ?? prerender;
        } else if (page_nodes) {
          config = page_nodes.get_config() ?? config;
          prerender = state.prerender_default = page_nodes.prerender();
        }
        if (state.emulator?.platform)
          event.platform = await state.emulator.platform({
            config,
            prerender
          });
        if (state.before_handle)
          return await state.before_handle(event, config, prerender, handle);
      }
    }
    return await handle();
  } catch (e) {
    if (e instanceof Redirect)
      try {
        const response = is_data_request || remote_id ? redirect_json_response(e) : route?.page && is_action_json_request(event) ? action_json_redirect(e) : redirect_response(e.status, e.location);
        add_cookies_to_headers(response.headers, new_cookies.values());
        return response;
      } catch (err) {
        return await handle_fatal_error(event, state, err);
      }
    return await handle_fatal_error(event, state, e);
  }
  async function handle() {
    set_trailing_slash(trailing_slash);
    if (state.prerendering && !state.prerendering.fallback && !state.prerendering.inside_reroute)
      disable_search(url2);
    const response = await record_span({
      name: "sveltekit.handle.root",
      attributes: {
        "http.route": event.route.id || "unknown",
        "http.method": event.request.method,
        "http.url": event.url.href,
        "sveltekit.is_data_request": is_data_request,
        "sveltekit.is_sub_request": event.isSubRequest
      },
      fn: async (root_span) => {
        const traced_event = {
          ...event,
          tracing: {
            enabled: false,
            root: root_span,
            current: root_span
          }
        };
        return await with_request_store({
          event: traced_event,
          state
        }, () => hooks.handle({
          event: traced_event,
          resolve: (event2, opts) => {
            return record_span({
              name: "sveltekit.resolve",
              attributes: { "http.route": event2.route.id || "unknown" },
              fn: (resolve_span) => {
                return with_request_store(null, () => resolve2(merge_tracing(event2, resolve_span), page_nodes, opts).then((response2) => {
                  for (const key2 in headers2) {
                    const value = headers2[key2];
                    response2.headers.set(key2, value);
                  }
                  add_cookies_to_headers(response2.headers, new_cookies.values());
                  if (state.prerendering && event2.route.id !== null)
                    response2.headers.set("x-sveltekit-routeid", encodeURI(event2.route.id));
                  resolve_span.setAttributes({
                    "http.response.status_code": response2.status,
                    "http.response.body.size": response2.headers.get("content-length") || "unknown"
                  });
                  return response2;
                }));
              }
            });
          }
        }));
      }
    });
    if (response.status === 200 && response.headers.has("etag")) {
      let if_none_match_value = request.headers.get("if-none-match");
      if (if_none_match_value?.startsWith('W/"'))
        if_none_match_value = if_none_match_value.substring(2);
      const etag2 = response.headers.get("etag");
      if (if_none_match_value === etag2) {
        const headers3 = new Headers({ etag: etag2 });
        for (const key2 of [
          "cache-control",
          "content-location",
          "date",
          "expires",
          "vary"
        ]) {
          const value = response.headers.get(key2);
          if (value)
            headers3.set(key2, value);
        }
        for (const cookie of response.headers.getSetCookie())
          headers3.append("set-cookie", cookie);
        return new Response(undefined, {
          status: 304,
          headers: headers3
        });
      }
    }
    if (is_data_request && response.status >= 300 && response.status <= 308) {
      const location = response.headers.get("location");
      if (location)
        return redirect_json_response(new Redirect(response.status, location));
    }
    return response;
  }
  async function resolve2(event2, page_nodes2, opts) {
    try {
      if (opts)
        resolve_opts = {
          transformPageChunk: opts.transformPageChunk || default_transform,
          filterSerializedResponseHeaders: opts.filterSerializedResponseHeaders || default_filter,
          preload: opts.preload || default_preload
        };
      if (resolved_path === null)
        return await respond_with_error({
          event: event2,
          state,
          manifest: manifest2,
          error: new SvelteKitError(400, "Malformed URI", `Failed to decode URI: ${event2.url.pathname}`),
          resolve_opts
        });
      if (state.prerendering?.fallback)
        return await render_response({
          event: event2,
          state,
          manifest: manifest2,
          page_config: {
            ssr: false,
            csr: true
          },
          status: 200,
          error: null,
          branch: [{
            node: await manifest2._.nodes[0](),
            data: null,
            server_data: null
          }],
          fetched: [],
          resolve_opts,
          data_serializer: server_data_serializer(event2, state)
        });
      if (remote_id)
        return await handle_remote_call(event2, state, manifest2, remote_id);
      if (route) {
        const method = event2.request.method;
        let response2;
        if (is_data_request)
          response2 = await render_data(event2, state, route, manifest2, invalidated_data_nodes, trailing_slash);
        else {
          let endpoint;
          if (route.endpoint && (!route.page || !state.prerendering && is_endpoint_request(event2))) {
            endpoint = await route.endpoint();
            if (route.page && (method === "GET" || method === "HEAD" || method === "POST")) {
              if (!(method === "POST" ? !!(endpoint.POST || endpoint.fallback) : !!(endpoint.GET || endpoint.fallback || method === "HEAD" && endpoint.HEAD)))
                endpoint = undefined;
            }
          }
          if (endpoint)
            response2 = await render_endpoint(event2, state, endpoint);
          else if (route.page) {
            if (!page_nodes2)
              throw new Error("page_nodes not found. This should never happen");
            else if (page_methods.has(method))
              response2 = await render_page(event2, state, route.page, manifest2, page_nodes2, resolve_opts);
            else {
              const allowed_methods2 = new Set(allowed_page_methods);
              if ((await manifest2._.nodes[route.page.leaf]())?.server?.actions)
                allowed_methods2.add("POST");
              if (method === "OPTIONS")
                response2 = new Response(null, {
                  status: 204,
                  headers: { allow: Array.from(allowed_methods2.values()).join(", ") }
                });
              else {
                const mod = [...allowed_methods2].reduce((acc, curr) => {
                  acc[curr] = true;
                  return acc;
                }, {});
                response2 = method_not_allowed(mod, method);
              }
            }
          } else
            throw new Error("Route is neither page nor endpoint. This should never happen");
        }
        if ((request.method === "GET" || request.method === "HEAD") && route.page && route.endpoint) {
          const vary = response2.headers.get("vary")?.split(",")?.map((v) => v.trim().toLowerCase());
          if (!(vary?.includes("accept") || vary?.includes("*"))) {
            response2 = new Response(response2.body, {
              status: response2.status,
              statusText: response2.statusText,
              headers: new Headers(response2.headers)
            });
            response2.headers.append("Vary", "Accept");
          }
        }
        return response2;
      }
      if (state.error && event2.isSubRequest) {
        const headers3 = new Headers(request.headers);
        headers3.set("x-sveltekit-error", "true");
        return await fetch(request, { headers: headers3 });
      }
      if (state.error)
        return text("Internal Server Error", { status: 500 });
      if (state.depth === 0) {
        if (!state.prerendering && is_data_request && invalidated_data_nodes?.length === 1 && invalidated_data_nodes[0])
          return await render_data(event2, state, { page: {
            layouts: [],
            leaf: 0
          } }, manifest2, invalidated_data_nodes, "ignore");
        if (non_html_fetch_destinations.has(event2.request.headers.get("sec-fetch-dest") ?? ""))
          return text("Not Found", {
            status: 404,
            headers: { vary: "Sec-Fetch-Dest" }
          });
        return await respond_with_error({
          event: event2,
          state,
          manifest: manifest2,
          error: new SvelteKitError(404, "Not Found", `Not found: ${event2.url.pathname}`),
          resolve_opts
        });
      }
      if (state.prerendering)
        return text("not found", { status: 404 });
      const response = await fetch(request);
      return new Response(response.body, response);
    } catch (e) {
      return await handle_fatal_error(event2, state, e);
    } finally {
      event2.cookies.set = () => {
        throw new Error("Cannot use `cookies.set(...)` after the response has been generated");
      };
      event2.setHeaders = () => {
        throw new Error("Cannot use `setHeaders(...)` after the response has been generated");
      };
    }
  }
}
function load_page_nodes(page, manifest2) {
  return Promise.all([...page.layouts.map((n) => n == undefined ? n : manifest2._.nodes[n]()), manifest2._.nodes[page.leaf]()]);
}
function propagate_context(fn) {
  return async (req, ...rest) => {
    if (otel === null)
      return fn(req, ...rest);
    const { propagation, context } = await otel;
    const c = propagation.extract(context.active(), Object.fromEntries(req.headers));
    return context.with(c, async () => {
      return await fn(req, ...rest);
    });
  };
}
function denormalise_url({ request_url, resolved_path, is_data_request, is_route_resolution_request }) {
  const url2 = new URL(request_url);
  url2.pathname = is_data_request ? add_data_suffix(resolved_path) : is_route_resolution_request ? add_resolution_suffix(resolved_path) : resolved_path;
  return url2;
}
set_options(options);
var init_promise;
var current = null;
var Server = class {
  #manifest;
  constructor(manifest2) {
    this.#manifest = manifest2;
    if (IN_WEBCONTAINER) {
      const respond2 = this.respond.bind(this);
      this.respond = async (...args) => {
        const { promise, resolve: resolve2 } = Promise.withResolvers();
        const previous = current;
        current = promise;
        await previous;
        return respond2(...args).finally(resolve2);
      };
    }
    set_manifest(manifest2);
  }
  async init({ env, read }) {
    if (read) {
      const wrapped_read = (file) => {
        const result = read(file);
        if (result instanceof ReadableStream)
          return result;
        return stream_from_iterable(async function* () {
          const stream = await result;
          if (stream)
            yield* stream;
        }());
      };
      set_read_implementation(wrapped_read);
    }
    await (init_promise ??= (async () => {
      try {
        const module2 = await get_hooks();
        set_hooks({
          handle: module2.handle || (({ event, resolve: resolve2 }) => resolve2(event)),
          handleError: module2.handleError || (({ kind, error: error2, issues }) => {
            if (kind === "validation") {
              console.error("Remote function schema validation failed:", issues);
              return;
            }
            if (kind !== "unknown")
              return;
            let e = error2;
            while (e instanceof Error) {
              if (e.stack)
                console.error(e.stack);
              e = e.cause;
            }
            if (e)
              console.error(String(e));
          }),
          handleFetch: module2.handleFetch || (({ request, fetch: fetch2 }) => fetch2(request)),
          reroute: module2.reroute || noop
        });
        init_transport(module2.transport ?? {});
        if (module2.init)
          await module2.init();
      } catch (e) {
        throw e;
      }
    })());
  }
  async respond(request, options2) {
    const request_state = create_request_state(options2);
    const response = await respond(request, this.#manifest, request_state);
    if (request_state.rerouted_url)
      response.headers.set(REROUTED_URL_HEADER, request_state.rerouted_url);
    return response;
  }
};

// node_modules/@sveltejs/adapter-bun/src/env.js
import process from "process";
var expected = new Set([
  "SOCKET_PATH",
  "HOST",
  "PORT",
  "REUSE_PORT",
  "IPV6_ONLY",
  "CONNECTION_IDLE_TIMEOUT",
  "BODY_SIZE_LIMIT",
  "SHUTDOWN_TIMEOUT",
  "DEVELOPMENT",
  "XFF_DEPTH",
  "ADDRESS_HEADER",
  "PROTOCOL_HEADER",
  "HOST_HEADER",
  "PORT_HEADER"
]);
if (env_prefix) {
  for (const name in process.env) {
    if (name.startsWith(env_prefix) && !expected.has(name.slice(env_prefix.length))) {
      throw new Error(`You should change envPrefix (${env_prefix}) to avoid conflicts with existing environment variables \u2014 unexpectedly saw ${name}`);
    }
  }
}
function parsing_error(name, value, expected2) {
  throw new Error(`Invalid value for environment variable ${env_prefix + name}: ${JSON.stringify(value)} (expected ${expected2})`);
}
function env(name, fallback) {
  const prefixed = env_prefix + name;
  return prefixed in process.env ? process.env[prefixed] : fallback;
}
var BOOLEANS = {
  1: true,
  true: true,
  yes: true,
  on: true,
  0: false,
  false: false,
  no: false,
  off: false
};
function boolean_env(name, fallback) {
  const value = env(name);
  if (value === undefined)
    return fallback;
  return BOOLEANS[value.toLowerCase()] ?? parsing_error(name, value, "a boolean");
}
function number_env(name, fallback, limits = {}) {
  const value = env(name);
  if (value === undefined)
    return fallback;
  if (!/^\d+$/.test(value)) {
    parsing_error(name, value, "a non-negative integer");
  }
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < (limits.min ?? 0) || number > (limits.max ?? Infinity)) {
    const range = limits.max === undefined ? `at least ${limits.min ?? 0}` : `between ${limits.min ?? 0} and ${limits.max}`;
    parsing_error(name, value, `an integer ${range}`);
  }
  return number;
}
function bytes_env(name, fallback) {
  const value = env(name);
  if (value === undefined)
    return fallback;
  if (value === "Infinity")
    return Infinity;
  if (!/^(?:\d+(?:\.\d*)?|\.\d+)(?:[KMG])?$/i.test(value)) {
    parsing_error(name, value, "a non-negative number with an optional K, M, or G suffix, or Infinity");
  }
  const suffix = value.at(-1)?.toUpperCase();
  const multiplier = {
    K: 1024,
    M: 1024 * 1024,
    G: 1024 * 1024 * 1024
  }[suffix] ?? 1;
  const number = Number(multiplier === 1 ? value : value.slice(0, -1)) * multiplier;
  if (!Number.isSafeInteger(number)) {
    parsing_error(name, value, "a non-negative number of whole bytes");
  }
  return number;
}

// node_modules/@sveltejs/adapter-bun/src/handler.js
var server = new Server(manifest);
var address_header = env("ADDRESS_HEADER", "").toLowerCase();
var protocol_header = env("PROTOCOL_HEADER", "").toLowerCase();
var host_header = env("HOST_HEADER", "").toLowerCase();
var port_header = env("PORT_HEADER", "").toLowerCase();
var xff_depth = number_env("XFF_DEPTH", 1, { min: 1 });
await server.init({
  env: Bun.env,
  read: (file) => server_assets.get(file)?.stream() ?? null
});
async function handler(request, bun_server) {
  const normalized_request = normalize_request(request);
  if (normalized_request instanceof Response)
    return normalized_request;
  const response = await server.respond(normalized_request, {
    platform: { server: bun_server },
    getClientAddress: () => get_client_address(request, bun_server)
  });
  if (response.headers.get("content-type")?.startsWith("text/event-stream")) {
    bun_server.timeout(request, 0);
    response.headers.set("x-accel-buffering", "no");
  }
  return response;
}
function normalize_request(request) {
  try {
    const url2 = new URL(request.url);
    const request_origin = origin || get_origin(request, url2);
    return request_origin === url2.origin ? request : new Request(request_origin + url2.pathname + url2.search, request);
  } catch (error2) {
    console.error(`Could not determine request origin: ${error2 instanceof Error ? error2.message : String(error2)}`);
    return new Response("Bad Request", { status: 400 });
  }
}
function get_origin(request, url2) {
  const protocol = decodeURIComponent(protocol_header && request.headers.get(protocol_header) || "https");
  if (!/^https?$/i.test(protocol)) {
    throw new Error(`The ${protocol_header} header specified ${protocol} which is an invalid protocol scheme. It should only contain the protocol scheme (e.g. \`https\`)`);
  }
  const host = host_header && request.headers.get(host_header) || (request.headers.get("host") ?? url2.host);
  if (!host) {
    throw new Error(`Could not determine host from the ${host_header ? `${host_header} or ` : ""}host header`);
  }
  const port = port_header ? request.headers.get(port_header) : null;
  if (port && isNaN(+port)) {
    throw new Error(`The ${port_header} header specified ${port} which is an invalid port because it is not a number. The value should only contain the port number (e.g. 443)`);
  }
  return new URL(`${protocol}://${host}${port ? `:${port}` : ""}`).origin;
}
function get_client_address(request, bun_server) {
  if (!address_header) {
    return bun_server.requestIP(request)?.address;
  }
  const value = request.headers.get(address_header);
  if (value === null) {
    throw new Error(`Address header was specified with ${env_prefix}ADDRESS_HEADER=${address_header} but is absent from request`);
  }
  if (address_header !== "x-forwarded-for")
    return value;
  const addresses = value.split(",");
  if (xff_depth > addresses.length) {
    throw new Error(`${env_prefix}XFF_DEPTH is ${xff_depth}, but only found ${addresses.length} addresses`);
  }
  return addresses[addresses.length - xff_depth].trim();
}

// node_modules/@sveltejs/adapter-bun/src/index.js
var options2 = { ...options_default };
var unix = env("SOCKET_PATH", options2.unix);
if (unix) {
  options2.unix = unix;
  delete options2.hostname;
  delete options2.port;
  delete options2.reusePort;
  delete options2.ipv6Only;
  try {
    if (fs2.statSync(unix).size === 0)
      fs2.rmSync(unix);
  } catch {}
} else {
  delete options2.unix;
  options2.hostname = env("HOST", options2.hostname);
  options2.port = env("PORT", options2.port?.toString()) ?? 3000;
  options2.reusePort = boolean_env("REUSE_PORT", options2.reusePort);
  options2.ipv6Only = boolean_env("IPV6_ONLY", options2.ipv6Only);
}
options2.idleTimeout = number_env("CONNECTION_IDLE_TIMEOUT", options2.idleTimeout, { max: 255 });
options2.development = boolean_env("DEVELOPMENT") ?? options2.development ?? false;
options2.maxRequestBodySize = bytes_env("BODY_SIZE_LIMIT", options2.maxRequestBodySize ?? 512 * 1024);
var shutdown_timeout = number_env("SHUTDOWN_TIMEOUT", 30);
options2.fetch = handler;
options2.routes = routes;
var server2 = Bun.serve(options2);
console.log(unix ? `Listening on ${unix}` : `Listening on ${server2.url}`);
var shutting_down = false;
async function graceful_shutdown(reason) {
  if (shutting_down)
    return process2.exit(1);
  shutting_down = true;
  if (server2.pendingRequests !== 0) {
    console.log(`Waiting for ${server2.pendingRequests} requests to finish before shutting down...
` + "Press Ctrl+C again to force shutdown.");
  }
  let deadline;
  const drained = await Promise.race([
    server2.stop().then(() => true),
    new Promise((resolve2) => {
      deadline = setTimeout(() => resolve2(false), shutdown_timeout * 1000);
    })
  ]);
  clearTimeout(deadline);
  if (!drained) {
    let grace;
    await Promise.race([
      server2.stop(true),
      new Promise((resolve2) => {
        grace = setTimeout(resolve2, 1000);
      })
    ]);
    clearTimeout(grace);
  }
  process2.emit("sveltekit:shutdown", reason);
}
process2.on("SIGTERM", () => graceful_shutdown("SIGTERM"));
process2.on("SIGINT", () => graceful_shutdown("SIGINT"));

//# debugId=51FC06702D65A63264756E2164756E21
