// @bun
import {
  __require
} from "./index-qcep1gwj.js";

// node_modules/@sveltejs/kit/src/exports/internal/shared.js
class HttpError {
  constructor(error) {
    this.status = error.status;
    this.body = error;
  }
  toString() {
    return JSON.stringify(this.body);
  }
}

class HandledHttpError extends HttpError {
}

class Redirect {
  constructor(status, location) {
    try {
      new Headers({ location });
    } catch {
      throw new Error(`Invalid redirect location ${JSON.stringify(location)}: ` + "this string contains characters that cannot be used in HTTP headers");
    }
    this.status = status;
    this.location = location;
  }
}

class SvelteKitError extends Error {
  constructor(status, text, message) {
    super(message);
    this.status = status;
    this.text = text;
  }
}

class ActionFailure {
  constructor(status, data) {
    this.status = status;
    this.data = data;
  }
}

class ValidationError extends Error {
  constructor(issues) {
    super("Validation failed");
    this.name = "ValidationError";
    this.issues = issues;
  }
}

// node_modules/esm-env/false.js
var false_default = false;
// node_modules/esm-env/true.js
var true_default = true;
// node_modules/@sveltejs/kit/src/constants.js
var MUTATIVE_METHODS = ["POST", "PUT", "PATCH", "DELETE"];
var BODY_DEPENDENT_METHODS = [...MUTATIVE_METHODS, "QUERY"];
var IN_WEBCONTAINER = !!globalThis.process?.versions?.webcontainer;

// node_modules/@sveltejs/kit/src/exports/internal/server/event.js
var sync_store = null;
var als;
import("async_hooks").then((hooks) => als = new hooks.AsyncLocalStorage).catch(() => {});
function try_get_request_store() {
  return sync_store ?? als?.getStore() ?? null;
}
function with_request_store(store, fn) {
  try {
    sync_store = store;
    return als ? als.run(store, fn) : fn();
  } finally {
    if (!IN_WEBCONTAINER) {
      sync_store = null;
    }
  }
}
// node_modules/@sveltejs/kit/src/telemetry.js
var noop_span = {
  spanContext() {
    return noop_span_context;
  },
  setAttribute() {
    return this;
  },
  setAttributes() {
    return this;
  },
  addEvent() {
    return this;
  },
  setStatus() {
    return this;
  },
  updateName() {
    return this;
  },
  end() {
    return this;
  },
  isRecording() {
    return false;
  },
  recordException() {
    return this;
  },
  addLink() {
    return this;
  },
  addLinks() {
    return this;
  }
};
var noop_span_context = {
  traceId: "",
  spanId: "",
  traceFlags: 0
};

// node_modules/@sveltejs/kit/src/exports/internal/server/telemetry.js
var otel = null;
async function record_span({ name, attributes, fn }) {
  if (otel === null) {
    return fn(noop_span);
  }
  const { SpanStatusCode, tracer } = await otel;
  return tracer.startActiveSpan(name, { attributes }, async (span) => {
    try {
      return await fn(span);
    } catch (error) {
      if (error instanceof HttpError) {
        span.setAttributes({
          [`${name}.result.type`]: "known_error",
          [`${name}.result.status`]: error.status,
          [`${name}.result.message`]: error.body.message
        });
        if (error.status >= 500) {
          span.recordException({
            name: "HttpError",
            message: error.body.message
          });
          span.setStatus({
            code: SpanStatusCode.ERROR,
            message: error.body.message
          });
        }
      } else if (error instanceof Redirect) {
        span.setAttributes({
          [`${name}.result.type`]: "redirect",
          [`${name}.result.status`]: error.status,
          [`${name}.result.location`]: error.location
        });
      } else if (error instanceof Error) {
        span.setAttributes({
          [`${name}.result.type`]: "unknown_error"
        });
        span.recordException({
          name: error.name,
          message: error.message,
          ...error.stack !== undefined && { stack: error.stack }
        });
        span.setStatus({
          code: SpanStatusCode.ERROR,
          message: error.message
        });
      } else {
        span.setAttributes({
          [`${name}.result.type`]: "unknown_error"
        });
        span.setStatus({ code: SpanStatusCode.ERROR });
      }
      throw error;
    } finally {
      span.end();
    }
  });
}

