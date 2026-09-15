// @bun
import {
  __require
} from "./index-qcep1gwj.js";

// node_modules/devalue/src/constants.js
var UNDEFINED = -1;
var HOLE = -2;
var NAN = -3;
var POSITIVE_INFINITY = -4;
var NEGATIVE_INFINITY = -5;
var NEGATIVE_ZERO = -6;
var SPARSE = -7;
var MAX_ARRAY_LEN = 2 ** 32 - 1;
var MAX_ARRAY_INDEX = MAX_ARRAY_LEN - 1;

// node_modules/devalue/src/utils.js
var escaped = {
  "<": "\\u003C",
  "\\": "\\\\",
  "\b": "\\b",
  "\f": "\\f",
  "\n": "\\n",
  "\r": "\\r",
  "\t": "\\t",
  "\u2028": "\\u2028",
  "\u2029": "\\u2029"
};

class DevalueError extends Error {
  constructor(message, keys, value, root) {
    super(message);
    this.name = "DevalueError";
    this.path = keys.join("");
    this.value = value;
    this.root = root;
  }
}
function is_primitive(thing) {
  return thing === null || typeof thing !== "object" && typeof thing !== "function";
}
var object_proto_names = /* @__PURE__ */ Object.getOwnPropertyNames(Object.prototype).sort().join("\x00");
function is_plain_object(thing) {
  const proto = Object.getPrototypeOf(thing);
  return proto === Object.prototype || proto === null || Object.getPrototypeOf(proto) === null || Object.getOwnPropertyNames(proto).sort().join("\x00") === object_proto_names;
}
function get_type(thing) {
  return Object.prototype.toString.call(thing).slice(8, -1);
}
function get_escaped_char(char) {
  switch (char) {
    case '"':
      return "\\\"";
    case "<":
      return "\\u003C";
    case "\\":
      return "\\\\";
    case `
`:
      return "\\n";
    case "\r":
      return "\\r";
    case "\t":
      return "\\t";
    case "\b":
      return "\\b";
    case "\f":
      return "\\f";
    case "\u2028":
      return "\\u2028";
    case "\u2029":
      return "\\u2029";
    default:
      return char < " " ? `\\u${char.charCodeAt(0).toString(16).padStart(4, "0")}` : "";
  }
}
function stringify_string(str) {
  let result = "";
  let last_pos = 0;
  const len = str.length;
  for (let i = 0;i < len; i += 1) {
    const char = str[i];
    const replacement = get_escaped_char(char);
    if (replacement) {
      result += str.slice(last_pos, i) + replacement;
      last_pos = i + 1;
    }
  }
  return `"${last_pos === 0 ? str : result + str.slice(last_pos)}"`;
}
function enumerable_symbols(object) {
  return Object.getOwnPropertySymbols(object).filter((symbol) => Object.getOwnPropertyDescriptor(object, symbol).enumerable);
}
var is_identifier = /^[a-zA-Z_$][a-zA-Z_$0-9]*$/;
function stringify_key(key) {
  return is_identifier.test(key) ? "." + key : "[" + JSON.stringify(key) + "]";
}
function is_valid_array_index(n) {
  if (!Number.isInteger(n))
    return false;
  if (n < 0)
    return false;
  if (n > MAX_ARRAY_INDEX)
    return false;
  return true;
}
function is_valid_array_len(n) {
  if (!Number.isInteger(n))
    return false;
  if (n < 0)
    return false;
  if (n > MAX_ARRAY_LEN)
    return false;
  return true;
}
function is_valid_array_index_string(s) {
  if (s.length === 0)
    return false;
  if (s.length > 1 && s.charCodeAt(0) === 48)
    return false;
  for (let i = 0;i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c < 48 || c > 57)
      return false;
  }
  return is_valid_array_index(+s);
}
function array_index_cut(keys) {
  for (var i = keys.length - 1;i >= 0; i--) {
    if (is_valid_array_index_string(keys[i])) {
      break;
    }
  }
  return i + 1;
}
function valid_array_indices(array) {
  const keys = Object.keys(array);
  keys.length = array_index_cut(keys);
  return keys;
}

