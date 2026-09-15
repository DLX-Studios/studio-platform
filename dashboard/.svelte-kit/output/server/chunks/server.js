import { n as noop } from "./functions.js";
import { error, text } from "@sveltejs/kit";
import { ActionFailure, HandledHttpError, HttpError, Redirect, SvelteKitError, ValidationError } from "@sveltejs/kit/internal";
import { merge_tracing, record_span, with_request_store } from "@sveltejs/kit/internal/server";
import * as devalue from "devalue";
//#region node_modules/@sveltejs/kit/src/runtime/utils.js
var text_encoder = new TextEncoder();
/**
* `ReadableStream.from`, for runtimes that don't support it (as of writing, every Bun release)
* @template T
* @param {AsyncIterable<T>} iterable
* @returns {ReadableStream<T>}
*/
function stream_from_iterable(iterable) {
	if (ReadableStream.from) return ReadableStream.from(iterable);
	const iterator = iterable[Symbol.asyncIterator]();
	return new ReadableStream({
		async pull(controller) {
			const { value, done } = await iterator.next();
			if (done) controller.close();
			else controller.enqueue(value);
		},
		async cancel(reason) {
			await iterator.return?.(reason);
		}
	});
}
/**
* @param {string} head
* @param {AsyncIterable<string>} chunks
* @returns {ReadableStream<Uint8Array>} `head` followed by each non-empty chunk, encoded
*/
function stream_text(head, chunks) {
	return stream_from_iterable((async function* () {
		yield text_encoder.encode(head);
		for await (const chunk of chunks) if (chunk) yield text_encoder.encode(chunk);
	})());
}
var text_decoder = new TextDecoder();
/**
* Like node's path.relative, but without using node
* @param {string} from
* @param {string} to
*/
function get_relative_path(from, to) {
	const from_parts = from.split(/[/\\]/);
	const to_parts = to.split(/[/\\]/);
	from_parts.pop();
	while (from_parts[0] === to_parts[0]) {
		from_parts.shift();
		to_parts.shift();
	}
	let i = from_parts.length;
	while (i--) from_parts[i] = "..";
	return from_parts.concat(to_parts).join("/");
}
/**
* @param {Uint8Array} bytes
* @returns {string}
*/
function base64_encode(bytes) {
	if (globalThis.Buffer) return globalThis.Buffer.from(bytes).toString("base64");
	let binary = "";
	for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
	return btoa(binary);
}
/**
* @param {string} encoded
* @returns {Uint8Array}
*/
function base64_decode(encoded) {
	if (globalThis.Buffer) {
		const buffer = globalThis.Buffer.from(encoded, "base64");
		return new Uint8Array(buffer);
	}
	const binary = atob(encoded);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/constants.js
/**
* A fake asset path used in `vite dev` and `vite preview`, so that we can
* serve local assets while verifying that requests are correctly prefixed
*/
var SVELTE_KIT_ASSETS = "/_svelte_kit_assets";
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
/** methods whose responses depend on the request body, so they can never be prerendered */
var BODY_DEPENDENT_METHODS = [...MUTATIVE_METHODS, "QUERY"];
var PAGE_METHODS = [
	"GET",
	"POST",
	"HEAD"
];
import.meta.dirname;
var IN_WEBCONTAINER = !!globalThis.process?.versions?.webcontainer;
/**
* If an an adapter deploys a catch-all serverless function, the rerouted URL
* is stored in this header.
*/
var REROUTED_URL_HEADER = "x-sveltekit-rerouted-url";
var assets = "";
var app_dir = "_app";
/** @param {string} path */
function set_assets(path) {
	assets = path;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/utils/routing.js
var escape_sequence_pattern = /\[([ux])\+([^\]]+)\]/;
/**
* @param {ParamMatcher} matcher
* @param {string} value
* @returns {{ success: true, value: any } | { success: false }}
*/
function run_matcher(matcher, value) {
	const result = matcher["~standard"].validate(value);
	if (result instanceof Promise) throw new Error("Async param matchers are not supported");
	if (result.issues) return { success: false };
	const parsed = result.value;
	if (typeof parsed !== "string" && typeof parsed !== "number" && typeof parsed !== "boolean" && typeof parsed !== "bigint") throw new Error("Param matcher must return a string, number, boolean, or bigint");
	return {
		success: true,
		value: parsed
	};
}
/**
* @param {RegExpMatchArray} match
* @param {import('types').RouteParam[]} params
* @param {Record<string, ParamMatcher>} matchers
*/
function exec(match, params, matchers) {
	/** @type {Record<string, any>} */
	const result = {};
	const values = match.slice(1);
	const values_needing_match = values.filter((value) => value !== void 0);
	let buffered = 0;
	for (let i = 0; i < params.length; i += 1) {
		const param = params[i];
		let value = values[i - buffered];
		if (param.chained && param.rest && buffered) {
			value = values.slice(i - buffered, i + 1).filter((s) => s).join("/");
			buffered = 0;
		}
		if (value === void 0) {
			if (param.rest) value = "";
			else continue;
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
		} else result[param.name] = decoded;
		const next_param = params[i + 1];
		const next_value = values[i + 1];
		if (next_param && !next_param.rest && next_param.optional && next_value && param.chained) buffered = 0;
		if (!next_param && !next_value && Object.keys(result).length === values_needing_match.length) buffered = 0;
	}
	if (buffered) return;
	return result;
}
new RegExp(`${escape_sequence_pattern.source}|${/\[(\[)?(\.\.\.)?([\w-]+?)(?:=([\w-]+))?\]\]?/g.source}`, "g");
/**
* Find the first route that matches the given path
* @template {{pattern: RegExp, params: import('types').RouteParam[]}} Route
* @param {string} path - The decoded pathname to match
* @param {Route[]} routes
* @param {Record<string, ParamMatcher>} matchers
* @returns {{ route: Route, params: Record<string, any> } | null}
*/
function find_route(path, routes, matchers) {
	for (const route of routes) {
		const match = route.pattern.exec(path);
		if (!match) continue;
		const matched = exec(match, route.params, matchers);
		if (matched) return {
			route,
			params: matched
		};
	}
	return null;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/utils/url.js
/**
* Matches a URI scheme. See https://www.rfc-editor.org/rfc/rfc3986#section-3.1
* @type {RegExp}
*/
var SCHEME = /^[a-z][a-z\d+\-.]*:/i;
var internal = new URL("a://");
/**
* @param {string} base
* @param {string} path
*/
function resolve(base, path) {
	if (path[0] === "/" && path[1] === "/") return path;
	let url = new URL(base, internal);
	url = new URL(path, url);
	return url.protocol === internal.protocol ? url.pathname + url.search + url.hash : url.href;
}
/**
* Relative reference from `from` to `to`, which must differ only by a trailing slash
* @param {string} from
* @param {string} to
* @returns {string}
*/
function relative_pathname(from, to) {
	const segment = to.replace(/\/$/, "").split("/").at(-1);
	return from.endsWith("/") ? `../${segment}` : `${segment}/`;
}
/**
* @param {string} path
* @param {import('types').TrailingSlash} trailing_slash
*/
function normalize_path(path, trailing_slash) {
	if (path === "/" || trailing_slash === "ignore") return path;
	if (trailing_slash === "never") return path.endsWith("/") ? path.slice(0, -1) : path;
	else if (trailing_slash === "always" && !path.endsWith("/")) return path + "/";
	return path;
}
/**
* Decode pathname excluding %25 to prevent further double decoding of params
* @param {string} pathname
*/
function decode_pathname(pathname) {
	return pathname.split("%25").map(decodeURI).join("%25");
}
/**
* @param {URL} url
* @param {() => void} callback
* @param {(search_param: string) => void} search_params_callback
* @param {boolean} [allow_hash]
*/
function make_trackable(url, callback, search_params_callback, allow_hash = false) {
	const tracked = new URL(url);
	Object.defineProperty(tracked, "searchParams", {
		value: new Proxy(tracked.searchParams, { get(obj, key) {
			if (key === "get" || key === "getAll" || key === "has") return (param, ...rest) => {
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
	/**
	* URL properties that could change during the lifetime of the page,
	* which excludes things like `origin`
	* @type {(keyof URL)[]}
	*/
	const tracked_url_properties = [
		"href",
		"pathname",
		"search",
		"toString",
		"toJSON"
	];
	if (allow_hash) tracked_url_properties.push("hash");
	for (const property of tracked_url_properties) Object.defineProperty(tracked, property, {
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
	if (!allow_hash) disable_hash(tracked);
	return tracked;
}
/**
* Disallow access to `url.hash` on the server and in `load`
* @param {URL} url
*/
function disable_hash(url) {
	allow_nodejs_console_log(url);
	Object.defineProperty(url, "hash", { get() {
		throw new Error("Cannot access event.url.hash. Consider using `page.url.hash` inside a component instead");
	} });
}
/**
* Disallow access to `url.search` and `url.searchParams` during prerendering
* @param {URL} url
*/
function disable_search(url) {
	allow_nodejs_console_log(url);
	for (const property of ["search", "searchParams"]) Object.defineProperty(url, property, { get() {
		throw new Error(`Cannot access url.${property} on a page with prerendering enabled`);
	} });
}
/**
* Allow URL to be console logged, bypassing disabled properties.
* @param {URL} url
*/
function allow_nodejs_console_log(url) {
	url[Symbol.for("nodejs.util.inspect.custom")] = (_depth, opts, inspect) => {
		return inspect(new URL(url), opts);
	};
}
//#endregion
//#region node_modules/@sveltejs/kit/src/pathname.js
var DATA_SUFFIX = "/__data.json";
var HTML_DATA_SUFFIX = ".html__data.json";
/** @param {string} pathname */
function has_data_suffix(pathname) {
	return pathname.endsWith(DATA_SUFFIX) || pathname.endsWith(HTML_DATA_SUFFIX);
}
/** @param {string} pathname */
function add_data_suffix(pathname) {
	if (pathname.endsWith(".html")) return pathname.replace(/\.html$/, HTML_DATA_SUFFIX);
	return pathname.replace(/\/$/, "") + DATA_SUFFIX;
}
/** @param {string} pathname */
function strip_data_suffix(pathname) {
	if (pathname.endsWith(HTML_DATA_SUFFIX)) return pathname.slice(0, -16) + ".html";
	return pathname.slice(0, -12);
}
var ROUTE_SUFFIX = "/__route.js";
var HTML_ROUTE_SUFFIX = ".html__route.js";
/**
* @param {string} pathname
* @returns {boolean}
*/
function has_resolution_suffix(pathname) {
	return pathname.endsWith(ROUTE_SUFFIX) || pathname.endsWith(HTML_ROUTE_SUFFIX);
}
/**
* Convert a regular URL to a route to send to SvelteKit's server-side route resolution endpoint
* @param {string} pathname
* @returns {string}
*/
function add_resolution_suffix(pathname) {
	if (pathname.endsWith(".html")) return pathname.replace(/\.html$/, HTML_ROUTE_SUFFIX);
	return pathname.replace(/\/$/, "") + ROUTE_SUFFIX;
}
/**
* @param {string} pathname
* @returns {string}
*/
function strip_resolution_suffix(pathname) {
	if (pathname.endsWith(HTML_ROUTE_SUFFIX)) return pathname.slice(0, -15) + ".html";
	return pathname.slice(0, -11);
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/dev.js
var symbol = Symbol.for("sveltekit.global_state");
globalThis[symbol] ??= {};
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/form-utils.js
/** @import { BinaryFormMeta, InternalRemoteFormIssue } from 'types' */
/** @import { StandardSchemaV1 } from '@standard-schema/spec' */
/**
* Sets a parsed form field value in a nested object, mutating the original object.
* @param {Record<string, any>} object
* @param {{ name: string; type: 'number' | 'boolean' | null }} field
* @param {any} value
*/
function set_nested_value(object, field, value) {
	deep_set(object, split_path(field.name), value);
}
/**
* Separates a form field's path from the metadata encoded in its name.
* @param {string} form_id
* @param {string} key
* @returns {{ name: string; type: 'number' | 'boolean' | null; is_array: boolean }}
*/
function parse_form_key(form_id, key) {
	const suffix = "/" + form_id;
	let name = key;
	if (!name.endsWith(suffix)) throw new Error(`Form contained a field that wasn't created with form.fields.as(...): ${name}`);
	name = name.slice(0, -suffix.length);
	/** @type {'number' | 'boolean' | null} */
	let type = null;
	if (name.startsWith("n:")) {
		name = name.slice(2);
		type = "number";
	} else if (name.startsWith("b:")) {
		name = name.slice(2);
		type = "boolean";
	}
	const is_array = name.endsWith("[]");
	if (is_array) name = name.slice(0, -2);
	return {
		name,
		type,
		is_array
	};
}
/**
* @param {'number' | 'boolean' | null} type
* @param {any} value
* @returns {any}
*/
function coerce_form_value(type, value) {
	if (Array.isArray(value)) return value.map((value) => coerce_form_value(type, value));
	if (type === "number") return value === "" ? void 0 : parseFloat(value);
	if (type === "boolean") return value === "on";
	return value;
}
/** Pass this to set_nested_value to delete the last part of the given path */
var DELETE_KEY = {};
/**
* Convert `FormData` into a POJO
* @param {string} form_id
* @param {FormData} data
*/
function convert_formdata(form_id, data) {
	/** @type {Record<string, any>} */
	const result = {};
	for (const field_name of data.keys()) {
		/** @type {any[]} */
		const values = data.getAll(field_name);
		const field = parse_form_key(form_id, field_name);
		const entries = values.filter((entry) => typeof entry === "string" || entry.name !== "" || entry.size > 0);
		if (entries.length === 0 && !field.is_array) continue;
		if (entries.length > 1 && !field.is_array) throw new Error(`Form cannot contain duplicated keys — "${field.name}" has ${entries.length} values`);
		set_nested_value(result, field, coerce_form_value(field.type, field.is_array ? entries : entries[0]));
	}
	return result;
}
var BINARY_FORM_CONTENT_TYPE = "application/x-sveltekit-formdata";
var BINARY_FORM_VERSION = 0;
var HEADER_BYTES = 7;
/**
* @param {Request} request
* @param {string} form_id
* @returns {Promise<{ data: Record<string, any>; meta: BinaryFormMeta; form_data: FormData | null }>}
*/
async function deserialize_binary_form(request, form_id) {
	if (request.headers.get("content-type") !== "application/x-sveltekit-formdata") {
		const form_data = await request.formData();
		return {
			data: convert_formdata(form_id, form_data),
			meta: {},
			form_data
		};
	}
	if (!request.body) throw deserialize_error("no body");
	const reader = request.body.getReader();
	/** @type {Array<Promise<Uint8Array<ArrayBuffer> | undefined>>} */
	const chunks = [];
	/**
	* @param {number} index
	* @returns {Promise<Uint8Array<ArrayBuffer> | undefined>}
	*/
	function get_chunk(index) {
		if (index in chunks) return chunks[index];
		let i = chunks.length;
		while (i <= index) {
			const previous = chunks[i - 1] ?? Promise.resolve(void 0);
			chunks[i] = previous.then(() => reader.read()).then((chunk) => chunk.value);
			i++;
		}
		return chunks[index];
	}
	/**
	* @param {number} offset
	* @param {number} length
	* @returns {Promise<Uint8Array | null>}
	*/
	async function get_buffer(offset, length) {
		/** @type {Uint8Array<ArrayBuffer>[]} */
		const parts = [];
		let total = 0;
		for await (const part of read_range(get_chunk, offset, length)) {
			parts.push(part);
			total += part.byteLength;
		}
		if (total < length || parts.length === 0) return null;
		if (parts.length === 1) return parts[0];
		const buffer = new Uint8Array(length);
		let cursor = 0;
		for (const part of parts) {
			buffer.set(part, cursor);
			cursor += part.byteLength;
		}
		return buffer;
	}
	const header = await get_buffer(0, HEADER_BYTES);
	if (!header) throw deserialize_error("too short");
	if (header[0] !== BINARY_FORM_VERSION) throw deserialize_error(`got version ${header[0]}, expected version ${BINARY_FORM_VERSION}`);
	const header_view = new DataView(header.buffer, header.byteOffset, header.byteLength);
	const data_length = header_view.getUint32(1, true);
	const file_offsets_length = header_view.getUint16(5, true);
	const data_buffer = await get_buffer(HEADER_BYTES, data_length);
	if (!data_buffer) throw deserialize_error("data too short");
	/** @type {Array<number | undefined>} */
	let file_offsets;
	/** @type {number} */
	let files_start_offset;
	if (file_offsets_length > 0) {
		const file_offsets_buffer = await get_buffer(HEADER_BYTES + data_length, file_offsets_length);
		if (!file_offsets_buffer) throw deserialize_error("file offset table too short");
		const parsed_offsets = JSON.parse(text_decoder.decode(file_offsets_buffer));
		if (!Array.isArray(parsed_offsets) || parsed_offsets.some((n) => typeof n !== "number" || !Number.isInteger(n) || n < 0)) throw deserialize_error("invalid file offset table");
		file_offsets = parsed_offsets;
		files_start_offset = HEADER_BYTES + data_length + file_offsets_length;
	}
	/** @type {Array<{ offset: number, size: number }>} */
	const file_spans = [];
	const [data, meta] = devalue.parse(text_decoder.decode(data_buffer), { File: ([name, type, size, last_modified, index]) => {
		if (typeof name !== "string" || typeof type !== "string" || typeof size !== "number" || typeof last_modified !== "number" || typeof index !== "number") throw deserialize_error("invalid file metadata");
		let offset = file_offsets[index];
		if (offset === void 0) throw deserialize_error("duplicate file offset table index");
		file_offsets[index] = void 0;
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
	for (let i = 1; i < file_spans.length; i++) {
		const previous = file_spans[i - 1];
		const current = file_spans[i];
		const previous_end = previous.offset + previous.size;
		if (previous_end < current.offset) throw deserialize_error("gaps in file data");
		if (previous_end > current.offset) throw deserialize_error("overlapping file data");
	}
	(async () => {
		let has_more = true;
		while (has_more) has_more = !!await get_chunk(chunks.length);
	})().catch(noop);
	return {
		data,
		meta,
		form_data: null
	};
}
/**
* @param {string} message
*/
function deserialize_error(message) {
	return new SvelteKitError(400, "Bad Request", `Could not deserialize binary form: ${message}`);
}
/**
* Yields the chunks that make up the byte range `[offset, offset + length)`,
* trimmed to its boundaries. Ends early if the underlying data runs out.
* @param {(index: number) => Promise<Uint8Array<ArrayBuffer> | undefined>} get_chunk
* @param {number} offset
* @param {number} length
* @returns {AsyncGenerator<Uint8Array<ArrayBuffer>, void, void>}
*/
async function* read_range(get_chunk, offset, length) {
	let chunk_start = 0;
	for (let index = 0;; index++) {
		const chunk = await get_chunk(index);
		if (!chunk) return;
		const chunk_end = chunk_start + chunk.byteLength;
		if (chunk_end > offset) {
			yield chunk.subarray(Math.max(0, offset - chunk_start), Math.min(chunk.byteLength, offset + length - chunk_start));
			if (offset + length <= chunk_end) return;
		}
		chunk_start = chunk_end;
	}
}
/** @implements {File} */
var LazyFile = class LazyFile {
	/** @type {(index: number) => Promise<Uint8Array<ArrayBuffer> | undefined>} */
	#get_chunk;
	/** @type {number} */
	#offset;
	/**
	* @param {string} name
	* @param {string} type
	* @param {number} size
	* @param {number} last_modified
	* @param {(index: number) => Promise<Uint8Array<ArrayBuffer> | undefined>} get_chunk
	* @param {number} offset
	*/
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
	/** @type {ArrayBuffer | undefined} */
	#buffer;
	async arrayBuffer() {
		this.#buffer ??= await new Response(this.stream()).arrayBuffer();
		return this.#buffer;
	}
	async bytes() {
		return new Uint8Array(await this.arrayBuffer());
	}
	/**
	* @param {number=} start
	* @param {number=} end
	* @param {string=} contentType
	*/
	slice(start = 0, end = this.size, contentType = this.type) {
		if (start < 0) start = Math.max(this.size + start, 0);
		else start = Math.min(start, this.size);
		if (end < 0) end = Math.max(this.size + end, 0);
		else end = Math.min(end, this.size);
		const size = Math.max(end - start, 0);
		return new LazyFile(this.name, contentType, size, this.lastModified, this.#get_chunk, this.#offset + start);
	}
	stream() {
		const range = read_range(this.#get_chunk, this.#offset, this.size);
		const size = this.size;
		return stream_from_iterable((async function* () {
			let cursor = 0;
			for await (const chunk of range) {
				cursor += chunk.byteLength;
				yield chunk;
			}
			if (cursor < size) throw new Error("incomplete file data");
		})());
	}
	async text() {
		return text_decoder.decode(await this.arrayBuffer());
	}
};
var path_regex = /^[a-zA-Z_$]\w*(\.[a-zA-Z_$]\w*|\[\d+\])*$/;
/**
* @param {string} path
*/
function split_path(path) {
	if (!path_regex.test(path)) throw new Error(`Invalid path ${path}`);
	return path.split(/\.|\[|\]/).filter(Boolean);
}
/**
* Check if a property key is dangerous and could lead to prototype pollution
* @param {string} key
*/
function check_prototype_pollution(key) {
	if (key === "__proto__" || key === "constructor" || key === "prototype") throw new Error(`Invalid key "${key}"`);
}
/**
* Sets a value in a nested object using an array of keys, mutating the original object.
* @param {Record<string, any>} object
* @param {string[]} keys
* @param {any} value
*/
function deep_set(object, keys, value) {
	let current = object;
	for (let i = 0; i < keys.length - 1; i += 1) {
		const key = keys[i];
		check_prototype_pollution(key);
		const is_array = /^\d+$/.test(keys[i + 1]);
		const inner = Object.hasOwn(current, key) ? current[key] : void 0;
		const exists = inner != null;
		if (exists && is_array !== Array.isArray(inner)) throw new Error(`Invalid array key ${keys[i + 1]}`);
		if (!exists) {
			if (value === DELETE_KEY) return;
			current[key] = is_array ? [] : {};
		}
		current = current[key];
	}
	const final_key = keys[keys.length - 1];
	check_prototype_pollution(final_key);
	if (value === DELETE_KEY) delete current[final_key];
	else current[final_key] = value;
}
/**
* @param {StandardSchemaV1.Issue} issue
* @param {boolean} server Whether this issue came from server validation
*/
function normalize_issue(issue, server = false) {
	/** @type {InternalRemoteFormIssue} */
	const normalized = {
		name: "",
		path: [],
		message: issue.message,
		server
	};
	if (issue.path !== void 0) {
		let name = "";
		for (const segment of issue.path) {
			const key = typeof segment === "object" ? segment.key : segment;
			normalized.path.push(key);
			if (typeof key === "number") name += `[${key}]`;
			else if (typeof key === "string") name += name === "" ? key : "." + key;
		}
		normalized.name = name;
	}
	return normalized;
}
/**
* @param {InternalRemoteFormIssue[]} issues
*/
function flatten_issues(issues) {
	/** @type {Record<string, InternalRemoteFormIssue[]>} */
	const result = {};
	for (const issue of issues) {
		(result.$ ??= []).push(issue);
		let name = "";
		if (issue.path !== void 0) for (const key of issue.path) {
			if (typeof key === "number") name += `[${key}]`;
			else if (typeof key === "string") name += name === "" ? key : "." + key;
			(result[name] ??= []).push(issue);
		}
	}
	return result;
}
/**
* Gets a nested value from an object using a path array
* @param {Record<string, any>} object
* @param {(string | number)[]} path
* @returns {any}
*/
function deep_get(object, path) {
	let current = object;
	for (const key of path) {
		if (current == null || typeof current !== "object") return current;
		current = current[key];
	}
	return current;
}
/**
*
* @param {string} field_type
* @param {boolean} is_array
* @param {unknown} input_value
*/
function get_type_prefix(field_type, is_array, input_value) {
	if (field_type === "number" || field_type === "range") return "n:";
	if (field_type === "checkbox" && !is_array) return "b:";
	if (field_type === "hidden" || field_type === "submit") {
		const input_type = typeof input_value;
		if (input_type === "number") return "n:";
		if (input_type === "boolean") return "b:";
	}
	return "";
}
/**
* A deep-clone implementation specifically for form data, where
* we don't need to worry about cycles and whatnot
* @param {any} value
* @returns {any}
*/
function deep_clone(value) {
	if (value !== null && typeof value === "object") {
		if (value instanceof Date) return new Date(value.getTime());
		if (value instanceof File) return value;
		if (Array.isArray(value)) return value.map(deep_clone);
		/** @type {Record<string, any>} */
		const clone = {};
		for (const key of Object.keys(value)) clone[key] = deep_clone(value[key]);
		return clone;
	}
	return value;
}
/**
* Creates a proxy-based field accessor for form data
* @param {{
* 	form_id: string,
* 	get: () => Record<string, any>,
* 	set: (path: (string | number)[], value: any) => void,
* 	get_issues: (path?: (string | number)[], all?: boolean) => Record<string, InternalRemoteFormIssue[]>,
* 	get_touched: () => Record<string, boolean>,
* 	get_dirty: () => Record<string, boolean>
* }} context - Form context, including value accessors and form metadata
* @param {any} target - Function or empty POJO
* @param {(string | number)[]} path - Current access path
* @returns {any} Proxy object with name(), value(), and issues() methods
*/
function create_field_proxy(context, target = {}, path = []) {
	const get_value = () => {
		return deep_clone(deep_get(context.get(), path));
	};
	return new Proxy(target, { get(target, prop) {
		if (typeof prop === "symbol") return target[prop];
		if (/^\d+$/.test(prop)) return create_field_proxy(context, {}, [...path, parseInt(prop, 10)]);
		const key = build_path_string(path);
		const next = [...path, prop];
		if (prop === "set") {
			const set_func = function(newValue) {
				context.set(path, newValue);
				return newValue;
			};
			return create_field_proxy(context, set_func, next);
		}
		if (prop === "value") return create_field_proxy(context, get_value, next);
		if (prop === "issues" || prop === "allIssues") {
			const issues_func = () => {
				const all_issues = context.get_issues(path, prop === "allIssues")[key === "" ? "$" : key];
				if (prop === "allIssues") return all_issues?.map((issue) => ({
					path: issue.path,
					message: issue.message
				}));
				const issues = all_issues?.filter((issue) => issue.name === key)?.map((issue) => ({
					path: issue.path,
					message: issue.message
				}));
				return issues?.length ? issues : void 0;
			};
			return create_field_proxy(context, issues_func, next);
		}
		if (prop === "touched" || prop === "dirty") {
			const fn = () => {
				const object = prop === "dirty" ? context.get_dirty() : context.get_touched();
				if (key === "") return Object.keys(object).length > 0;
				if (Object.hasOwn(object, key)) return true;
				for (const candidate in object) {
					if (!Object.hasOwn(object, candidate)) continue;
					if (!candidate.startsWith(key)) continue;
					const next = candidate[key.length];
					if (next === "." || next === "[") return true;
				}
				return false;
			};
			return create_field_proxy(context, fn, next);
		}
		if (prop === "as") {
			/**
			* @param {string} type
			* @param {unknown} [input_value]
			*/
			const as_func = (type, input_value) => {
				const is_array = type === "file multiple" || type === "select multiple" || type === "checkbox" && typeof input_value === "string";
				/** @type {Record<string, any>} */
				const base_props = {
					name: get_type_prefix(type, is_array, input_value) + key + (is_array ? "[]" : "") + "/" + context.form_id,
					get "aria-invalid"() {
						const issues = context.get_issues();
						return key in issues ? "true" : void 0;
					}
				};
				if (type !== "text" && type !== "select" && type !== "select multiple") base_props.type = type === "file multiple" ? "file" : type;
				if (type === "submit" || type === "hidden") return Object.defineProperties(base_props, { value: {
					value: typeof input_value === "boolean" ? input_value ? "on" : "off" : input_value,
					enumerable: true
				} });
				if (type === "select" || type === "select multiple") return Object.defineProperties(base_props, {
					multiple: {
						value: is_array,
						enumerable: true
					},
					value: {
						enumerable: true,
						get() {
							return get_value() ?? input_value;
						}
					}
				});
				if (type === "checkbox" || type === "radio") {
					if (type === "checkbox" && !is_array) return Object.defineProperties(base_props, {
						defaultChecked: {
							enumerable: true,
							get() {
								return input_value;
							}
						},
						checked: {
							enumerable: true,
							get() {
								return get_value() ?? input_value;
							}
						}
					});
					return Object.defineProperties(base_props, {
						value: {
							value: input_value ?? "on",
							enumerable: true
						},
						checked: {
							enumerable: true,
							get() {
								const value = get_value();
								if (type === "radio") return value === input_value;
								return (value ?? []).includes(input_value);
							}
						}
					});
				}
				if (type === "file" || type === "file multiple") return Object.defineProperties(base_props, {
					multiple: {
						value: is_array,
						enumerable: true
					},
					files: {
						enumerable: true,
						get() {
							const value = get_value();
							if (value instanceof File) {
								if (typeof DataTransfer !== "undefined") {
									const fileList = new DataTransfer();
									fileList.items.add(value);
									return fileList.files;
								}
								return {
									0: value,
									length: 1
								};
							}
							if (Array.isArray(value) && value.every((f) => f instanceof File)) {
								if (typeof DataTransfer !== "undefined") {
									const fileList = new DataTransfer();
									value.forEach((file) => fileList.items.add(file));
									return fileList.files;
								}
								/** @type {any} */
								const fileListLike = { length: value.length };
								value.forEach((file, index) => {
									fileListLike[index] = file;
								});
								return fileListLike;
							}
							return null;
						}
					}
				});
				return Object.defineProperties(base_props, {
					defaultValue: {
						enumerable: true,
						get() {
							return input_value;
						}
					},
					value: {
						enumerable: true,
						get() {
							const value = get_value() ?? input_value;
							return value != null ? String(value) : "";
						}
					}
				});
			};
			return create_field_proxy(context, as_func, next);
		}
		return create_field_proxy(context, {}, next);
	} });
}
/**
* Builds a path string from an array of path segments
* @param {(string | number)[]} path
* @returns {string}
*/
function build_path_string(path) {
	let result = "";
	for (const segment of path) if (typeof segment === "number") result += `[${segment}]`;
	else result += result === "" ? segment : "." + segment;
	return result;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/utils/http.js
/**
* Given an Accept header and a list of possible content types, pick
* the most suitable one to respond with
* @param {string} accept
* @param {string[]} types
*/
function negotiate(accept, types) {
	/** @type {Array<{ type: string, subtype: string, q: number, i: number }>} */
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
		if (a.q !== b.q) return b.q - a.q;
		if (a.subtype === "*" !== (b.subtype === "*")) return a.subtype === "*" ? 1 : -1;
		if (a.type === "*" !== (b.type === "*")) return a.type === "*" ? 1 : -1;
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
/**
* Returns `true` if a `content-type` header value is one of the given types, ignoring
* parameters such as `charset` and comparing case-insensitively
* @param {string | null | undefined} header
* @param  {...string} types
*/
function matches_content_type(header, ...types) {
	const type = header?.split(";", 1)[0].trim() ?? "";
	return types.includes(type.toLowerCase());
}
/**
* @param {Request} request
*/
function is_form_content_type(request) {
	return matches_content_type(request.headers.get("content-type"), "application/x-www-form-urlencoded", "multipart/form-data", "text/plain", BINARY_FORM_CONTENT_TYPE);
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/app/internal/transport.js
/** @import { Transport } from '@sveltejs/kit/hooks' */
/** @type {(thing: any) => string} */
var uneval = () => {
	throw new Error("");
};
/** @type {(data: any) => string} */
var stringify = () => {
	throw new Error("");
};
/** @type {(data: string) => any} */
var parse = () => {
	throw new Error("");
};
/** @type {Record<string, (data: any) => any>} */
var encoders = {};
/** @type {Record<string, (data: any) => any>} */
var decoders = {};
var has_custom_transporters = false;
/**
*
* @param {Transport} transport
*/
function init_transport(transport) {
	const transporters = Object.entries(transport);
	has_custom_transporters = transporters.length > 0;
	/** @param {unknown} thing */
	const replacer = (thing) => {
		for (const key of Object.keys(transport)) {
			const encoded = transport[key].encode(thing);
			if (encoded) return `app.decode('${key}', ${devalue.uneval(encoded, replacer)})`;
		}
	};
	encoders = Object.fromEntries(transporters.map(([k, v]) => [k, v.encode]));
	decoders = Object.fromEntries(transporters.map(([k, v]) => [k, v.decode]));
	uneval = (data) => devalue.uneval(data, replacer);
	stringify = (data) => devalue.stringify(data, encoders);
	parse = (data) => devalue.parse(data, decoders);
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/shared.js
/**
* @param {string} route_id
* @param {string} dep
*/
function validate_depends(route_id, dep) {
	const match = /^(moz-icon|view-source|jar):/.exec(dep);
	if (match) console.warn(`${route_id}: Calling \`depends('${dep}')\` will throw an error in Firefox because \`${match[1]}\` is a special URI scheme`);
}
var INVALIDATED_PARAM = "x-sveltekit-invalidated";
var TRAILING_SLASH_PARAM = "x-sveltekit-trailing-slash";
/**
* @param {any} data
* @param {string} [location_description]
*/
function validate_load_response(data, location_description) {
	if (data != null && Object.getPrototypeOf(data) !== Object.prototype) throw new Error(`a load function ${location_description} returned ${typeof data !== "object" ? `a ${typeof data}` : data instanceof Response ? "a Response object" : Array.isArray(data) ? "an array" : "a non-plain object"}, but must return a plain object at the top level (i.e. \`return {...}\`)`);
}
var object_proto_names = /* @__PURE__ */ Object.getOwnPropertyNames(Object.prototype).sort().join("\0");
/**
* @param {unknown} thing
* @returns {thing is Record<PropertyKey, unknown>}
*/
function is_plain_object(thing) {
	if (typeof thing !== "object" || thing === null) return false;
	const proto = Object.getPrototypeOf(thing);
	return proto === Object.prototype || proto === null || Object.getPrototypeOf(proto) === null || Object.getOwnPropertyNames(proto).sort().join("\0") === object_proto_names;
}
/**
* @param {Record<string, any>} value
* @param {Map<object, any>} clones
*/
function to_sorted(value, clones) {
	const clone = Object.getPrototypeOf(value) === null ? Object.create(null) : {};
	clones.set(value, clone);
	Object.defineProperty(clone, remote_arg_marker, { value: true });
	for (const key of Object.keys(value).sort()) {
		const property = value[key];
		Object.defineProperty(clone, key, {
			value: clones.get(property) ?? property,
			enumerable: true,
			configurable: true,
			writable: true
		});
	}
	return clone;
}
var remote_object = "__skrao";
var remote_map = "__skram";
var remote_set = "__skras";
var remote_file = "__skraf";
var remote_regex_guard = "__skrag";
var remote_arg_marker = Symbol(remote_object);
/**
* @param {boolean} sort
*/
function create_remote_arg_reducers(sort) {
	/** @type {Record<string, (value: unknown) => unknown>} */
	const remote_fns_reducers = { 
	/** @param {unknown} value */
[remote_regex_guard]: (value) => {
		if (value instanceof RegExp) throw new Error("Regular expressions are not valid remote function arguments");
	} };
	if (sort) {
		const clones = /* @__PURE__ */ new Map();
		/** @type {(value: unknown) => Array<[unknown, unknown]> | undefined} */
		remote_fns_reducers[remote_map] = (value) => {
			if (!(value instanceof Map)) return;
			/** @type {Array<[string, string]>} */
			const entries = [];
			for (const [key, val] of value) entries.push([stringify(key), stringify(val)]);
			return entries.sort(([a1, a2], [b1, b2]) => {
				if (a1 < b1) return -1;
				if (a1 > b1) return 1;
				if (a2 < b2) return -1;
				if (a2 > b2) return 1;
				return 0;
			});
		};
		/** @type {(value: unknown) => unknown[] | undefined} */
		remote_fns_reducers[remote_set] = (value) => {
			if (!(value instanceof Set)) return;
			/** @type {string[]} */
			const items = [];
			for (const item of value) items.push(stringify(item));
			items.sort();
			return items;
		};
		/** @type {(value: unknown) => Record<PropertyKey, unknown> | undefined} */
		remote_fns_reducers[remote_object] = (value) => {
			if (!is_plain_object(value)) return;
			if (Object.hasOwn(value, remote_arg_marker)) return;
			if (clones.has(value)) return clones.get(value);
			return to_sorted(value, clones);
		};
	}
	const all_reducers = {
		...encoders,
		...remote_fns_reducers
	};
	/** @type {(value: unknown) => string} */
	const stringify = (value) => devalue.stringify(value, all_reducers);
	return all_reducers;
}
function create_remote_arg_revivers() {
	const remote_fns_revivers = {
		/** @type {(value: unknown) => unknown} */
		[remote_object]: (value) => value,
		/** @type {(value: unknown) => Map<unknown, unknown>} */
		[remote_map]: (value) => {
			if (!Array.isArray(value)) throw new Error("Invalid data for Map reviver");
			const map = /* @__PURE__ */ new Map();
			for (const item of value) {
				if (!Array.isArray(item) || item.length !== 2 || typeof item[0] !== "string" || typeof item[1] !== "string") throw new Error("Invalid data for Map reviver");
				const [key, val] = item;
				map.set(parse(key), parse(val));
			}
			return map;
		},
		/** @type {(value: unknown) => Set<unknown>} */
		[remote_set]: (value) => {
			if (!Array.isArray(value)) throw new Error("Invalid data for Set reviver");
			const set = /* @__PURE__ */ new Set();
			for (const item of value) {
				if (typeof item !== "string") throw new Error("Invalid data for Set reviver");
				set.add(parse(item));
			}
			return set;
		},
		/** @type {(value: any) => File} */
		[remote_file]: (value) => {
			if (!value || typeof value !== "object" || typeof value.name !== "string" || typeof value.type !== "string" || typeof value.size !== "number" || typeof value.lastModified !== "number" || !(value.data instanceof ArrayBuffer)) throw new Error("Invalid data for File reviver");
			const { data, name, ...meta } = value;
			return new File([data], name, meta);
		}
	};
	const all_revivers = {
		...decoders,
		...remote_fns_revivers
	};
	/** @type {(data: string) => unknown} */
	const parse = (data) => devalue.parse(data, all_revivers);
	return all_revivers;
}
/**
* Stringifies the argument (if any) for a remote function in such a way that
* it is both a valid URL and a valid file name (necessary for prerendering).
* @param {any} value
*/
function stringify_remote_arg(value) {
	if (value === void 0) return "";
	return url_friendly_base64_encode(devalue.stringify(value, create_remote_arg_reducers(true)));
}
/**
* Base64-encodes `string` in such a way that the result is safe to use
* as both a URI component and a filename
* @param {string} string
*/
function url_friendly_base64_encode(string) {
	return base64_encode(text_encoder.encode(string)).replaceAll("=", "").replaceAll("+", "-").replaceAll("/", "_");
}
/**
* Parses the argument (if any) for a remote function
* @param {string} string
*/
function parse_remote_arg(string) {
	if (!string) return void 0;
	const json_string = text_decoder.decode(base64_decode(string.replaceAll("-", "+").replaceAll("_", "/")));
	return devalue.parse(json_string, create_remote_arg_revivers());
}
/**
* @param {string} id
* @param {string} payload
*/
function create_remote_key(id, payload) {
	return id + "/" + payload;
}
/**
* @param {string} key
* @returns {{ id: string; payload: string }}
*/
function split_remote_key(key) {
	const i = key.lastIndexOf("/");
	if (i === -1) throw new Error(`Invalid remote key: ${key}`);
	return {
		id: key.slice(0, i),
		payload: key.slice(i + 1)
	};
}
//#endregion
//#region node_modules/@sveltejs/kit/src/utils/error.js
/**
* @param {unknown} err
* @return {Error}
*/
function coalesce_to_error(err) {
	return err instanceof Error || err && err.name && err.message ? err : new Error(JSON.stringify(err));
}
/**
* This is an identity function that exists to make TypeScript less
* paranoid about people throwing things that aren't errors, which
* frankly is not something we should care about
* @param {unknown} error
*/
function normalize_error(error) {
	return error;
}
/**
* @param {unknown} error
*/
function get_status(error) {
	return error instanceof HttpError || error instanceof SvelteKitError ? error.status : 500;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/utils/escape.js
/**
* When inside a double-quoted attribute value, only `&` and `"` hold special meaning.
* @see https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state
* @type {Record<string, string>}
*/
var escape_html_attr_dict = {
	"&": "&amp;",
	"\"": "&quot;"
};
/**
* @type {Record<string, string>}
*/
var escape_html_dict = {
	"&": "&amp;",
	"<": "&lt;"
};
/** @param {Record<string, string>} dict */
var escape_regex = (dict) => new RegExp(`[${Object.keys(dict).join("")}]|\\p{Surrogate}`, "gu");
var escape_html_attr_regex = escape_regex(escape_html_attr_dict);
var escape_html_regex = escape_regex(escape_html_dict);
/**
* Escapes unpaired surrogates (which are allowed in js strings but invalid in HTML) and
* escapes characters that are special.
*
* @param {string} str
* @param {boolean} [is_attr]
* @returns {string} escaped string
* @example const html = `<tag data-value="${escape_html('value', true)}">...</tag>`;
*/
function escape_html(str, is_attr) {
	const dict = is_attr ? escape_html_attr_dict : escape_html_dict;
	return str.replace(is_attr ? escape_html_attr_regex : escape_html_regex, (match) => dict[match] ?? `&#${match.charCodeAt(0)};`);
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/errors.js
/**
* @param {import('@sveltejs/kit').RequestEvent} event
* @param {import('types').RequestState} state
* @param {unknown} error
*/
async function handle_fatal_error(event, state, error) {
	const body = await handle_error_and_jsonify(event, state, error);
	const status = body.status;
	const type = negotiate(event.request.headers.get("accept") || "text/html", ["application/json", "text/html"]);
	if (event.isDataRequest || type === "application/json") return Response.json(body, { status });
	return static_error_page(status, body.message);
}
/**
* @param {import('@sveltejs/kit').RequestEvent} event
* @param {import('types').RequestState} state
* @param {any} error
* @returns {App.Error | Promise<App.Error>}
*/
function handle_error_and_jsonify(event, state, error) {
	if (error instanceof HandledHttpError) return error.body;
	/** @type {import('@sveltejs/kit/hooks').CaughtError} */
	let caught;
	if (error instanceof HttpError) caught = {
		kind: "app",
		error: error.body
	};
	else if (error instanceof SvelteKitError) caught = {
		kind: "framework",
		error: {
			status: error.status,
			message: error.text
		}
	};
	else if (error instanceof ValidationError) caught = {
		kind: "validation",
		error: {
			status: 400,
			message: "Bad Request"
		},
		issues: error.issues
	};
	else {
		caught = {
			kind: "unknown",
			error
		};
		let e = error;
		while (e instanceof Error) {
			fix_stack_trace(e);
			e = e.cause;
		}
	}
	const fallback = caught.kind === "unknown" ? {
		status: 500,
		message: "Internal Error"
	} : caught.error;
	/**
	* The hook returns only the properties it wants to override; anything it omits
	* (including by returning nothing at all) is inherited from the caught error.
	* @param {Awaited<ReturnType<import('@sveltejs/kit/hooks').HandleServerError>>} body
	* @returns {App.Error}
	*/
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
		log_handle_error_hook_failure(error, hook_error);
		return {
			status: fallback.status,
			message: "Internal Error"
		};
	}
	if (result instanceof Promise) {
		if (state.is_in_render) {
			console.warn(`To use an async \`handleError\` hook to handle errors that occur during rendering, you must enable \`compilerOptions.experimental.async\` in the SvelteKit plugin of your Vite config. The returned error has been replaced with a generic object`);
			result.catch((hook_error) => log_handle_error_hook_failure(error, hook_error));
			return {
				status: fallback.status,
				message: "Internal Error"
			};
		}
		return result.then(merge, (hook_error) => {
			log_handle_error_hook_failure(error, hook_error);
			return {
				status: fallback.status,
				message: "Internal Error"
			};
		});
	}
	return merge(result);
}
/**
* @param {unknown} error
* @param {unknown} hook_error
*/
function log_handle_error_hook_failure(error, hook_error) {
	const failure = new Error("The `handleError` hook failed", { cause: coalesce_to_error(hook_error) });
	failure.stack = failure.message;
	console.error(failure);
	if (error instanceof SvelteKitError) console.error(`Original error: ${error.status} ${error.text}: ${error.message}`);
	else console.error("Original error:", error);
}
/**
* Return as a response that renders the error.html
*
* @param {number} status
* @param {string} message
*/
function static_error_page(status, message) {
	let page = options$1.templates.error({
		status,
		message: escape_html(message)
	});
	return text(page, {
		headers: { "content-type": "text/html; charset=utf-8" },
		status
	});
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/utils.js
/**
* @param {Partial<Record<import('types').HttpMethod, any>>} mod
* @param {import('types').HttpMethod} method
*/
function method_not_allowed(mod, method) {
	return text(`${method} method not allowed`, {
		status: 405,
		headers: { allow: allowed_methods(mod).join(", ") }
	});
}
/** @param {Partial<Record<import('types').HttpMethod, any>>} mod */
function allowed_methods(mod) {
	const allowed = ENDPOINT_METHODS.filter((method) => method in mod);
	if ("GET" in mod && !("HEAD" in mod)) allowed.push("HEAD");
	return allowed;
}
/**
* @param {number} status
* @param {string} location
*/
function redirect_response(status, location) {
	return new Response(void 0, {
		status,
		headers: { location }
	});
}
/**
* @param {Response} response
*/
function with_version_header(response) {
	response.headers.set("x-sveltekit-version", "1788652438838");
	return response;
}
/**
* @param {import('@sveltejs/kit').RequestEvent} event
* @param {Error & { path: string }} error
*/
function clarify_devalue_error(event, error) {
	if (error.path) return `Data returned from \`load\` while rendering ${event.route.id} is not serializable: ${error.message} (${error.path}). If you need to serialize/deserialize custom types, use transport hooks: https://svelte.dev/docs/kit/hooks#transport.`;
	if (error.path === "") return `Data returned from \`load\` while rendering ${event.route.id} is not a plain object`;
	return error.message;
}
/**
* @param {import('types').ServerDataNode} node
*/
function serialize_uses(node) {
	const uses = {};
	if (node.uses && node.uses.dependencies.size > 0) uses.dependencies = Array.from(node.uses.dependencies);
	if (node.uses && node.uses.search_params.size > 0) uses.search_params = Array.from(node.uses.search_params);
	if (node.uses && node.uses.params.size > 0) uses.params = Array.from(node.uses.params);
	if (node.uses?.parent) uses.parent = 1;
	if (node.uses?.route) uses.route = 1;
	if (node.uses?.url) uses.url = 1;
	return uses;
}
/**
* Returns `true` if the given path was prerendered
* @param {import('@sveltejs/kit').SSRManifest} manifest
* @param {string} pathname Should include the base and be decoded
*/
function has_prerendered_path(manifest, pathname) {
	return manifest._.prerendered_routes.has(pathname) || pathname.at(-1) === "/" && manifest._.prerendered_routes.has(pathname.slice(0, -1));
}
/**
* Returns the filename without the extension. e.g., `+page.server`, `+page`, etc.
* @param {string | undefined} node_id
* @returns {string}
*/
function get_node_type(node_id) {
	const filename = (node_id?.split("/"))?.at(-1);
	if (!filename) return "unknown";
	return filename.split(".").slice(0, -1).join(".");
}
/**
* Counts HTML comments that are not SSI directives (which start with `<!--#`).
* Used to detect when `transformPageChunk` removes comments that Svelte needs for hydration.
* @param {string} str
* @returns {number}
*/
function count_non_ssi_comments(str) {
	return (str.match(/<!--(?!#)/g) ?? []).length;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/page/actions.js
/** @import { RequestEvent, Actions } from '@sveltejs/kit' */
/** @import { ActionResult } from '$app/forms' */
/** @import { SSRNode, ServerNode, ServerActionResult } from 'types' */
/** @param {RequestEvent} event */
function is_action_json_request(event) {
	return negotiate(event.request.headers.get("accept") ?? "*/*", ["application/json", "text/html"]) === "application/json" && event.request.method === "POST";
}
/**
* @param {RequestEvent} event
* @param {import('types').RequestState} state
* @param {SSRNode['server'] | undefined} server
*/
async function handle_action_json_request(event, state, server) {
	return action_result_json(event, state, await handle_action_request(event, state, server));
}
/**
* @param {RequestEvent} event
* @param {import('types').RequestState} state
* @param {ServerActionResult} result
* @returns {Promise<Response>}
*/
async function action_result_json(event, state, result) {
	if (result.type === "redirect") return action_json(result);
	if (result.type === "error") {
		const error = await handle_error_and_jsonify(event, state, result.error);
		return action_json({
			...result,
			error
		}, { status: error.status });
	}
	if (result.type === "success" && !result.data) return action_json({
		...result,
		status: 204,
		data: void 0
	});
	try {
		return action_json({
			...result,
			data: try_serialize(result.data, stringify, event.route.id)
		}, { status: result.status });
	} catch (e) {
		return action_result_json(event, state, action_error_result(e, result.location));
	}
}
/**
* @param {URL} url
*/
function get_action_location(url) {
	const location = new URL(url);
	for (const key of location.searchParams.keys()) if (key.startsWith("/")) {
		location.searchParams.delete(key);
		break;
	}
	return location.pathname + location.search;
}
/**
* @param {RequestEvent} event
* @param {string} location
* @returns {Extract<ServerActionResult, { type: 'error' }>}
*/
function method_not_allowed_result(event, location) {
	event.setHeaders({ allow: "GET" });
	return {
		type: "error",
		location,
		error: new SvelteKitError(405, "Method Not Allowed", `POST method not allowed. No form actions exist for this page`)
	};
}
/**
* @param {unknown} e
* @param {string} location
* @returns {Extract<ServerActionResult, { type: 'redirect' | 'error' }>}
*/
function action_error_result(e, location) {
	const err = normalize_error(e);
	if (err instanceof Redirect) return {
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
/**
* @param {Redirect} redirect
*/
function action_json_redirect(redirect) {
	return action_json({
		type: "redirect",
		status: redirect.status,
		location: redirect.location
	});
}
/**
* @param {ActionResult} data
* @param {ResponseInit} [init]
*/
function action_json(data, init) {
	return with_version_header(Response.json(data, init));
}
/**
* @param {RequestEvent} event
*/
function is_action_request(event) {
	return event.request.method === "POST";
}
/**
* @param {RequestEvent} event
* @param {import('types').RequestState} state
* @param {SSRNode['server'] | undefined} server
* @returns {Promise<ServerActionResult>}
*/
async function handle_action_request(event, state, server) {
	const actions = server?.actions;
	const location = get_action_location(event.url);
	if (!actions) return method_not_allowed_result(event, location);
	check_named_default_separate(actions);
	try {
		const data = await call_action(event, state, actions);
		if (data instanceof ActionFailure) return {
			type: "failure",
			status: data.status,
			location,
			data: data.data
		};
		else return {
			type: "success",
			status: 200,
			location,
			data
		};
	} catch (e) {
		return action_error_result(e instanceof ActionFailure ? /* @__PURE__ */ new Error("Cannot \"throw fail()\". Use \"return fail()\"") : e, location);
	}
}
/**
* @param {Actions} actions
*/
function check_named_default_separate(actions) {
	if (actions.default && Object.keys(actions).length > 1) throw new Error("When using named actions, the default action cannot be used. See the docs for more info: https://svelte.dev/docs/kit/form-actions#named-actions");
}
/**
* @param {RequestEvent} event
* @param {import('types').RequestState} state
* @param {NonNullable<ServerNode['actions']>} actions
* @throws {Redirect | HttpError | SvelteKitError | Error}
*/
async function call_action(event, state, actions) {
	const url = new URL(event.request.url);
	let name = "default";
	for (const param of url.searchParams) if (param[0].startsWith("/")) {
		name = param[0].slice(1);
		if (name === "default") throw new Error("Cannot use reserved action name \"default\"");
		break;
	}
	if (!Object.hasOwn(actions, name)) throw new SvelteKitError(404, "Not Found", `No action with name '${name}' found`);
	const action = actions[name];
	if (!is_form_content_type(event.request)) throw new SvelteKitError(415, "Unsupported Media Type", `Form actions expect form-encoded data — received ${event.request.headers.get("content-type")}`);
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
			if (result instanceof ActionFailure) current.setAttributes({
				"sveltekit.form_action.result.type": "failure",
				"sveltekit.form_action.result.status": result.status
			});
			return result;
		}
	});
}
/**
* Try to `devalue.uneval` the data object, and if it fails, return a proper Error with context
* @param {any} data
* @param {string} route_id
*/
function uneval_action_response(data, route_id) {
	return try_serialize(data, uneval, route_id);
}
/**
* @param {any} data
* @param {(data: any) => string} fn
* @param {string} route_id
*/
function try_serialize(data, fn, route_id) {
	try {
		return fn(data);
	} catch (e) {
		const error = e;
		if (data instanceof Response) throw new Error(`Data returned from action inside ${route_id} is not serializable. Form actions need to return plain objects or fail(). E.g. return { success: true } or return fail(400, { message: "invalid" });`, { cause: e });
		if ("path" in error) {
			let message = `Data returned from action inside ${route_id} is not serializable: ${error.message}`;
			if (error.path !== "") message += ` (data.${error.path})`;
			throw new Error(message, { cause: e });
		}
		throw error;
	}
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/remote-functions.js
/** @import { RequestEvent, SSRManifest } from '@sveltejs/kit' */
/** @import { RemoteForm } from '$app/server' */
/** @import { RemoteFormInternals, RemoteFunctionData, RemoteFunctionResponse, RemoteInternals, RequestState, ServerActionResult } from 'types' */
/**
* How long (in milliseconds) to wait after the last message was sent before
* sending a `: keep-alive` SSE comment, to prevent proxies/load balancers with
* an idle timeout from closing an otherwise-quiet `query.live` connection.
*/
var KEEP_ALIVE_INTERVAL = 3e4;
/**
* @param {RequestEvent} event
* @param {RequestState} state
* @param {import('types').RemoteQueryLiveInternals} internals
* @param {any} arg
*/
function create_live_query_response(event, state, internals, arg) {
	const cancellation = new AbortController();
	const live_event = {
		...event,
		request: new Request(event.request, { signal: AbortSignal.any([event.request.signal, cancellation.signal]) })
	};
	const generator = internals.run(live_event, state, arg);
	let open = true;
	let pulling = false;
	/** @type {ReadableStreamDefaultController<Uint8Array>} */
	let stream_controller;
	/** @type {ReturnType<typeof setTimeout> | undefined} */
	let keep_alive;
	/** @type {string | undefined} */
	let result;
	function schedule_keep_alive() {
		clearTimeout(keep_alive);
		keep_alive = setTimeout(() => {
			if (!open) return;
			if ((stream_controller.desiredSize ?? 0) > 0) stream_controller.enqueue(text_encoder.encode(": keep-alive\n\n"));
			schedule_keep_alive();
		}, KEEP_ALIVE_INTERVAL);
	}
	/** @param {any} data */
	function send(data) {
		if (!open) return;
		stream_controller.enqueue(text_encoder.encode("data: " + JSON.stringify(data) + "\n\n"));
		schedule_keep_alive();
	}
	/** @param {boolean} cancelled */
	function teardown(cancelled) {
		if (!open) return;
		open = false;
		clearTimeout(keep_alive);
		cancellation.abort();
		if (!cancelled) stream_controller.close();
		generator.return(void 0).catch(() => {});
	}
	event.request.signal.addEventListener("abort", () => teardown(true), { once: true });
	return new Response(new ReadableStream({
		start(controller) {
			stream_controller = controller;
			schedule_keep_alive();
		},
		async pull() {
			if (!open || pulling) return;
			pulling = true;
			try {
				while (open) {
					const { value, done } = await generator.next();
					if (!open) return;
					if (done) {
						teardown(false);
						return;
					}
					if (result !== (result = stringify(value))) {
						send({
							type: "result",
							result
						});
						return;
					}
				}
			} catch (error) {
				if (!open) return;
				if (error instanceof Redirect) send({
					type: "redirect",
					location: error.location
				});
				else send({
					type: "error",
					error: await handle_error_and_jsonify(event, state, error)
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
/** @type {typeof handle_remote_call_internal} */
async function handle_remote_call(event, state, manifest, id) {
	return record_span({
		name: "sveltekit.remote.call",
		attributes: { "sveltekit.remote.call.id": id },
		fn: async (current) => {
			const traced_event = merge_tracing(event, current);
			return with_version_header(await with_request_store({
				event: traced_event,
				state
			}, () => handle_remote_call_internal(traced_event, state, manifest, id)));
		}
	});
}
/**
* @param {RequestEvent} event
* @param {RequestState} state
* @param {SSRManifest} manifest
* @param {string} id
*/
async function handle_remote_call_internal(event, state, manifest, id) {
	const [hash, name, additional_args] = id.split("/");
	const remotes = manifest._.remotes;
	if (!Object.hasOwn(remotes, hash)) error(404);
	const module = await remotes[hash]();
	const fn = Object.hasOwn(module.default, name) ? module.default[name] : void 0;
	if (!fn) error(404);
	/** @type {RemoteInternals} */
	const internals = fn.__;
	event.tracing.current.setAttributes({
		"sveltekit.remote.call.type": internals.type,
		"sveltekit.remote.call.name": internals.name
	});
	/** @type {HeadersInit | undefined} */
	const headers = state.prerendering ? void 0 : { "cache-control": "private, no-store" };
	try {
		/** @type {RemoteFunctionData} */
		const data = {};
		switch (internals.type) {
			case "query_live":
				if (event.request.method !== "GET") throw new SvelteKitError(405, "Method Not Allowed", `\`query.live\` functions must be invoked via GET request, not ${event.request.method}`);
				return create_live_query_response(event, state, internals, parse_remote_arg(new URL(event.request.url).searchParams.get("payload")));
			case "query_batch": {
				if (event.request.method !== "POST") throw new SvelteKitError(405, "Method Not Allowed", `\`query.batch\` functions must be invoked via POST request, not ${event.request.method}`);
				/** @type {{ payloads: string[] }} */
				const { payloads } = await event.request.json();
				const args = await Promise.all(payloads.map((payload) => parse_remote_arg(payload)));
				data._ = await with_request_store({
					event,
					state
				}, () => internals.run(args));
				break;
			}
			case "form": {
				if (event.request.method !== "POST") throw new SvelteKitError(405, "Method Not Allowed", `\`form\` functions must be invoked via POST request, not ${event.request.method}`);
				if (!is_form_content_type(event.request)) throw new SvelteKitError(415, "Unsupported Media Type", `\`form\` functions expect form-encoded data — received ${event.request.headers.get("content-type")}`);
				const { data: input, meta, form_data } = await deserialize_binary_form(event.request, internals.id);
				state.remote.requested = create_requested_map(meta.remote_refreshes);
				if (additional_args && !("id" in input)) input.id = JSON.parse(decodeURIComponent(additional_args));
				const fn = internals.fn;
				data._ = await with_request_store({
					event,
					state: {
						...state,
						is_in_remote_form_or_command: true
					}
				}, () => fn(input, meta, form_data));
				if (data._.issues) return Response.json({
					type: "result",
					data: stringify(data)
				}, { headers });
				break;
			}
			case "command": {
				/** @type {{ payload: string, refreshes?: string[] }} */
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
			data: stringify(data)
		}, { headers });
	} catch (error) {
		if (error instanceof Redirect) {
			const data = await collect_remote_data({ redirect: error.location }, event, state);
			return Response.json({
				type: "result",
				data: stringify(data)
			}, { headers });
		}
		const transformed = await handle_error_and_jsonify(event, state, error);
		return Response.json({
			type: "error",
			error: transformed
		}, {
			status: state.prerendering ? transformed.status : void 0,
			headers: { "cache-control": "private, no-store" }
		});
	}
}
/**
* Collects all the query/prerender data that was retrieved
* during the request and adds it to `data`
* @param {RemoteFunctionData} data
* @param {RequestEvent} event
* @param {RequestState} state
*/
async function collect_remote_data(data, event, state) {
	/**
	*
	* @param {unknown} error
	* @returns {Promise<App.Error>}
	*/
	function convert_error(error) {
		return Promise.resolve(handle_error_and_jsonify(event, state, error));
	}
	/** @type {Promise<any>[]} */
	const promises = [];
	/** @type {Set<string>} */
	const processed = /* @__PURE__ */ new Set();
	if (state.remote.explicit) {
		const { explicit } = state.remote;
		/** @type {Promise<void>[]} */
		const inflight = [];
		const drain = () => {
			for (const [remote_key, { internals, fn }] of explicit) {
				explicit.delete(remote_key);
				if (processed.has(remote_key)) continue;
				processed.add(remote_key);
				data.r = true;
				const type = internals.type === "query_live" ? "l" : internals.type[0];
				inflight.push(fn().then((v) => {
					(data[type] ??= {})[remote_key] = { v };
					drain();
				}, async (e) => {
					if (!(e instanceof Redirect)) (data[type] ??= {})[remote_key] = { e: await convert_error(e) };
					drain();
				}));
			}
		};
		drain();
		for (const promise of inflight) await promise;
	}
	if (state.remote.implicit) for (const [internals, record] of state.remote.implicit) {
		if (!internals.id) continue;
		for (const key in record) {
			const remote_key = internals.type === "form" ? key : create_remote_key(internals.id, key);
			if (processed.has(remote_key)) continue;
			const type = internals.type === "query_live" ? "l" : internals.type[0];
			const promise = state.remote.data?.get(internals)?.[key] ?? record[key]();
			let resolved = true;
			await Promise.race([Promise.resolve(promise).then((v) => {
				if (resolved) ((data[type] ??= {})[remote_key] ??= {}).v = v;
			}, (e) => {
				if (e instanceof Redirect) return;
				if (resolved) promises.push(convert_error(e).then((e) => {
					((data[type] ??= {})[remote_key] ??= {}).e = e;
				}));
			}), Promise.resolve().then(() => resolved = false)]);
		}
	}
	await Promise.all(promises);
	return data;
}
/**
* @param {string[] | undefined} refreshes
*/
function create_requested_map(refreshes) {
	/** @type {Map<string, string[]>} */
	const requested = /* @__PURE__ */ new Map();
	for (const key of refreshes ?? []) {
		const parts = split_remote_key(key);
		const existing = requested.get(parts.id);
		if (existing) existing.push(parts.payload);
		else requested.set(parts.id, [parts.payload]);
	}
	return requested;
}
/** @type {typeof handle_remote_form_post_internal} */
async function handle_remote_form_post(event, state, manifest, id) {
	return record_span({
		name: "sveltekit.remote.form.post",
		attributes: { "sveltekit.remote.form.post.id": id },
		fn: (current) => {
			const traced_event = merge_tracing(event, current);
			return with_request_store({
				event: traced_event,
				state
			}, () => handle_remote_form_post_internal(traced_event, state, manifest, id));
		}
	});
}
/**
* @param {RequestEvent} event
* @param {RequestState} state
* @param {SSRManifest} manifest
* @param {string} id
* @returns {Promise<ServerActionResult>}
*/
async function handle_remote_form_post_internal(event, state, manifest, id) {
	const location = get_action_location(event.url);
	const [hash, name, ...rest] = id.split("/");
	const action_id = rest.join("/");
	const remotes = manifest._.remotes;
	const module = Object.hasOwn(remotes, hash) ? await remotes[hash]() : void 0;
	let form = module && Object.hasOwn(module.default, name) ? module.default[name] : void 0;
	if (!form) return method_not_allowed_result(event, location);
	if (action_id) form = with_request_store({
		event,
		state
	}, () => form.for(JSON.parse(action_id)));
	try {
		const __ = form.__;
		const { data, meta, form_data } = await deserialize_binary_form(event.request, __.id);
		if (action_id && !("id" in data)) data.id = JSON.parse(decodeURIComponent(action_id));
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
/**
* @param {URL} url
*/
function has_remote_prefix(url) {
	return url.pathname.startsWith(`/${app_dir}/remote/`);
}
/**
* @param {URL} url
*/
function strip_remote_prefix(url) {
	return url.pathname.replace(`/${app_dir}/remote/`, "");
}
/**
* @param {URL} url
*/
function get_remote_id(url) {
	return has_remote_prefix(url) && strip_remote_prefix(url);
}
/**
* @param {URL} url
*/
function get_remote_action(url) {
	return url.searchParams.get("/remote");
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/sourcemaps.js
var fs = globalThis.process?.getBuiltinModule?.("node:fs");
var url = globalThis.process?.getBuiltinModule?.("node:url");
var path = globalThis.process?.getBuiltinModule?.("node:path");
var module = globalThis.process?.getBuiltinModule?.("node:module");
var cwd = globalThis.process?.cwd?.();
/** @type {(file: string) => string} */
var relative = cwd ? (file) => path.relative(cwd, file) : (file) => file;
/**
* Applies sourcemaps, makes paths relative to the cwd, and truncates
* non-user code from the bottom of the stack
* @param {Error} error
* @returns void
*/
var fix_stack_trace = (error) => {
	if (!error.stack || !fs) return;
	let end = 0;
	error.stack = error.stack.split("\n").map((line, i) => {
		const match = line.match(/^ {4}at.+(file:\/\/\/.*):(\d+):(\d+)(\)?)$/);
		if (!match) {
			if (!line.includes("node:internal/")) end = i + 1;
			return line;
		}
		const file = url.fileURLToPath(match[1]);
		const traced = trace(file, Number(match[2]) - 1, Number(match[3]) - 1);
		if (!/[\\/]node_modules[\\/]/.test(traced?.file ?? file)) end = i + 1;
		if (traced?.line) {
			const location = `${match[1]}:${match[2]}:${match[3]}`;
			const original = `${relative(traced.file)}:${traced.line}:${traced.column}`;
			return line.replace(location, original);
		}
		if (traced) return `${line.replace(match[1], relative(file))} [${traced.file}]`;
		return line;
	}).slice(0, end).join("\n");
};
/**
* Override the implementation of fix_stack_trace (for using during dev)
* @param {(error: Error) => void} fn
*/
function set_fix_stack_trace(fn) {
	fix_stack_trace = fn;
}
/** @type {Map<string, { map: import('node:module').SourceMap; directory: string } | null>} */
var source_maps = /* @__PURE__ */ new Map();
/** @type {Map<string, Array<string | undefined>>} */
var source_regions = /* @__PURE__ */ new Map();
/** @param {string} file */
function get_source_map(file) {
	if (source_maps.has(file)) return source_maps.get(file);
	try {
		let source;
		let directory = path.dirname(file);
		const code = fs.readFileSync(file, "utf8");
		const url = Array.from(code.matchAll(/\/\/[#@]\s*sourceMappingURL=(\S+)/g)).at(-1)?.[1];
		if (url?.startsWith("data:")) {
			const comma = url.indexOf(",");
			const metadata = url.slice(5, comma);
			const data = url.slice(comma + 1);
			source = metadata.endsWith(";base64") ? Buffer.from(data, "base64").toString() : decodeURIComponent(data);
		} else {
			const map_file = url ? path.resolve(path.dirname(file), decodeURIComponent(url)) : `${file}.map`;
			if (fs.existsSync(map_file)) {
				directory = path.dirname(map_file);
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
/**
*
* @param {string} file
* @param {number} line
* @param {number} column
* @returns {null | { file: string, line?: number, column?: number }}
*/
function trace(file, line, column) {
	const source_map = get_source_map(file);
	if (!source_map) return null;
	const entry = source_map.map.findEntry(line, column);
	if (entry && "originalSource" in entry && entry.originalSource && typeof entry.originalLine === "number" && typeof entry.originalColumn === "number") {
		const traced = {
			file: entry.originalSource.startsWith("file:") ? url.fileURLToPath(entry.originalSource) : path.resolve(source_map.directory, entry.originalSource),
			line: entry.originalLine + 1,
			column: entry.originalColumn + 1
		};
		return trace(traced.file, traced.line - 1, traced.column - 1) ?? traced;
	}
	let regions = source_regions.get(file);
	if (!regions) {
		/** @type {string | undefined} */
		let source;
		regions = fs.readFileSync(file, "utf8").split("\n").map((line) => {
			const start = line.match(/^\/\/#region (.+)$/);
			if (start) source = start[1];
			if (line === "//#endregion") source = void 0;
			return source;
		});
		source_regions.set(file, regions);
	}
	const source = regions[line];
	if (source) return { file: source };
	return null;
}
//#endregion
//#region node_modules/@sveltejs/kit/src/runtime/server/internal.js
var styleText = globalThis.process?.getBuiltinModule?.("node:util")?.styleText ?? ((_format, text) => text);
var read_implementation = false;
var options$1 = false;
var hooks = false;
/**
* @param {(path: string) => ReadableStream<any>} fn
*/
function set_read_implementation(fn) {
	read_implementation = fn;
}
/**
*
* @param {SSRManifest} value
*/
function set_manifest(value) {}
/**
* @param {SSROptions} value
*/
function set_options(value) {
	options$1 = value;
}
/**
* @param {ServerHooks} value
*/
function set_hooks(value) {
	hooks = value;
}
/**
* @param {number} status
* @param {Request} request
* @returns {string}
*/
function format_response(status, request) {
	const url = new URL(request.url);
	const requested = url.href.replace(url.origin, "");
	let log = `${styleText(status < 400 ? ["cyan"] : ["bold", "red"], `${status}`)} ${request.method} `;
	if (has_data_suffix(url.pathname)) {
		const pathname = strip_data_suffix(url.pathname) || "/";
		log += pathname + styleText("dim", requested.slice(pathname.length));
	} else if (has_resolution_suffix(url.pathname)) {
		const pathname = strip_resolution_suffix(url.pathname) || "/";
		log += pathname + styleText("dim", requested.slice(pathname.length));
	} else if (has_remote_prefix(url)) {
		const id = strip_remote_prefix(url);
		const [file_hash, name, arg_hash] = id.split("/");
		log += styleText("dim", `${url.pathname.slice(0, -id.length)}${file_hash}/`) + name;
		if (arg_hash) log += styleText("dim", `/${arg_hash}`);
		if (url.search) log += styleText("dim", url.search);
	} else log += requested;
	return log;
}
var prerendering = false;
function set_building() {}
function set_prerendering() {
	prerendering = true;
}
//#endregion
//#region .svelte-kit/generated/build/shared/error-template.js
var error_template_default = ({ status, message }) => "<!doctype html>\n<html lang=\"en\">\n	<head>\n		<meta charset=\"utf-8\" />\n		<title>" + message + "</title>\n\n		<style>\n			body {\n				--bg: white;\n				--fg: #222;\n				--divider: #ccc;\n				background: var(--bg);\n				color: var(--fg);\n				font-family:\n					system-ui,\n					-apple-system,\n					BlinkMacSystemFont,\n					'Segoe UI',\n					Roboto,\n					Oxygen,\n					Ubuntu,\n					Cantarell,\n					'Open Sans',\n					'Helvetica Neue',\n					sans-serif;\n				display: flex;\n				align-items: center;\n				justify-content: center;\n				height: 100vh;\n				margin: 0;\n			}\n\n			.error {\n				display: flex;\n				align-items: center;\n				max-width: 32rem;\n				margin: 0 1rem;\n			}\n\n			.status {\n				font-weight: 200;\n				font-size: 3rem;\n				line-height: 1;\n				position: relative;\n				top: -0.05rem;\n			}\n\n			.message {\n				border-left: 1px solid var(--divider);\n				padding: 0 0 0 1rem;\n				margin: 0 0 0 1rem;\n				min-height: 2.5rem;\n				display: flex;\n				align-items: center;\n			}\n\n			.message h1 {\n				font-weight: 400;\n				font-size: 1em;\n				margin: 0;\n			}\n\n			@media (prefers-color-scheme: dark) {\n				body {\n					--bg: #222;\n					--fg: #ddd;\n					--divider: #666;\n				}\n			}\n		</style>\n	</head>\n	<body>\n		<div class=\"error\">\n			<span class=\"status\">" + status + "</span>\n			<div class=\"message\">\n				<h1>" + message + "</h1>\n			</div>\n		</div>\n	</body>\n</html>\n";
//#endregion
//#region .svelte-kit/generated/build/server.js
var options = {
	app_template_contains_nonce: false,
	csp: {
		"mode": "auto",
		"directives": {
			"upgrade-insecure-requests": false,
			"block-all-mixed-content": false
		},
		"reportOnly": {
			"upgrade-insecure-requests": false,
			"block-all-mixed-content": false
		}
	},
	csrf_trusted_origins: [],
	service_worker_options: void 0,
	templates: {
		app: ({ head, body, assets, nonce, env }) => "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\" />\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n    <link rel=\"preconnect\" href=\"https://fonts.googleapis.com\" />\n    <link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin />\n    <link\n      href=\"https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;600&display=swap\"\n      rel=\"stylesheet\"\n    />\n    " + head + "\n  </head>\n  <body data-sveltekit-preload-data=\"hover\">\n    <div style=\"display: contents\">" + body + "</div>\n  </body>\n</html>\n",
		error: error_template_default
	}
};
async function get_hooks() {
	let handle;
	let handleFetch;
	let handleError;
	let init;
	({handle, handleFetch, handleError, init} = await import("../entries/hooks.server.js"));
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
//#endregion
export { uneval as $, has_prerendered_path as A, REROUTED_URL_HEADER as At, normalize_error as B, handle_action_request as C, assets as Ct, clarify_devalue_error as D, IN_WEBCONTAINER as Dt, uneval_action_response as E, ENDPOINT_METHODS as Et, handle_error_and_jsonify as F, stream_text as Ft, stringify_remote_arg as G, TRAILING_SLASH_PARAM as H, handle_fatal_error as I, text_encoder as It, encoders as J, validate_depends as K, static_error_page as L, redirect_response as M, base64_encode as Mt, serialize_uses as N, get_relative_path as Nt, count_non_ssi_comments as O, MUTATIVE_METHODS as Ot, with_version_header as P, stream_from_iterable as Pt, stringify as Q, escape_html as R, handle_action_json_request as S, app_dir as St, is_action_request as T, BODY_DEPENDENT_METHODS as Tt, create_remote_key as U, INVALIDATED_PARAM as V, parse_remote_arg as W, init_transport as X, has_custom_transporters as Y, parse as Z, get_remote_action as _, make_trackable as _t, set_prerendering as a, normalize_issue as at, handle_remote_form_post as b, resolve as bt, options$1 as c, add_data_suffix as ct, set_manifest as d, has_resolution_suffix as dt, is_form_content_type as et, set_options as f, strip_data_suffix as ft, collect_remote_data as g, disable_search as gt, set_fix_stack_trace as h, decode_pathname as ht, set_building as i, flatten_issues as it, method_not_allowed as j, SVELTE_KIT_ASSETS as jt, get_node_type as k, PAGE_METHODS as kt, read_implementation as l, add_resolution_suffix as lt, fix_stack_trace as m, SCHEME as mt, options as n, create_field_proxy as nt, format_response as o, parse_form_key as ot, set_read_implementation as p, strip_resolution_suffix as pt, validate_load_response as q, prerendering as r, deep_set as rt, hooks as s, set_nested_value as st, get_hooks as t, negotiate as tt, set_hooks as u, has_data_suffix as ut, get_remote_id as v, normalize_path as vt, is_action_json_request as w, set_assets as wt, action_json_redirect as x, find_route as xt, handle_remote_call as y, relative_pathname as yt, get_status as z };

//# sourceMappingURL=server.js.map