// node_modules/@sveltejs/kit/src/exports/internal/server/index.js
function get_origin() {
  const request = try_get_request_store()?.event.request;
  return request && new URL(request.url).origin;
}
function merge_tracing(event_like, current) {
  return {
    ...event_like,
    tracing: {
      ...event_like.tracing,
      current
    }
  };
}

// node_modules/@sveltejs/kit/src/utils/url.js
var internal = new URL("a://");
function matches_external_allowlist_entry(location, allowed) {
  if (location === allowed)
    return true;
  try {
    const allow = new URL(allowed);
    const loc = new URL(location, allow);
    return loc.protocol === allow.protocol && loc.host === allow.host;
  } catch {
    return false;
  }
}

// node_modules/@sveltejs/kit/src/exports/url.js
var REDIRECT_BASE = "https://sveltekit-redirect.invalid";
function is_external_location(location) {
  const origin = get_origin();
  try {
    return !matches_external_allowlist_entry(location, origin ?? REDIRECT_BASE);
  } catch {
    return true;
  }
}
var javascript_protocols = new Set(["javascript:", "data:"]);
function is_javascript_location(location) {
  try {
    return javascript_protocols.has(new URL(location, REDIRECT_BASE).protocol);
  } catch {
    return false;
  }
}
function validate_redirect_location(location, options) {
  if (!is_external_location(location))
    return;
  const external = options?.external;
  if (!external) {
    throw new Error(true_default ? `Cannot redirect to external URL ${JSON.stringify(location)}. ` + "To redirect to an external URL, pass `{ external: true }` or an allowlist of permitted origins as the third argument to `redirect`" : "Cannot redirect to external URL unless explicitly allowed");
  }
  if (external === true) {
    if (is_javascript_location(location)) {
      throw new Error(true_default ? `Cannot redirect to ${JSON.stringify(location)} with \`{ external: true }\`. ` + "The `javascript:` and `data:` protocols must be explicitly listed in the `external` allowlist" : "Cannot redirect to external URL unless explicitly allowed");
    }
    return;
  }
  if (Array.isArray(external)) {
    if (!external.some((allowed) => matches_external_allowlist_entry(location, allowed))) {
      throw new Error(true_default ? `Cannot redirect to ${JSON.stringify(location)}: URL origin is not included in the \`external\` allowlist` : "Cannot redirect to external URL unless explicitly allowed");
    }
    return;
  }
  throw new Error(true_default ? "`redirect` options.external must be `true` or an array of allowed origins" : "Invalid redirect options.external value");
}

// node_modules/@sveltejs/kit/src/exports/index.js
var text_encoder = new TextEncoder;
function error(status, message, properties) {
  if ((!false_default || true_default) && (isNaN(status) || status < 400 || status > 599)) {
    throw new Error(`HTTP error status codes must be between 400 and 599 \u2014 ${status} is invalid`);
  }
  if (message !== undefined && typeof message !== "string") {
    if (true_default) {
      console.warn("Passing an `App.Error` body as the second argument is deprecated \u2014 pass the `message` as the second argument, and any additional properties as the third");
    }
    ({ message, ...properties } = message);
  }
  throw new HttpError({ ...properties, status, message: message ?? `Error: ${status}` });
}
function redirect(status, location, options) {
  if ((!false_default || true_default) && (isNaN(status) || status < 300 || status > 308)) {
    throw new Error("Invalid status code");
  }
  const href = location.toString();
  validate_redirect_location(href, options);
  throw new Redirect(status, href);
}
function isRedirect(e) {
  return e instanceof Redirect;
}
function json(data, init) {
  const body = JSON.stringify(data);
  const headers = new Headers(init?.headers);
  if (!headers.has("content-length")) {
    headers.set("content-length", text_encoder.encode(body).byteLength.toString());
  }
  if (!headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  return new Response(body, {
    ...init,
    headers
  });
}
function text(body, init) {
  const headers = new Headers(init?.headers);
  if (!headers.has("content-length")) {
    const encoded = text_encoder.encode(body);
    headers.set("content-length", encoded.byteLength.toString());
    return new Response(encoded, {
      ...init,
      headers
    });
  }
  return new Response(body, {
    ...init,
    headers
  });
}

export { HttpError, HandledHttpError, Redirect, SvelteKitError, ActionFailure, ValidationError, with_request_store, otel, record_span, merge_tracing, error, redirect, isRedirect, json, text };

//# debugId=16800C81C6E4797B64756E2164756E21