// node_modules/devalue/src/uneval.js
var chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
var unsafe_chars = /[<\b\f\n\r\t\0\u2028\u2029]/g;
var reserved = /^(?:do|if|in|for|int|let|new|try|var|byte|case|char|else|enum|goto|long|this|void|with|await|break|catch|class|const|final|float|short|super|throw|while|yield|delete|double|export|import|native|return|switch|throws|typeof|boolean|default|extends|finally|package|private|abstract|continue|debugger|function|volatile|interface|protected|transient|implements|instanceof|synchronized)$/;
function uneval(value, replacer) {
  const counts = new Map;
  const keys = [];
  const custom = new Map;
  function walk(thing) {
    if (!is_primitive(thing)) {
      if (counts.has(thing)) {
        counts.set(thing, counts.get(thing) + 1);
        return;
      }
      counts.set(thing, 1);
      if (replacer) {
        const str2 = replacer(thing, (value2) => uneval(value2, replacer));
        if (typeof str2 === "string") {
          custom.set(thing, str2);
          return;
        }
      }
      if (typeof thing === "function") {
        throw new DevalueError(`Cannot stringify a function`, keys, thing, value);
      }
      const type = get_type(thing);
      switch (type) {
        case "Number":
        case "BigInt":
        case "String":
        case "Boolean":
        case "Date":
        case "RegExp":
        case "URL":
        case "URLSearchParams":
          return;
        case "Array":
          thing.forEach((value2, i) => {
            keys.push(`[${i}]`);
            walk(value2);
            keys.pop();
          });
          break;
        case "Set":
          Array.from(thing).forEach(walk);
          break;
        case "Map":
          for (const [key, value2] of thing) {
            keys.push(`.get(${is_primitive(key) ? stringify_primitive(key) : "..."})`);
            walk(key);
            walk(value2);
            keys.pop();
          }
          break;
        case "Int8Array":
        case "Uint8Array":
        case "Uint8ClampedArray":
        case "Int16Array":
        case "Uint16Array":
        case "Float16Array":
        case "Int32Array":
        case "Uint32Array":
        case "Float32Array":
        case "Float64Array":
        case "BigInt64Array":
        case "BigUint64Array":
        case "DataView":
          walk(thing.buffer);
          return;
        case "ArrayBuffer":
          return;
        case "Temporal.Duration":
        case "Temporal.Instant":
        case "Temporal.PlainDate":
        case "Temporal.PlainTime":
        case "Temporal.PlainDateTime":
        case "Temporal.PlainMonthDay":
        case "Temporal.PlainYearMonth":
        case "Temporal.ZonedDateTime":
          return;
        default:
          if (!is_plain_object(thing)) {
            throw new DevalueError(`Cannot stringify arbitrary non-POJOs`, keys, thing, value);
          }
          if (enumerable_symbols(thing).length > 0) {
            throw new DevalueError(`Cannot stringify POJOs with symbolic keys`, keys, thing, value);
          }
          for (const key of Object.keys(thing)) {
            if (key === "__proto__") {
              throw new DevalueError(`Cannot stringify objects with __proto__ keys`, keys, thing, value);
            }
            keys.push(stringify_key(key));
            walk(thing[key]);
            keys.pop();
          }
      }
    } else if (typeof thing === "symbol") {
      throw new DevalueError(`Cannot stringify a Symbol primitive`, keys, thing, value);
    }
  }
  walk(value);
  const names = new Map;
  Array.from(counts).filter((entry) => entry[1] > 1).sort((a, b) => b[1] - a[1]).forEach((entry, i) => {
    names.set(entry[0], get_name(i));
  });
  function stringify(thing) {
    if (names.has(thing)) {
      return names.get(thing);
    }
    if (is_primitive(thing)) {
      return stringify_primitive(thing);
    }
    if (custom.has(thing)) {
      return custom.get(thing);
    }
    const type = get_type(thing);
    switch (type) {
      case "Number":
      case "String":
      case "Boolean":
      case "BigInt":
        return `Object(${stringify(thing.valueOf())})`;
      case "RegExp":
        const { source, flags } = thing;
        return flags ? `new RegExp(${stringify_string(source)},"${flags}")` : `new RegExp(${stringify_string(source)})`;
      case "Date":
        return `new Date(${thing.getTime()})`;
      case "URL":
        return `new URL(${stringify_string(thing.toString())})`;
      case "URLSearchParams":
        return `new URLSearchParams(${stringify_string(thing.toString())})`;
      case "Array": {
        let has_holes = false;
        let result = "[";
        for (let i = 0;i < thing.length; i += 1) {
          if (i > 0)
            result += ",";
          if (Object.hasOwn(thing, i)) {
            result += stringify(thing[i]);
          } else if (!has_holes) {
            const populated_keys = valid_array_indices(thing);
            const population = populated_keys.length;
            const d = String(thing.length).length;
            const hole_cost = thing.length + 2;
            const sparse_cost = 25 + d + population * (d + 2);
            if (hole_cost > sparse_cost) {
              const entries = populated_keys.map((k) => `${k}:${stringify(thing[k])}`).join(",");
              return `Object.assign(Array(${thing.length}),{${entries}})`;
            }
            has_holes = true;
          }
        }
        const tail = thing.length === 0 || thing.length - 1 in thing ? "" : ",";
        return result + tail + "]";
      }
      case "Set":
      case "Map":
        return `new ${type}([${Array.from(thing).map(stringify).join(",")}])`;
      case "Int8Array":
      case "Uint8Array":
      case "Uint8ClampedArray":
      case "Int16Array":
      case "Uint16Array":
      case "Float16Array":
      case "Int32Array":
      case "Uint32Array":
      case "Float32Array":
      case "Float64Array":
      case "BigInt64Array":
      case "BigUint64Array": {
        let str2 = `new ${type}`;
        if (!names.has(thing.buffer)) {
          str2 += `([${stringify_typed_array_elements(type, thing.buffer)}])`;
        } else {
          str2 += `(${stringify(thing.buffer)})`;
        }
        if (thing.byteLength !== thing.buffer.byteLength) {
          const start = thing.byteOffset / thing.BYTES_PER_ELEMENT;
          const end = start + thing.length;
          str2 += `.subarray(${start},${end})`;
        }
        return str2;
      }
      case "DataView": {
        let str2 = `new DataView`;
        if (!names.has(thing.buffer)) {
          str2 += `(new Uint8Array([${new Uint8Array(thing.buffer)}]).buffer`;
        } else {
          str2 += `(${stringify(thing.buffer)}`;
        }
        if (thing.byteLength !== thing.buffer.byteLength) {
          str2 += `,${thing.byteOffset},${thing.byteLength}`;
        }
        return str2 + ")";
      }
      case "ArrayBuffer": {
        const ui8 = new Uint8Array(thing);
        return `new Uint8Array([${ui8.toString()}]).buffer`;
      }
      case "Temporal.Duration":
      case "Temporal.Instant":
      case "Temporal.PlainDate":
      case "Temporal.PlainTime":
      case "Temporal.PlainDateTime":
      case "Temporal.PlainMonthDay":
      case "Temporal.PlainYearMonth":
      case "Temporal.ZonedDateTime":
        return `${type}.from(${stringify_string(thing.toString())})`;
      default:
        const keys2 = Object.keys(thing);
        const obj = keys2.map((key) => `${safe_key(key)}:${stringify(thing[key])}`).join(",");
        const proto = Object.getPrototypeOf(thing);
        if (proto === null) {
          return keys2.length > 0 ? `{${obj},__proto__:null}` : `{__proto__:null}`;
        }
        return `{${obj}}`;
    }
  }
  const str = stringify(value);
  if (names.size) {
    const params = [];
    const statements = [];
    const values = [];
    const reconstructions = [];
    names.forEach((name, thing) => {
      params.push(name);
      if (custom.has(thing)) {
        values.push(custom.get(thing));
        return;
      }
      if (is_primitive(thing)) {
        values.push(stringify_primitive(thing));
        return;
      }
      const type = get_type(thing);
      switch (type) {
        case "Number":
        case "String":
        case "Boolean":
        case "BigInt":
          values.push(`Object(${stringify(thing.valueOf())})`);
          break;
        case "RegExp":
          const { source, flags } = thing;
          const regexp = flags ? `new RegExp(${stringify_string(source)},"${flags}")` : `new RegExp(${stringify_string(source)})`;
          values.push(regexp);
          break;
        case "Date":
          values.push(`new Date(${thing.getTime()})`);
          break;
        case "URL":
          values.push(`new URL(${stringify_string(thing.toString())})`);
          break;
        case "URLSearchParams":
          values.push(`new URLSearchParams(${stringify_string(thing.toString())})`);
          break;
        case "Array":
          values.push(`Array(${thing.length})`);
          thing.forEach((v, i) => {
            statements.push(`${name}[${i}]=${stringify(v)}`);
          });
          break;
        case "Set": {
          values.push(`new Set`);
          const adds = Array.from(thing).map((v) => `.add(${stringify(v)})`);
          if (adds.length > 0)
            statements.push(name + adds.join(""));
          break;
        }
        case "Map": {
          values.push(`new Map`);
          const sets = Array.from(thing).map(([k, v]) => `.set(${stringify(k)}, ${stringify(v)})`);
          if (sets.length > 0)
            statements.push(name + sets.join(""));
          break;
        }
        case "Int8Array":
        case "Uint8Array":
        case "Uint8ClampedArray":
        case "Int16Array":
        case "Uint16Array":
        case "Float16Array":
        case "Int32Array":
        case "Uint32Array":
        case "Float32Array":
        case "Float64Array":
        case "BigInt64Array":
        case "BigUint64Array": {
          let str2 = `new ${type}`;
          if (!names.has(thing.buffer)) {
            str2 += `([${stringify_typed_array_elements(type, thing.buffer)}])`;
          } else {
            str2 += `(${stringify(thing.buffer)})`;
          }
          if (thing.byteLength !== thing.buffer.byteLength) {
            const start = thing.byteOffset / thing.BYTES_PER_ELEMENT;
            const end = start + thing.length;
            str2 += `.subarray(${start},${end})`;
          }
          values.push(`{}`);
          reconstructions.push(`${name}=${str2}`);
          break;
        }
        case "DataView": {
          let str2 = `new DataView`;
          if (!names.has(thing.buffer)) {
            str2 += `(new Uint8Array([${new Uint8Array(thing.buffer)}]).buffer`;
          } else {
            str2 += `(${stringify(thing.buffer)}`;
          }
          if (thing.byteLength !== thing.buffer.byteLength) {
            str2 += `,${thing.byteOffset},${thing.byteLength}`;
          }
          str2 += ")";
          values.push(`{}`);
          reconstructions.push(`${name}=${str2}`);
          break;
        }
        case "ArrayBuffer":
          values.push(`new Uint8Array([${new Uint8Array(thing)}]).buffer`);
          break;
        case "Temporal.Duration":
        case "Temporal.Instant":
        case "Temporal.PlainDate":
        case "Temporal.PlainTime":
        case "Temporal.PlainDateTime":
        case "Temporal.PlainMonthDay":
        case "Temporal.PlainYearMonth":
        case "Temporal.ZonedDateTime":
          values.push(`${type}.from(${stringify_string(thing.toString())})`);
          break;
        default:
          values.push(Object.getPrototypeOf(thing) === null ? "Object.create(null)" : "{}");
          Object.keys(thing).forEach((key) => {
            statements.push(`${name}${safe_prop(key)}=${stringify(thing[key])}`);
          });
      }
    });
    statements.push(`return ${str}`);
    const body = [...reconstructions, ...statements].join(";");
    if (params.length > 65534) {
      return `(function(){var[${params.join(",")}]=arguments[0];${body}}([${values.join(",")}]))`;
    }
    return `(function(${params.join(",")}){${body}}(${values.join(",")}))`;
  } else {
    return str;
  }
}
function stringify_typed_array_elements(type, buffer) {
  const array = new globalThis[type](buffer);
  if (type === "BigInt64Array" || type === "BigUint64Array") {
    return Array.from(array, (element) => `${element}n`).join(",");
  }
  if (array instanceof Float32Array || array instanceof Float64Array || typeof Float16Array !== "undefined" && array instanceof Float16Array) {
    return Array.from(array, (element) => Object.is(element, -0) ? "-0" : `${element}`).join(",");
  }
  return array.toString();
}
function get_name(num) {
  let name = "";
  do {
    name = chars[num % chars.length] + name;
    num = ~~(num / chars.length) - 1;
  } while (num >= 0);
  return reserved.test(name) ? `${name}0` : name;
}
function escape_unsafe_char(c) {
  return escaped[c] || c;
}
function escape_unsafe_chars(str) {
  return str.replace(unsafe_chars, escape_unsafe_char);
}
function safe_key(key) {
  return /^[_$a-zA-Z][_$a-zA-Z0-9]*$/.test(key) ? key : escape_unsafe_chars(JSON.stringify(key));
}
function safe_prop(key) {
  return /^[_$a-zA-Z][_$a-zA-Z0-9]*$/.test(key) ? `.${key}` : `[${escape_unsafe_chars(JSON.stringify(key))}]`;
}
function stringify_primitive(thing) {
  const type = typeof thing;
  if (type === "string")
    return stringify_string(thing);
  if (thing === undefined)
    return "void 0";
  if (thing === 0 && 1 / thing < 0)
    return "-0";
  const str = String(thing);
  if (type === "number")
    return str.replace(/^(-)?0\./, "$1.");
  if (type === "bigint")
    return thing + "n";
  return str;
}
// node_modules/devalue/src/base64.js
function encode_native(array_buffer) {
  return new Uint8Array(array_buffer).toBase64();
}
function decode_native(base64) {
  return Uint8Array.fromBase64(base64).buffer;
}
function encode_buffer(array_buffer) {
  return Buffer.from(array_buffer).toString("base64");
}
function decode_buffer(base64) {
  return Uint8Array.from(Buffer.from(base64, "base64")).buffer;
}
function encode_legacy(array_buffer) {
  const array = new Uint8Array(array_buffer);
  let binary = "";
  const chunk_size = 32768;
  for (let i = 0;i < array.length; i += chunk_size) {
    const chunk = array.subarray(i, i + chunk_size);
    binary += String.fromCharCode.apply(null, chunk);
  }
  return btoa(binary);
}
function decode_legacy(base64) {
  const binary_string = atob(base64);
  const len = binary_string.length;
  const array = new Uint8Array(len);
  for (let i = 0;i < len; i++) {
    array[i] = binary_string.charCodeAt(i);
  }
  return array.buffer;
}
var native = typeof Uint8Array.fromBase64 === "function";
var buffer = typeof process === "object" && process.versions?.node !== undefined;
var encode64 = native ? encode_native : buffer ? encode_buffer : encode_legacy;
var decode64 = native ? decode_native : buffer ? decode_buffer : decode_legacy;

// node_modules/devalue/src/operations.js
function merge_operations(defaults, overrides) {
  if (!overrides)
    return defaults;
  const merged = {};
  for (const key of Object.keys(defaults)) {
    merged[key] = overrides[key] ?? defaults[key];
  }
  return merged;
}
var NOT_PLAIN = Object.freeze({ kind: "not-plain" });
var SYMBOL_KEYS = Object.freeze({ kind: "symbol-keys" });
var stringify_operations = {
  identify: (value) => value,
  typeOf: (value) => value === null ? "null" : typeof value,
  toPrimitive: (value) => value,
  tagOf: (value) => get_type(value),
  isThenable: (value) => typeof value.then === "function",
  toPromise: (thenable) => Promise.resolve(thenable),
  unbox: (boxed) => boxed.valueOf(),
  toISOString: (date) => isNaN(date.getDate()) ? "" : date.toISOString(),
  toStringValue: (value) => value.toString(),
  regExpInfo: (regexp) => ({ source: regexp.source, flags: regexp.flags }),
  valuesOf: (set) => set,
  entriesOf: (map) => map,
  viewInfo: (view) => ({
    buffer: view.buffer,
    byteOffset: view.byteOffset,
    byteLength: view.byteLength,
    length: view.length,
    bufferByteLength: view.buffer.byteLength
  }),
  toArrayBuffer: (buffer2) => buffer2,
  lengthOf: (array) => array.length,
  hasOwn: (value, key) => Object.hasOwn(value, key),
  indicesOf: (array) => valid_array_indices(array),
  shapeOf: (value) => {
    if (!is_plain_object(value))
      return NOT_PLAIN;
    if (enumerable_symbols(value).length > 0)
      return SYMBOL_KEYS;
    return {
      kind: Object.getPrototypeOf(value) === null ? "null-proto" : "plain",
      keys: Object.keys(value)
    };
  },
  get: (value, key) => value[key]
};
var default_stringify_operations = Object.freeze(stringify_operations);
var parse_operations = {
  fromPrimitive: (primitive) => primitive,
  fromISOString: (iso) => new Date(iso),
  fromStringValue: (tag, text) => {
    if (tag === "URL")
      return new URL(text);
    if (tag === "URLSearchParams")
      return new URLSearchParams(text);
    return Temporal[tag.slice(9)].from(text);
  },
  fromArrayBuffer: (buffer2) => buffer2,
  fromRegExpInfo: (source, flags) => new RegExp(source, flags),
  fromViewInfo: (tag, buffer2, byteOffset, length) => {
    const Constructor = globalThis[tag];
    return byteOffset !== undefined ? new Constructor(buffer2, byteOffset, length) : new Constructor(buffer2);
  },
  box: (value) => Object(value),
  createArray: (length) => new Array(length),
  createSparseArray: (length) => {
    const array = [];
    array[MAX_ARRAY_INDEX] = undefined;
    delete array[MAX_ARRAY_INDEX];
    array.length = length;
    return array;
  },
  createObject: () => ({}),
  createNullPrototypeObject: () => Object.create(null),
  createSet: () => new Set,
  createMap: () => new Map,
  set: (target, key, value) => {
    target[key] = value;
  },
  addValue: (set, value) => {
    set.add(value);
  },
  addEntry: (map, key, value) => {
    map.set(key, value);
  }
};
var default_parse_operations = Object.freeze(parse_operations);

// node_modules/devalue/src/parse.js
function parse(serialized, revivers, options) {
  return unflatten(JSON.parse(serialized), revivers, options);
}
function unflatten(parsed, revivers, options) {
  const ops = merge_operations(default_parse_operations, options?.operations);
  if (typeof parsed === "number")
    return hydrate(parsed, true);
  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error("Invalid input");
  }
  const values = parsed;
  const hydrated = Array(values.length);
  let hydrating = null;
  function hydrate(index, standalone = false) {
    if (index === UNDEFINED)
      return ops.fromPrimitive(undefined);
    if (index === NAN)
      return ops.fromPrimitive(NaN);
    if (index === POSITIVE_INFINITY)
      return ops.fromPrimitive(Infinity);
    if (index === NEGATIVE_INFINITY)
      return ops.fromPrimitive(-Infinity);
    if (index === NEGATIVE_ZERO)
      return ops.fromPrimitive(-0);
    if (standalone || typeof index !== "number") {
      throw new Error(`Invalid input`);
    }
    if (index in hydrated)
      return hydrated[index];
    if (index >= values.length) {
      throw new Error(`Invalid input`);
    }
    const value = values[index];
    if (!value || typeof value !== "object") {
      hydrated[index] = ops.fromPrimitive(value);
    } else if (Array.isArray(value)) {
      if (typeof value[0] === "string") {
        const type = value[0];
        const reviver = revivers && Object.hasOwn(revivers, type) ? revivers[type] : undefined;
        if (reviver) {
          let i = value[1];
          if (typeof i !== "number") {
            i = values.push(value[1]) - 1;
          }
          if (Object.hasOwn(hydrated, i)) {
            return hydrated[index] = reviver(hydrated[i]);
          }
          hydrating ??= new Set;
          if (hydrating.has(i)) {
            throw new Error("Invalid circular reference");
          }
          hydrating.add(i);
          hydrated[index] = reviver(hydrate(i));
          hydrating.delete(i);
          return hydrated[index];
        }
        switch (type) {
          case "Date":
            hydrated[index] = ops.fromISOString(value[1]);
            break;
          case "Set":
            const set = ops.createSet();
            hydrated[index] = set;
            for (let i = 1;i < value.length; i += 1) {
              ops.addValue(set, hydrate(value[i]));
            }
            break;
          case "Map":
            const map = ops.createMap();
            hydrated[index] = map;
            for (let i = 1;i < value.length; i += 2) {
              ops.addEntry(map, hydrate(value[i]), hydrate(value[i + 1]));
            }
            break;
          case "RegExp":
            hydrated[index] = ops.fromRegExpInfo(value[1], value[2]);
            break;
          case "Object": {
            const wrapped_index = value[1];
            if (typeof values[wrapped_index] === "object" && values[wrapped_index][0] !== "BigInt") {
              throw new Error("Invalid input");
            }
            hydrated[index] = ops.box(hydrate(wrapped_index));
            break;
          }
          case "BigInt":
            hydrated[index] = ops.fromPrimitive(BigInt(value[1]));
            break;
          case "null":
            const obj = ops.createNullPrototypeObject();
            hydrated[index] = obj;
            for (let i = 1;i < value.length; i += 2) {
              if (value[i] === "__proto__") {
                throw new Error("Cannot parse an object with a `__proto__` property");
              }
              ops.set(obj, value[i], hydrate(value[i + 1]));
            }
            break;
          case "Int8Array":
          case "Uint8Array":
          case "Uint8ClampedArray":
          case "Int16Array":
          case "Uint16Array":
          case "Float16Array":
          case "Int32Array":
          case "Uint32Array":
          case "Float32Array":
          case "Float64Array":
          case "BigInt64Array":
          case "BigUint64Array":
          case "DataView": {
            if (values[value[1]][0] !== "ArrayBuffer") {
              throw new Error("Invalid data");
            }
            const buffer2 = hydrate(value[1]);
            hydrated[index] = ops.fromViewInfo(type, buffer2, value[2], value[3]);
            break;
          }
          case "ArrayBuffer": {
            const base64 = value[1];
            if (typeof base64 !== "string") {
              throw new Error("Invalid ArrayBuffer encoding");
            }
            hydrated[index] = ops.fromArrayBuffer(decode64(base64));
            break;
          }
          case "URL":
          case "URLSearchParams":
          case "Temporal.Duration":
          case "Temporal.Instant":
          case "Temporal.PlainDate":
          case "Temporal.PlainTime":
          case "Temporal.PlainDateTime":
          case "Temporal.PlainMonthDay":
          case "Temporal.PlainYearMonth":
          case "Temporal.ZonedDateTime": {
            hydrated[index] = ops.fromStringValue(type, value[1]);
            break;
          }
          default:
            throw new Error(`Unknown type ${type}`);
        }
      } else if (value[0] === SPARSE) {
        const len = value[1];
        if (!is_valid_array_len(len)) {
          throw new Error("Invalid input");
        }
        const array = ops.createSparseArray(len);
        hydrated[index] = array;
        for (let i = 2;i < value.length; i += 2) {
          const idx = value[i];
          if (!is_valid_array_index(idx) || idx >= len) {
            throw new Error("Invalid input");
          }
          ops.set(array, idx, hydrate(value[i + 1]));
        }
      } else {
        const array = ops.createArray(value.length);
        hydrated[index] = array;
        for (let i = 0;i < value.length; i += 1) {
          const n = value[i];
          if (n === HOLE)
            continue;
          ops.set(array, i, hydrate(n));
        }
      }
    } else {
      const object = ops.createObject();
      hydrated[index] = object;
      for (const key of Object.keys(value)) {
        if (key === "__proto__") {
          throw new Error("Cannot parse an object with a `__proto__` property");
        }
        ops.set(object, key, hydrate(value[key]));
      }
    }
    return hydrated[index];
  }
  return hydrate(0);
}
// node_modules/devalue/src/stringify.js
function stringify(value, reducers, options) {
  const stringified = run(false, value, reducers, options);
  return typeof stringified === "string" ? stringified : `[${stringified.join(",")}]`;
}
function run(async, value, reducers, options) {
  const ops = merge_operations(default_stringify_operations, options?.operations);
  const stringified = [];
  const indexes = new Map;
  const custom = [];
  if (reducers) {
    for (const key of Object.getOwnPropertyNames(reducers)) {
      custom.push({ key, fn: reducers[key] });
    }
  }
  const keys = [];
  let p = 0;
  function flatten(thing, index2) {
    const type = ops.typeOf(thing);
    if (type === "undefined")
      return UNDEFINED;
    let number;
    if (type === "number") {
      number = ops.toPrimitive(thing);
      if (Number.isNaN(number))
        return NAN;
      if (number === Infinity)
        return POSITIVE_INFINITY;
      if (number === -Infinity)
        return NEGATIVE_INFINITY;
      if (number === 0 && 1 / number < 0)
        return NEGATIVE_ZERO;
    }
    const id = ops.identify(thing);
    if (indexes.has(id))
      return indexes.get(id);
    index2 ??= p++;
    indexes.set(id, index2);
    for (const { key, fn } of custom) {
      const value2 = fn(thing);
      if (value2) {
        stringified[index2] = `["${key}",${flatten(value2)}]`;
        return index2;
      }
    }
    if (type === "function") {
      throw new DevalueError(`Cannot stringify a function`, keys, thing, value);
    } else if (type === "symbol") {
      throw new DevalueError(`Cannot stringify a Symbol primitive`, keys, thing, value);
    }
    let str = "";
    if (type !== "object") {
      str = stringify_primitive2(type === "number" ? number : ops.toPrimitive(thing));
    } else if (ops.isThenable(thing)) {
      if (!async) {
        throw new DevalueError(`Cannot stringify a Promise or thenable \u2014 use stringifyAsync instead`, keys, thing, value);
      }
      str = ops.toPromise(thing).then((value2) => {
        const i = flatten(value2, index2);
        if (i < 0)
          stringified[index2] = i;
      });
    } else {
      const tag = ops.tagOf(thing);
      switch (tag) {
        case "Number":
        case "String":
        case "Boolean":
        case "BigInt":
          str = `["Object",${flatten(ops.unbox(thing))}]`;
          break;
        case "Date":
          str = `["Date","${ops.toISOString(thing)}"]`;
          break;
        case "URL":
          str = `["URL",${stringify_string(ops.toStringValue(thing))}]`;
          break;
        case "URLSearchParams":
          str = `["URLSearchParams",${stringify_string(ops.toStringValue(thing))}]`;
          break;
        case "RegExp":
          const { source, flags } = ops.regExpInfo(thing);
          str = flags ? `["RegExp",${stringify_string(source)},"${flags}"]` : `["RegExp",${stringify_string(source)}]`;
          break;
        case "Array": {
          let mostly_dense = false;
          const length = ops.lengthOf(thing);
          str = "[";
          for (let i = 0;i < length; i += 1) {
            if (i > 0)
              str += ",";
            if (ops.hasOwn(thing, i)) {
              keys.push(`[${i}]`);
              str += flatten(ops.get(thing, i));
              keys.pop();
            } else if (mostly_dense) {
              str += HOLE;
            } else {
              const populated_keys = ops.indicesOf(thing);
              const population = populated_keys.length;
              const d = String(length).length;
              const hole_cost = (length - population) * 3;
              const sparse_cost = 4 + d + population * (d + 1);
              if (hole_cost > sparse_cost) {
                str = "[" + SPARSE + "," + length;
                for (let j = 0;j < populated_keys.length; j++) {
                  const key = populated_keys[j];
                  keys.push(`[${key}]`);
                  str += "," + key + "," + flatten(ops.get(thing, key));
                  keys.pop();
                }
                break;
              } else {
                mostly_dense = true;
                str += HOLE;
              }
            }
          }
          str += "]";
          break;
        }
        case "Set":
          str = '["Set"';
          for (const value2 of ops.valuesOf(thing)) {
            str += `,${flatten(value2)}`;
          }
          str += "]";
          break;
        case "Map":
          str = '["Map"';
          for (const [key, value2] of ops.entriesOf(thing)) {
            const key_type = ops.typeOf(key);
            const key_is_primitive = key_type !== "object" && key_type !== "function" && key_type !== "symbol";
            keys.push(`.get(${key_is_primitive ? stringify_primitive2(ops.toPrimitive(key)) : "..."})`);
            str += `,${flatten(key)},${flatten(value2)}`;
            keys.pop();
          }
          str += "]";
          break;
        case "Int8Array":
        case "Uint8Array":
        case "Uint8ClampedArray":
        case "Int16Array":
        case "Uint16Array":
        case "Float16Array":
        case "Int32Array":
        case "Uint32Array":
        case "Float32Array":
        case "Float64Array":
        case "BigInt64Array":
        case "BigUint64Array": {
          const info = ops.viewInfo(thing);
          str = '["' + tag + '",' + flatten(info.buffer);
          if (info.byteLength !== info.bufferByteLength) {
            str += `,${info.byteOffset},${info.length}`;
          }
          str += "]";
          break;
        }
        case "DataView": {
          const info = ops.viewInfo(thing);
          str = '["' + tag + '",' + flatten(info.buffer);
          if (info.byteLength !== info.bufferByteLength) {
            str += `,${info.byteOffset},${info.byteLength}`;
          }
          str += "]";
          break;
        }
        case "ArrayBuffer": {
          const base64 = encode64(ops.toArrayBuffer(thing));
          str = `["ArrayBuffer","${base64}"]`;
          break;
        }
        case "Temporal.Duration":
        case "Temporal.Instant":
        case "Temporal.PlainDate":
        case "Temporal.PlainTime":
        case "Temporal.PlainDateTime":
        case "Temporal.PlainMonthDay":
        case "Temporal.PlainYearMonth":
        case "Temporal.ZonedDateTime":
          str = `["${tag}",${stringify_string(ops.toStringValue(thing))}]`;
          break;
        default: {
          const shape = ops.shapeOf(thing);
          if (shape.kind === "not-plain") {
            throw new DevalueError(`Cannot stringify arbitrary non-POJOs`, keys, thing, value);
          }
          if (shape.kind === "symbol-keys") {
            throw new DevalueError(`Cannot stringify POJOs with symbolic keys`, keys, thing, value);
          }
          if (shape.kind === "null-proto") {
            str = '["null"';
            for (const key of shape.keys) {
              if (key === "__proto__") {
                throw new DevalueError(`Cannot stringify objects with __proto__ keys`, keys, thing, value);
              }
              keys.push(stringify_key(key));
              str += `,${stringify_string(key)},${flatten(ops.get(thing, key))}`;
              keys.pop();
            }
            str += "]";
          } else {
            str = "{";
            let started = false;
            for (const key of shape.keys) {
              if (key === "__proto__") {
                throw new DevalueError(`Cannot stringify objects with __proto__ keys`, keys, thing, value);
              }
              if (started)
                str += ",";
              started = true;
              keys.push(stringify_key(key));
              str += `${stringify_string(key)}:${flatten(ops.get(thing, key))}`;
              keys.pop();
            }
            str += "}";
          }
        }
      }
    }
    stringified[index2] = str;
    return index2;
  }
  const index = flatten(value);
  if (index < 0)
    return `${index}`;
  return stringified;
}
function stringify_primitive2(thing) {
  const type = typeof thing;
  if (type === "string")
    return stringify_string(thing);
  if (thing === undefined)
    return UNDEFINED.toString();
  if (thing === 0 && 1 / thing < 0)
    return NEGATIVE_ZERO.toString();
  if (type === "bigint")
    return `["BigInt","${thing}"]`;
  return String(thing);
}
// node_modules/clsx/dist/clsx.mjs
function r(e) {
  var t, f, n = "";
  if (typeof e == "string" || typeof e == "number")
    n += e;
  else if (typeof e == "object")
    if (Array.isArray(e)) {
      var o = e.length;
      for (t = 0;t < o; t++)
        e[t] && (f = r(e[t])) && (n && (n += " "), n += f);
    } else
      for (f in e)
        e[f] && (n && (n += " "), n += f);
  return n;
}
function clsx() {
  for (var e, t, f = 0, n = "", o = arguments.length;f < o; f++)
    (e = arguments[f]) && (t = r(e)) && (n && (n += " "), n += t);
  return n;
}

// .svelte-kit/output/server/chunks/server2.js
var UNINITIALIZED = Symbol("uninitialized");
var ATTR_REGEX = /[&"<]/g;
var CONTENT_REGEX = /[&<]/g;
function escape_html(value, is_attr) {
  const str = String(value ?? "");
  const pattern = is_attr ? ATTR_REGEX : CONTENT_REGEX;
  pattern.lastIndex = 0;
  let escaped2 = "";
  let last = 0;
  while (pattern.test(str)) {
    const i = pattern.lastIndex - 1;
    const ch = str[i];
    escaped2 += str.substring(last, i) + (ch === "&" ? "&amp;" : ch === '"' ? "&quot;" : "&lt;");
    last = i + 1;
  }
  return escaped2 + str.substring(last);
}
var is_array = Array.isArray;
Array.prototype.indexOf;
Array.prototype.includes;
Array.prototype;
var has_own_property = Object.prototype.hasOwnProperty;
var noop = () => {};
function deferred() {
  var resolve;
  var reject;
  return {
    promise: new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    }),
    resolve,
    reject
  };
}
var replacements = { translate: /* @__PURE__ */ new Map([[true, "yes"], [false, "no"]]) };
function attr(name, value, is_boolean = false) {
  if (name === "hidden" && value !== "until-found")
    is_boolean = true;
  if (value == null || is_boolean && !value && value !== "")
    return "";
  const normalized = has_own_property.call(replacements, name) && replacements[name].get(value) || value;
  return ` ${name}${is_boolean ? `=""` : `="${escape_html(normalized, true)}"`}`;
}
function clsx$1(value) {
  if (typeof value === "object")
    return clsx(value);
  else
    return value ?? "";
}
var whitespace = [...` 	
\r\f\xA0\v\uFEFF`];
function to_class(value, hash, directives) {
  var classname = value == null ? "" : "" + value;
  if (hash)
    classname = classname ? classname + " " + hash : hash;
  if (directives) {
    for (var key of Object.keys(directives))
      if (directives[key])
        classname = classname ? classname + " " + key : key;
      else if (classname.length) {
        var len = key.length;
        var a = 0;
        while ((a = classname.indexOf(key, a)) >= 0) {
          var b = a + len;
          if ((a === 0 || whitespace.includes(classname[a - 1])) && (b === classname.length || whitespace.includes(classname[b])))
            classname = (a === 0 ? "" : classname.substring(0, a)) + classname.substring(b + 1);
          else
            a = b;
        }
      }
  }
  return classname === "" ? null : classname;
}
function append_styles(styles, important = false) {
  var separator = important ? " !important;" : ";";
  var css = "";
  for (var key of Object.keys(styles)) {
    var value = styles[key];
    if (value != null && value !== "")
      css += " " + key + ": " + value + separator;
  }
  return css;
}
function to_css_name(name) {
  if (name[0] !== "-" || name[1] !== "-")
    return name.toLowerCase();
  return name;
}
function to_style(value, styles) {
  if (styles) {
    var new_style = "";
    var normal_styles;
    var important_styles;
    if (Array.isArray(styles)) {
      normal_styles = styles[0];
      important_styles = styles[1];
    } else
      normal_styles = styles;
    if (value) {
      value = String(value).replaceAll(/\/\*.*?\*\//g, "").trim();
      var in_str = false;
      var in_apo = 0;
      var in_comment = false;
      var reserved_names = [];
      if (normal_styles)
        reserved_names.push(...Object.keys(normal_styles).map(to_css_name));
      if (important_styles)
        reserved_names.push(...Object.keys(important_styles).map(to_css_name));
      var start_index = 0;
      var name_index = -1;
      const len = value.length;
      for (var i = 0;i < len; i++) {
        var c = value[i];
        if (in_comment) {
          if (c === "/" && value[i - 1] === "*")
            in_comment = false;
        } else if (in_str) {
          if (in_str === c)
            in_str = false;
        } else if (c === "/" && value[i + 1] === "*")
          in_comment = true;
        else if (c === '"' || c === "'")
          in_str = c;
        else if (c === "(")
          in_apo++;
        else if (c === ")")
          in_apo--;
        if (!in_comment && in_str === false && in_apo === 0) {
          if (c === ":" && name_index === -1)
            name_index = i;
          else if (c === ";" || i === len - 1) {
            if (name_index !== -1) {
              var name = to_css_name(value.substring(start_index, name_index).trim());
              if (!reserved_names.includes(name)) {
                if (c !== ";")
                  i++;
                var property = value.substring(start_index, i).trim();
                new_style += " " + property + ";";
              }
            }
            start_index = i + 1;
            name_index = -1;
          }
        }
      }
    }
    if (normal_styles)
      new_style += append_styles(normal_styles);
    if (important_styles)
      new_style += append_styles(important_styles, true);
    new_style = new_style.trim();
    return new_style === "" ? null : new_style;
  }
  return value == null ? null : String(value);
}
var CLEAN = 1024;
var DIRTY = 2048;
var MAYBE_DIRTY = 4096;
var STALE_REACTION = new class StaleReactionError extends Error {
  name = "StaleReactionError";
  message = "The reaction that called `getAbortSignal()` was re-run or destroyed";
};
globalThis.document?.contentType;
function lifecycle_outside_component(name) {
  throw new Error(`https://svelte.dev/e/lifecycle_outside_component`);
}
var async_mode_flag = false;
function get_parent_context(context) {
  let parent = context.p;
  while (parent !== null && parent.c === null)
    parent = parent.p;
  return parent?.c ?? null;
}
function get_or_init_context_map(context, name) {
  if (context === null)
    lifecycle_outside_component(name);
  return context.c ??= new Map(get_parent_context(context) || undefined);
}
~(DIRTY | MAYBE_DIRTY | CLEAN);
var BLOCK_OPEN = `<!--[-->`;
var BLOCK_CLOSE = `<!--]-->`;
var DOM_BOOLEAN_ATTRIBUTES = [
  "allowfullscreen",
  "async",
  "autofocus",
  "autoplay",
  "checked",
  "controls",
  "default",
  "disabled",
  "formnovalidate",
  "indeterminate",
  "inert",
  "ismap",
  "loop",
  "multiple",
  "muted",
  "nomodule",
  "novalidate",
  "open",
  "playsinline",
  "readonly",
  "required",
  "reversed",
  "seamless",
  "selected",
  "webkitdirectory",
  "defer",
  "disablepictureinpicture",
  "disableremoteplayback"
];
function is_boolean_attribute(name) {
  return DOM_BOOLEAN_ATTRIBUTES.includes(name);
}
[...DOM_BOOLEAN_ATTRIBUTES];
var ssr_context = null;
function set_ssr_context(v) {
  ssr_context = v;
}
function getContext(key) {
  return get_or_init_context_map(ssr_context, "getContext").get(key);
}
function hasContext(key) {
  return get_or_init_context_map(ssr_context, "hasContext").has(key);
}
function push(fn) {
  ssr_context = {
    p: ssr_context,
    c: null,
    r: null
  };
}
function pop() {
  ssr_context = ssr_context.p;
}
function async_local_storage_unavailable() {
  const error = /* @__PURE__ */ new Error(`async_local_storage_unavailable
The node API \`AsyncLocalStorage\` is not available, but is required to use async server rendering.
https://svelte.dev/e/async_local_storage_unavailable`);
  error.name = "Svelte error";
  throw error;
}
function await_invalid() {
  const error = /* @__PURE__ */ new Error(`await_invalid
Encountered asynchronous work while rendering synchronously.
https://svelte.dev/e/await_invalid`);
  error.name = "Svelte error";
  throw error;
}
function html_deprecated() {
  const error = /* @__PURE__ */ new Error(`html_deprecated
The \`html\` property of server render results has been deprecated. Use \`body\` instead.
https://svelte.dev/e/html_deprecated`);
  error.name = "Svelte error";
  throw error;
}
function invalid_csp() {
  const error = /* @__PURE__ */ new Error(`invalid_csp
\`csp.nonce\` was set while \`csp.hash\` was \`true\`. These options cannot be used simultaneously.
https://svelte.dev/e/invalid_csp`);
  error.name = "Svelte error";
  throw error;
}
function invalid_id_prefix() {
  const error = /* @__PURE__ */ new Error(`invalid_id_prefix
The \`idPrefix\` option cannot include \`--\`.
https://svelte.dev/e/invalid_id_prefix`);
  error.name = "Svelte error";
  throw error;
}
function server_context_required() {
  const error = /* @__PURE__ */ new Error(`server_context_required
Could not resolve \`render\` context.
https://svelte.dev/e/server_context_required`);
  error.name = "Svelte error";
  throw error;
}
function unresolved_hydratable(key, stack) {
  console.warn(`https://svelte.dev/e/unresolved_hydratable`);
}
var current_render = null;
var context = null;
function get_render_context() {
  const store = context ?? als?.getStore();
  if (!store)
    server_context_required();
  return store;
}
async function with_render_context(fn) {
  context = { hydratable: {
    lookup: /* @__PURE__ */ new Map,
    comparisons: [],
    unresolved_promises: /* @__PURE__ */ new Map
  } };
  if (in_webcontainer()) {
    const { promise, resolve } = deferred();
    const previous_render = current_render;
    current_render = promise;
    await previous_render;
    return fn().finally(resolve);
  }
  try {
    if (als === null)
      async_local_storage_unavailable();
    return als.run(context, fn);
  } finally {
    context = null;
  }
}
var als = null;
var als_import = null;
function init_render_context() {
  als_import ??= import("async_hooks").then((hooks) => {
    als = new hooks.AsyncLocalStorage;
  }).then(noop, noop);
  return als_import;
}
function in_webcontainer() {
  return !!globalThis.process?.versions?.webcontainer;
}
var text_encoder;
var crypto;
var obfuscated_import = (module_name) => import(module_name);
async function sha256(data) {
  text_encoder ??= new TextEncoder;
  crypto ??= globalThis.crypto?.subtle?.digest ? globalThis.crypto : (await obfuscated_import("node:crypto")).webcrypto;
  return base64_encode(await crypto.subtle.digest("SHA-256", text_encoder.encode(data)));
}
function base64_encode(bytes) {
  if (globalThis.Buffer)
    return globalThis.Buffer.from(bytes).toString("base64");
  let binary = "";
  for (let i = 0;i < bytes.length; i++)
    binary += String.fromCharCode(bytes[i]);
  return btoa(binary);
}
var Renderer = class Renderer2 {
  #out = [];
  #on_destroy = undefined;
  #is_component_body = false;
  #boundary = null;
  type;
  #parent;
  promise = undefined;
  global;
  local;
  constructor(global, parent) {
    this.#parent = parent;
    this.global = global;
    this.local = parent ? { ...parent.local } : {
      select_value: undefined,
      multiple: false
    };
    this.type = parent ? parent.type : "body";
  }
  head(fn) {
    const head = new Renderer2(this.global, this);
    head.type = "head";
    this.#out.push(head);
    head.child(fn);
  }
  async_block(blockers, fn) {
    this.#out.push(BLOCK_OPEN);
    this.async(blockers, fn);
    this.#out.push(BLOCK_CLOSE);
  }
  async(blockers, fn) {
    let callback = fn;
    if (blockers.length > 0) {
      const context2 = ssr_context;
      callback = (renderer) => {
        return Promise.all(blockers).then(() => {
          const previous_context = ssr_context;
          try {
            set_ssr_context(context2);
            return fn(renderer);
          } finally {
            set_ssr_context(previous_context);
          }
        });
      };
    }
    this.child(callback);
  }
  run(thunks) {
    const context2 = ssr_context;
    let promise = Promise.resolve(thunks[0]());
    const promises = [promise];
    for (const fn of thunks.slice(1)) {
      promise = promise.then(() => {
        const previous_context = ssr_context;
        set_ssr_context(context2);
        try {
          return fn();
        } finally {
          set_ssr_context(previous_context);
        }
      });
      promises.push(promise);
    }
    promise.catch(noop);
    this.promise = this.global.track(promise);
    return promises;
  }
  child_block(fn) {
    this.#out.push(BLOCK_OPEN);
    this.child(fn);
    this.#out.push(BLOCK_CLOSE);
  }
  child(fn) {
    const child = new Renderer2(this.global, this);
    this.#out.push(child);
    const parent = ssr_context;
    set_ssr_context({
      ...ssr_context,
      p: parent,
      c: null,
      r: child
    });
    const result = fn(child);
    set_ssr_context(parent);
    if (result instanceof Promise) {
      result.catch(noop);
      result.finally(() => set_ssr_context(null)).catch(noop);
      if (child.global.mode === "sync")
        await_invalid();
      child.promise = child.global.track(result);
    }
    return child;
  }
  boundary(props, children_fn) {
    const child = new Renderer2(this.global, this);
    this.#out.push(child);
    const parent_context = ssr_context;
    if (props.failed)
      child.#boundary = {
        failed: props.failed,
        transformError: this.global.transformError,
        context: parent_context
      };
    set_ssr_context({
      ...ssr_context,
      p: parent_context,
      c: null,
      r: child
    });
    try {
      const result = children_fn(child);
      set_ssr_context(parent_context);
      if (result instanceof Promise) {
        if (child.global.mode === "sync")
          await_invalid();
        result.catch(noop);
        child.promise = child.global.track(result);
      }
    } catch (error) {
      set_ssr_context(parent_context);
      const failed_snippet = props.failed;
      if (!failed_snippet)
        throw error;
      const result = this.global.transformError(error);
      child.#out.length = 0;
      child.#boundary = null;
      if (result instanceof Promise) {
        if (this.global.mode === "sync")
          await_invalid();
        child.promise = child.global.track(result.then((transformed) => {
          set_ssr_context(parent_context);
          child.#out.push(Renderer2.#serialize_failed_boundary(transformed));
          failed_snippet(child, transformed, noop);
          child.#out.push(BLOCK_CLOSE);
        }));
        child.promise.catch(noop);
      } else {
        child.#out.push(Renderer2.#serialize_failed_boundary(result));
        failed_snippet(child, result, noop);
        child.#out.push(BLOCK_CLOSE);
      }
    }
  }
  component(fn, component_fn) {
    push(component_fn);
    this.child((renderer) => {
      renderer.#is_component_body = true;
      return fn(renderer);
    });
    pop();
  }
  select(attrs, fn, css_hash, classes, styles, flags, is_rich) {
    const { value, defaultValue, ...select_attrs } = attrs;
    if (select_attrs.multiple === "")
      select_attrs.multiple = true;
    this.push(`<select${attributes(select_attrs, css_hash, classes, styles, flags)}>`);
    this.child((renderer) => {
      renderer.local.select_value = value === undefined ? defaultValue : value;
      renderer.local.multiple = !!select_attrs.multiple;
      fn(renderer);
    });
    this.push(`${is_rich ? "<!>" : ""}</select>`);
  }
  option(attrs, body, css_hash, classes, styles, flags, is_rich) {
    this.#out.push(`<option${attributes(attrs, css_hash, classes, styles, flags)}`);
    const close = (renderer, value, { head, body: body2 }) => {
      if (has_own_property.call(attrs, "value"))
        value = attrs.value;
      var select_value = this.local.select_value;
      if (this.local.multiple && is_array(select_value) ? select_value.includes(value) : value === select_value)
        renderer.#out.push(' selected=""');
      renderer.#out.push(`>${body2}${is_rich ? "<!>" : ""}</option>`);
      if (head)
        renderer.head((child) => child.push(head));
    };
    if (typeof body === "function")
      this.child((renderer) => {
        const r2 = new Renderer2(this.global, this);
        body(r2);
        if (this.global.mode === "async")
          return r2.#collect_content_async().then((content) => {
            close(renderer, content.body.replaceAll("<!---->", ""), content);
          });
        else {
          const content = r2.#collect_content();
          close(renderer, content.body.replaceAll("<!---->", ""), content);
        }
      });
    else
      close(this, body, { body: escape_html(body) });
  }
  title(fn) {
    const path = this.get_path();
    const close = (head) => {
      this.global.set_title(head, path);
    };
    this.child((renderer) => {
      const r2 = new Renderer2(renderer.global, renderer);
      fn(r2);
      if (renderer.global.mode === "async")
        return r2.#collect_content_async().then((content) => {
          close(content.head);
        });
      else {
        const content = r2.#collect_content();
        close(content.head);
      }
    });
  }
  push(content) {
    if (typeof content === "function")
      this.child(async (renderer) => renderer.push(await content()));
    else
      this.#out.push(content);
  }
  on_destroy(fn) {
    (this.#on_destroy ??= []).push(fn);
  }
  get_path() {
    return this.#parent ? [...this.#parent.get_path(), this.#parent.#out.indexOf(this)] : [];
  }
  copy() {
    const copy = new Renderer2(this.global, this.#parent);
    copy.type = this.type;
    copy.#out = this.#out.map((item) => item instanceof Renderer2 ? item.copy() : item);
    copy.promise = this.promise;
    return copy;
  }
  subsume(other) {
    if (this.global.mode !== other.global.mode)
      throw new Error("invariant: A renderer cannot switch modes. If you're seeing this, there's a compiler bug. File an issue!");
    this.local = other.local;
    this.#out = other.#out.map((item, i) => {
      const current = this.#out[i];
      if (current instanceof Renderer2 && item instanceof Renderer2) {
        current.subsume(item);
        return current;
      }
      return item;
    });
    this.promise = other.promise;
    this.type = other.type;
  }
  get length() {
    return this.#out.length;
  }
  static #serialize_failed_boundary(error) {
    return `<!--[?${JSON.stringify(error).replace(/>/g, "\\u003e").replace(/</g, "\\u003c")}-->`;
  }
  static render(component, options = {}) {
    let sync;
    let async;
    const result = {};
    Object.defineProperties(result, {
      html: { get: () => {
        return (sync ??= Renderer2.#render(component, options)).body;
      } },
      head: { get: () => {
        return (sync ??= Renderer2.#render(component, options)).head;
      } },
      body: { get: () => {
        return (sync ??= Renderer2.#render(component, options)).body;
      } },
      hashes: { value: { script: "" } },
      then: { value: (onfulfilled, onrejected) => {
        if (!async_mode_flag) {
          const result2 = sync ??= Renderer2.#render(component, options);
          const user_result = onfulfilled({
            head: result2.head,
            body: result2.body,
            html: result2.body,
            hashes: { script: [] }
          });
          return Promise.resolve(user_result);
        }
        async ??= init_render_context().then(() => with_render_context(() => Renderer2.#render_async(component, options)));
        return async.then((result2) => {
          Object.defineProperty(result2, "html", { get: () => {
            html_deprecated();
          } });
          return onfulfilled(result2);
        }, onrejected);
      } }
    });
    return result;
  }
  *#collect_on_destroy() {
    for (const component of this.#traverse_components())
      yield* component.#collect_ondestroy();
  }
  *#traverse_components() {
    for (const child of this.#out)
      if (typeof child !== "string")
        yield* child.#traverse_components();
    if (this.#is_component_body)
      yield this;
  }
  *#collect_ondestroy() {
    if (this.#on_destroy)
      for (const fn of this.#on_destroy)
        yield fn;
    for (const child of this.#out)
      if (child instanceof Renderer2 && !child.#is_component_body)
        yield* child.#collect_ondestroy();
  }
  #run_on_destroy(suppress_errors) {
    let first_error;
    let has_error = false;
    for (const cleanup of this.#collect_on_destroy())
      try {
        cleanup();
      } catch (error) {
        if (!suppress_errors && !has_error) {
          first_error = error;
          has_error = true;
        }
      }
    if (has_error)
      throw first_error;
  }
  static #create(mode, options) {
    if (options.idPrefix?.includes("--"))
      invalid_id_prefix();
    return new Renderer2(new SSRState(mode, options.idPrefix ? options.idPrefix + "-" : "", options.csp, options.transformError));
  }
  static #render(component, options) {
    var previous_context = ssr_context;
    const renderer = Renderer2.#create("sync", options);
    let result;
    let render_error;
    let failed = false;
    try {
      try {
        Renderer2.#open_render(renderer, component, options);
        result = Renderer2.#close_render(renderer.#collect_content(), renderer);
      } catch (error) {
        render_error = error;
        failed = true;
      }
      renderer.#run_on_destroy(failed);
      if (failed)
        throw render_error;
      return result;
    } finally {
      renderer.global.abort();
      set_ssr_context(previous_context);
    }
  }
  static async#render_async(component, options) {
    const previous_context = ssr_context;
    const renderer = Renderer2.#create("async", options);
    let result;
    let render_error;
    let failed = false;
    try {
      try {
        Renderer2.#open_render(renderer, component, options);
        const content = await renderer.#collect_content_async();
        const hydratables = await renderer.#collect_hydratables();
        if (hydratables !== null)
          content.head = hydratables + content.head;
        result = Renderer2.#close_render(content, renderer);
      } catch (error) {
        render_error = error;
        failed = true;
        renderer.global.abort();
        await renderer.global.settle();
      }
      renderer.#run_on_destroy(failed);
      if (failed)
        throw render_error;
      return result;
    } finally {
      set_ssr_context(previous_context);
      renderer.global.abort();
    }
  }
  #collect_content(content = {
    head: "",
    body: ""
  }) {
    for (const item of this.#out)
      if (typeof item === "string")
        content[this.type] += item;
      else if (item instanceof Renderer2)
        item.#collect_content(content);
    return content;
  }
  async#collect_content_async(content = {
    head: "",
    body: ""
  }) {
    await this.promise;
    for (const item of this.#out)
      if (typeof item === "string")
        content[this.type] += item;
      else if (item instanceof Renderer2) {
        if (item.#boundary) {
          const boundary_content = {
            head: "",
            body: ""
          };
          try {
            await item.#collect_content_async(boundary_content);
            content.head += boundary_content.head;
            content.body += boundary_content.body;
          } catch (error) {
            const { context: context2, failed, transformError } = item.#boundary;
            set_ssr_context(context2);
            let promise = transformError(error);
            set_ssr_context(null);
            let transformed = await promise;
            set_ssr_context(context2);
            const failed_renderer = new Renderer2(item.global, item);
            failed_renderer.type = item.type;
            failed_renderer.#out.push(Renderer2.#serialize_failed_boundary(transformed));
            failed(failed_renderer, transformed, noop);
            failed_renderer.#out.push(BLOCK_CLOSE);
            await failed_renderer.#collect_content_async(content);
          }
        } else
          await item.#collect_content_async(content);
      }
    return content;
  }
  async#collect_hydratables() {
    const ctx = get_render_context().hydratable;
    for (const [_, key] of ctx.unresolved_promises)
      unresolved_hydratable(key, ctx.lookup.get(key)?.stack ?? "<missing stack trace>");
    for (const comparison of ctx.comparisons)
      await comparison;
    return await this.#hydratable_block(ctx);
  }
  static #open_render(renderer, component, options) {
    var previous_context = ssr_context;
    try {
      set_ssr_context({
        p: null,
        c: options.context ?? null,
        r: renderer
      });
      renderer.push(BLOCK_OPEN);
      component(renderer, options.props ?? {});
      renderer.push(BLOCK_CLOSE);
    } finally {
      set_ssr_context(previous_context);
    }
  }
  static #close_render(content, renderer) {
    let head = content.head + renderer.global.get_title();
    let body = content.body;
    for (const { hash, code } of renderer.global.css)
      head += `<style id="${hash}">${code}</style>`;
    return {
      head,
      body,
      hashes: { script: renderer.global.csp.script_hashes }
    };
  }
  async#hydratable_block(ctx) {
    if (ctx.lookup.size === 0)
      return null;
    let entries = [];
    let has_promises = false;
    for (const [k, v] of ctx.lookup) {
      if (v.promises) {
        has_promises = true;
        for (const p of v.promises)
          await p;
      }
      entries.push(`[${uneval(k)},${v.serialized}]`);
    }
    let prelude = `const h = (window.__svelte ??= {}).h ??= new Map();`;
    if (has_promises)
      prelude = `const r = (v) => Promise.resolve(v);
				${prelude}`;
    const body = `
			{
				${prelude}

				for (const [k, v] of [
					${entries.join(`,
					`)}
				]) {
					h.set(k, v);
				}
			}
		`;
    let csp_attr = "";
    if (this.global.csp.nonce)
      csp_attr = ` nonce="${this.global.csp.nonce}"`;
    else if (this.global.csp.hash) {
      const hash = await sha256(body);
      this.global.csp.script_hashes.push(`sha256-${hash}`);
    }
    return `
		<script${csp_attr}>${body}</script>`;
  }
};
var SSRState = class {
  csp;
  mode;
  uid;
  css = /* @__PURE__ */ new Set;
  #pending = /* @__PURE__ */ new Set;
  #controller = null;
  #aborted = false;
  transformError;
  #title = {
    path: [],
    value: ""
  };
  constructor(mode, id_prefix = "", csp = { hash: false }, transformError) {
    this.mode = mode;
    this.csp = {
      ...csp,
      script_hashes: []
    };
    this.transformError = transformError ?? ((error) => {
      throw error;
    });
    let uid = 1;
    this.uid = () => `${id_prefix}s${uid++}`;
  }
  track(promise) {
    this.#pending.add(promise);
    promise.then(() => this.#pending.delete(promise), () => this.#pending.delete(promise));
    return promise;
  }
  async settle() {
    while (this.#pending.size > 0)
      await Promise.allSettled(this.#pending);
  }
  abort() {
    if (this.#aborted)
      return;
    this.#aborted = true;
    this.#controller?.abort(STALE_REACTION);
  }
  get_abort_signal() {
    const controller = this.#controller ??= new AbortController;
    if (this.#aborted)
      controller.abort(STALE_REACTION);
    return controller.signal;
  }
  get_title() {
    return this.#title.value;
  }
  set_title(value, path) {
    const current = this.#title.path;
    let i = 0;
    let l = Math.min(path.length, current.length);
    while (i < l && path[i] === current[i])
      i += 1;
    if (path[i] === undefined)
      return;
    if (current[i] === undefined || path[i] > current[i]) {
      this.#title.path = path;
      this.#title.value = value;
    }
  }
};
var INVALID_ATTR_NAME_CHAR_REGEX = /[\s'">/=\u{FDD0}-\u{FDEF}\u{FFFE}\u{FFFF}\u{1FFFE}\u{1FFFF}\u{2FFFE}\u{2FFFF}\u{3FFFE}\u{3FFFF}\u{4FFFE}\u{4FFFF}\u{5FFFE}\u{5FFFF}\u{6FFFE}\u{6FFFF}\u{7FFFE}\u{7FFFF}\u{8FFFE}\u{8FFFF}\u{9FFFE}\u{9FFFF}\u{AFFFE}\u{AFFFF}\u{BFFFE}\u{BFFFF}\u{CFFFE}\u{CFFFF}\u{DFFFE}\u{DFFFF}\u{EFFFE}\u{EFFFF}\u{FFFFE}\u{FFFFF}\u{10FFFE}\u{10FFFF}]/u;
function render(component, options = {}) {
  if (options.csp?.hash && options.csp.nonce)
    invalid_csp();
  return Renderer.render(component, options);
}
function attributes(attrs, css_hash, classes, styles, flags = 0) {
  if (styles)
    attrs.style = to_style(attrs.style, styles);
  if (attrs.class)
    attrs.class = clsx$1(attrs.class);
  if (css_hash || classes)
    attrs.class = to_class(attrs.class, css_hash, classes);
  let attr_str = "";
  let name;
  const is_html = (flags & 1) === 0;
  const lowercase = (flags & 2) === 0;
  const is_input = (flags & 4) !== 0;
  for (name of Object.keys(attrs)) {
    if (typeof attrs[name] === "function")
      continue;
    if (name[0] === "$" && name[1] === "$")
      continue;
    if (name === "" || INVALID_ATTR_NAME_CHAR_REGEX.test(name))
      continue;
    var value = attrs[name];
    var lower = name.toLowerCase();
    if (lowercase)
      name = lower;
    if (lower.length > 2 && lower.startsWith("on"))
      continue;
    if (is_input) {
      if (name === "defaultvalue" || name === "defaultchecked") {
        name = name === "defaultvalue" ? "value" : "checked";
        if (attrs[name])
          continue;
      }
    }
    attr_str += attr(name, value, is_html && is_boolean_attribute(name));
  }
  return attr_str;
}
function stringify2(value) {
  return typeof value === "string" ? value : value == null ? "" : value + "";
}
function attr_class(value, hash, directives) {
  var result = to_class(value, hash, directives);
  return result ? ` class="${escape_html(result, true)}"` : "";
}
function attr_style(value, directives) {
  var result = to_style(value, directives);
  return result ? ` style="${escape_html(result, true)}"` : "";
}
function bind_props(props_parent, props_now) {
  for (const key of Object.keys(props_now)) {
    const initial_value = props_parent[key];
    const value = props_now[key];
    if (initial_value === undefined && value !== undefined && Object.getOwnPropertyDescriptor(props_parent, key)?.set)
      props_parent[key] = value;
  }
}
function ensure_array_like(array_like_or_iterator) {
  if (array_like_or_iterator)
    return array_like_or_iterator.length !== undefined ? array_like_or_iterator : Array.from(array_like_or_iterator);
  return [];
}
function once(get_value) {
  let value = UNINITIALIZED;
  return () => {
    if (value === UNINITIALIZED)
      value = get_value();
    return value;
  };
}
function derived(fn) {
  const get_value = ssr_context === null ? fn : once(fn);
  let updated_value;
  return function(new_value) {
    if (arguments.length === 0)
      return updated_value ?? get_value();
    updated_value = new_value;
    return updated_value;
  };
}

export { uneval, parse, stringify, escape_html, attr, clsx$1, ssr_context, getContext, hasContext, render, attributes, stringify2 as stringify1, attr_class, attr_style, bind_props, ensure_array_like, derived };

//# debugId=2FF9F5FD27AF57CC64756E2164756E21
