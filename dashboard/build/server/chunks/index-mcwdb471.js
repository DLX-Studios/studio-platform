// @bun
import {
  addEvent,
  getMachine,
  getSetting
} from "./index-jr5xvt5y.js";
import {
  __commonJS,
  __toESM
} from "./index-qcep1gwj.js";

// node_modules/@opentelemetry/api/build/src/version.js
var require_version = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.VERSION = undefined;
  exports.VERSION = "1.9.1";
});

// node_modules/@opentelemetry/api/build/src/internal/semver.js
var require_semver = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.isCompatible = exports._makeCompatibilityCheck = undefined;
  var version_1 = require_version();
  var re = /^(\d+)\.(\d+)\.(\d+)(-(.+))?$/;
  function _makeCompatibilityCheck(ownVersion) {
    const acceptedVersions = new Set([ownVersion]);
    const rejectedVersions = new Set;
    const myVersionMatch = ownVersion.match(re);
    if (!myVersionMatch) {
      return () => false;
    }
    const ownVersionParsed = {
      major: +myVersionMatch[1],
      minor: +myVersionMatch[2],
      patch: +myVersionMatch[3],
      prerelease: myVersionMatch[4]
    };
    if (ownVersionParsed.prerelease != null) {
      return function isExactmatch(globalVersion) {
        return globalVersion === ownVersion;
      };
    }
    function _reject(v) {
      rejectedVersions.add(v);
      return false;
    }
    function _accept(v) {
      acceptedVersions.add(v);
      return true;
    }
    return function isCompatible(globalVersion) {
      if (acceptedVersions.has(globalVersion)) {
        return true;
      }
      if (rejectedVersions.has(globalVersion)) {
        return false;
      }
      const globalVersionMatch = globalVersion.match(re);
      if (!globalVersionMatch) {
        return _reject(globalVersion);
      }
      const globalVersionParsed = {
        major: +globalVersionMatch[1],
        minor: +globalVersionMatch[2],
        patch: +globalVersionMatch[3],
        prerelease: globalVersionMatch[4]
      };
      if (globalVersionParsed.prerelease != null) {
        return _reject(globalVersion);
      }
      if (ownVersionParsed.major !== globalVersionParsed.major) {
        return _reject(globalVersion);
      }
      if (ownVersionParsed.major === 0) {
        if (ownVersionParsed.minor === globalVersionParsed.minor && ownVersionParsed.patch <= globalVersionParsed.patch) {
          return _accept(globalVersion);
        }
        return _reject(globalVersion);
      }
      if (ownVersionParsed.minor <= globalVersionParsed.minor) {
        return _accept(globalVersion);
      }
      return _reject(globalVersion);
    };
  }
  exports._makeCompatibilityCheck = _makeCompatibilityCheck;
  exports.isCompatible = _makeCompatibilityCheck(version_1.VERSION);
});

// node_modules/@opentelemetry/api/build/src/internal/global-utils.js
var require_global_utils = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.unregisterGlobal = exports.getGlobal = exports.registerGlobal = undefined;
  var version_1 = require_version();
  var semver_1 = require_semver();
  var major = version_1.VERSION.split(".")[0];
  var GLOBAL_OPENTELEMETRY_API_KEY = Symbol.for(`opentelemetry.js.api.${major}`);
  var _global = typeof globalThis === "object" ? globalThis : typeof self === "object" ? self : typeof window === "object" ? window : typeof global === "object" ? global : {};
  function registerGlobal(type, instance, diag, allowOverride = false) {
    var _a;
    const api = _global[GLOBAL_OPENTELEMETRY_API_KEY] = (_a = _global[GLOBAL_OPENTELEMETRY_API_KEY]) !== null && _a !== undefined ? _a : {
      version: version_1.VERSION
    };
    if (!allowOverride && api[type]) {
      const err = new Error(`@opentelemetry/api: Attempted duplicate registration of API: ${type}`);
      diag.error(err.stack || err.message);
      return false;
    }
    if (api.version !== version_1.VERSION) {
      const err = new Error(`@opentelemetry/api: Registration of version v${api.version} for ${type} does not match previously registered API v${version_1.VERSION}`);
      diag.error(err.stack || err.message);
      return false;
    }
    api[type] = instance;
    diag.debug(`@opentelemetry/api: Registered a global for ${type} v${version_1.VERSION}.`);
    return true;
  }
  exports.registerGlobal = registerGlobal;
  function getGlobal(type) {
    var _a, _b;
    const globalVersion = (_a = _global[GLOBAL_OPENTELEMETRY_API_KEY]) === null || _a === undefined ? undefined : _a.version;
    if (!globalVersion || !(0, semver_1.isCompatible)(globalVersion)) {
      return;
    }
    return (_b = _global[GLOBAL_OPENTELEMETRY_API_KEY]) === null || _b === undefined ? undefined : _b[type];
  }
  exports.getGlobal = getGlobal;
  function unregisterGlobal(type, diag) {
    diag.debug(`@opentelemetry/api: Unregistering a global for ${type} v${version_1.VERSION}.`);
    const api = _global[GLOBAL_OPENTELEMETRY_API_KEY];
    if (api) {
      delete api[type];
    }
  }
  exports.unregisterGlobal = unregisterGlobal;
});

// node_modules/@opentelemetry/api/build/src/diag/ComponentLogger.js
var require_ComponentLogger = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.DiagComponentLogger = undefined;
  var global_utils_1 = require_global_utils();

  class DiagComponentLogger {
    constructor(props) {
      this._namespace = props.namespace || "DiagComponentLogger";
    }
    debug(...args) {
      return logProxy("debug", this._namespace, args);
    }
    error(...args) {
      return logProxy("error", this._namespace, args);
    }
    info(...args) {
      return logProxy("info", this._namespace, args);
    }
    warn(...args) {
      return logProxy("warn", this._namespace, args);
    }
    verbose(...args) {
      return logProxy("verbose", this._namespace, args);
    }
  }
  exports.DiagComponentLogger = DiagComponentLogger;
  function logProxy(funcName, namespace, args) {
    const logger = (0, global_utils_1.getGlobal)("diag");
    if (!logger) {
      return;
    }
    return logger[funcName](namespace, ...args);
  }
});

// node_modules/@opentelemetry/api/build/src/diag/types.js
var require_types = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.DiagLogLevel = undefined;
  var DiagLogLevel;
  (function(DiagLogLevel2) {
    DiagLogLevel2[DiagLogLevel2["NONE"] = 0] = "NONE";
    DiagLogLevel2[DiagLogLevel2["ERROR"] = 30] = "ERROR";
    DiagLogLevel2[DiagLogLevel2["WARN"] = 50] = "WARN";
    DiagLogLevel2[DiagLogLevel2["INFO"] = 60] = "INFO";
    DiagLogLevel2[DiagLogLevel2["DEBUG"] = 70] = "DEBUG";
    DiagLogLevel2[DiagLogLevel2["VERBOSE"] = 80] = "VERBOSE";
    DiagLogLevel2[DiagLogLevel2["ALL"] = 9999] = "ALL";
  })(DiagLogLevel = exports.DiagLogLevel || (exports.DiagLogLevel = {}));
});

// node_modules/@opentelemetry/api/build/src/diag/internal/logLevelLogger.js
var require_logLevelLogger = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.createLogLevelDiagLogger = undefined;
  var types_1 = require_types();
  function createLogLevelDiagLogger(maxLevel, logger) {
    if (maxLevel < types_1.DiagLogLevel.NONE) {
      maxLevel = types_1.DiagLogLevel.NONE;
    } else if (maxLevel > types_1.DiagLogLevel.ALL) {
      maxLevel = types_1.DiagLogLevel.ALL;
    }
    logger = logger || {};
    function _filterFunc(funcName, theLevel) {
      const theFunc = logger[funcName];
      if (typeof theFunc === "function" && maxLevel >= theLevel) {
        return theFunc.bind(logger);
      }
      return function() {};
    }
    return {
      error: _filterFunc("error", types_1.DiagLogLevel.ERROR),
      warn: _filterFunc("warn", types_1.DiagLogLevel.WARN),
      info: _filterFunc("info", types_1.DiagLogLevel.INFO),
      debug: _filterFunc("debug", types_1.DiagLogLevel.DEBUG),
      verbose: _filterFunc("verbose", types_1.DiagLogLevel.VERBOSE)
    };
  }
  exports.createLogLevelDiagLogger = createLogLevelDiagLogger;
});

// node_modules/@opentelemetry/api/build/src/api/diag.js
var require_diag = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.DiagAPI = undefined;
  var ComponentLogger_1 = require_ComponentLogger();
  var logLevelLogger_1 = require_logLevelLogger();
  var types_1 = require_types();
  var global_utils_1 = require_global_utils();
  var API_NAME = "diag";

  class DiagAPI {
    static instance() {
      if (!this._instance) {
        this._instance = new DiagAPI;
      }
      return this._instance;
    }
    constructor() {
      function _logProxy(funcName) {
        return function(...args) {
          const logger = (0, global_utils_1.getGlobal)("diag");
          if (!logger)
            return;
          return logger[funcName](...args);
        };
      }
      const self2 = this;
      const setLogger = (logger, optionsOrLogLevel = { logLevel: types_1.DiagLogLevel.INFO }) => {
        var _a, _b, _c;
        if (logger === self2) {
          const err = new Error("Cannot use diag as the logger for itself. Please use a DiagLogger implementation like ConsoleDiagLogger or a custom implementation");
          self2.error((_a = err.stack) !== null && _a !== undefined ? _a : err.message);
          return false;
        }
        if (typeof optionsOrLogLevel === "number") {
          optionsOrLogLevel = {
            logLevel: optionsOrLogLevel
          };
        }
        const oldLogger = (0, global_utils_1.getGlobal)("diag");
        const newLogger = (0, logLevelLogger_1.createLogLevelDiagLogger)((_b = optionsOrLogLevel.logLevel) !== null && _b !== undefined ? _b : types_1.DiagLogLevel.INFO, logger);
        if (oldLogger && !optionsOrLogLevel.suppressOverrideMessage) {
          const stack = (_c = new Error().stack) !== null && _c !== undefined ? _c : "<failed to generate stacktrace>";
          oldLogger.warn(`Current logger will be overwritten from ${stack}`);
          newLogger.warn(`Current logger will overwrite one already registered from ${stack}`);
        }
        return (0, global_utils_1.registerGlobal)("diag", newLogger, self2, true);
      };
      self2.setLogger = setLogger;
      self2.disable = () => {
        (0, global_utils_1.unregisterGlobal)(API_NAME, self2);
      };
      self2.createComponentLogger = (options) => {
        return new ComponentLogger_1.DiagComponentLogger(options);
      };
      self2.verbose = _logProxy("verbose");
      self2.debug = _logProxy("debug");
      self2.info = _logProxy("info");
      self2.warn = _logProxy("warn");
      self2.error = _logProxy("error");
    }
  }
  exports.DiagAPI = DiagAPI;
});

// node_modules/@opentelemetry/api/build/src/baggage/internal/baggage-impl.js
var require_baggage_impl = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.BaggageImpl = undefined;

  class BaggageImpl {
    constructor(entries) {
      this._entries = entries ? new Map(entries) : new Map;
    }
    getEntry(key) {
      const entry = this._entries.get(key);
      if (!entry) {
        return;
      }
      return Object.assign({}, entry);
    }
    getAllEntries() {
      return Array.from(this._entries.entries());
    }
    setEntry(key, entry) {
      const newBaggage = new BaggageImpl(this._entries);
      newBaggage._entries.set(key, entry);
      return newBaggage;
    }
    removeEntry(key) {
      const newBaggage = new BaggageImpl(this._entries);
      newBaggage._entries.delete(key);
      return newBaggage;
    }
    removeEntries(...keys) {
      const newBaggage = new BaggageImpl(this._entries);
      for (const key of keys) {
        newBaggage._entries.delete(key);
      }
      return newBaggage;
    }
    clear() {
      return new BaggageImpl;
    }
  }
  exports.BaggageImpl = BaggageImpl;
});

// node_modules/@opentelemetry/api/build/src/baggage/internal/symbol.js
var require_symbol = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.baggageEntryMetadataSymbol = undefined;
  exports.baggageEntryMetadataSymbol = Symbol("BaggageEntryMetadata");
});

// node_modules/@opentelemetry/api/build/src/baggage/utils.js
var require_utils = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.baggageEntryMetadataFromString = exports.createBaggage = undefined;
  var diag_1 = require_diag();
  var baggage_impl_1 = require_baggage_impl();
  var symbol_1 = require_symbol();
  var diag = diag_1.DiagAPI.instance();
  function createBaggage(entries = {}) {
    return new baggage_impl_1.BaggageImpl(new Map(Object.entries(entries)));
  }
  exports.createBaggage = createBaggage;
  function baggageEntryMetadataFromString(str) {
    if (typeof str !== "string") {
      diag.error(`Cannot create baggage metadata from unknown type: ${typeof str}`);
      str = "";
    }
    return {
      __TYPE__: symbol_1.baggageEntryMetadataSymbol,
      toString() {
        return str;
      }
    };
  }
  exports.baggageEntryMetadataFromString = baggageEntryMetadataFromString;
});

// node_modules/@opentelemetry/api/build/src/context/context.js
var require_context = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.ROOT_CONTEXT = exports.createContextKey = undefined;
  function createContextKey(description) {
    return Symbol.for(description);
  }
  exports.createContextKey = createContextKey;

  class BaseContext {
    constructor(parentContext) {
      const self2 = this;
      self2._currentContext = parentContext ? new Map(parentContext) : new Map;
      self2.getValue = (key) => self2._currentContext.get(key);
      self2.setValue = (key, value) => {
        const context = new BaseContext(self2._currentContext);
        context._currentContext.set(key, value);
        return context;
      };
      self2.deleteValue = (key) => {
        const context = new BaseContext(self2._currentContext);
        context._currentContext.delete(key);
        return context;
      };
    }
  }
  exports.ROOT_CONTEXT = new BaseContext;
});

// node_modules/@opentelemetry/api/build/src/diag/consoleLogger.js
var require_consoleLogger = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.DiagConsoleLogger = exports._originalConsoleMethods = undefined;
  var consoleMap = [
    { n: "error", c: "error" },
    { n: "warn", c: "warn" },
    { n: "info", c: "info" },
    { n: "debug", c: "debug" },
    { n: "verbose", c: "trace" }
  ];
  exports._originalConsoleMethods = {};
  if (typeof console !== "undefined") {
    const keys = [
      "error",
      "warn",
      "info",
      "debug",
      "trace",
      "log"
    ];
    for (const key of keys) {
      if (typeof console[key] === "function") {
        exports._originalConsoleMethods[key] = console[key];
      }
    }
  }

  class DiagConsoleLogger {
    constructor() {
      function _consoleFunc(funcName) {
        return function(...args) {
          let theFunc = exports._originalConsoleMethods[funcName];
          if (typeof theFunc !== "function") {
            theFunc = exports._originalConsoleMethods["log"];
          }
          if (typeof theFunc !== "function" && console) {
            theFunc = console[funcName];
            if (typeof theFunc !== "function") {
              theFunc = console.log;
            }
          }
          if (typeof theFunc === "function") {
            return theFunc.apply(console, args);
          }
        };
      }
      for (let i = 0;i < consoleMap.length; i++) {
        this[consoleMap[i].n] = _consoleFunc(consoleMap[i].c);
      }
    }
  }
  exports.DiagConsoleLogger = DiagConsoleLogger;
});

// node_modules/@opentelemetry/api/build/src/metrics/NoopMeter.js
var require_NoopMeter = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.createNoopMeter = exports.NOOP_OBSERVABLE_UP_DOWN_COUNTER_METRIC = exports.NOOP_OBSERVABLE_GAUGE_METRIC = exports.NOOP_OBSERVABLE_COUNTER_METRIC = exports.NOOP_UP_DOWN_COUNTER_METRIC = exports.NOOP_HISTOGRAM_METRIC = exports.NOOP_GAUGE_METRIC = exports.NOOP_COUNTER_METRIC = exports.NOOP_METER = exports.NoopObservableUpDownCounterMetric = exports.NoopObservableGaugeMetric = exports.NoopObservableCounterMetric = exports.NoopObservableMetric = exports.NoopHistogramMetric = exports.NoopGaugeMetric = exports.NoopUpDownCounterMetric = exports.NoopCounterMetric = exports.NoopMetric = exports.NoopMeter = undefined;

  class NoopMeter {
    constructor() {}
    createGauge(_name, _options) {
      return exports.NOOP_GAUGE_METRIC;
    }
    createHistogram(_name, _options) {
      return exports.NOOP_HISTOGRAM_METRIC;
    }
    createCounter(_name, _options) {
      return exports.NOOP_COUNTER_METRIC;
    }
    createUpDownCounter(_name, _options) {
      return exports.NOOP_UP_DOWN_COUNTER_METRIC;
    }
    createObservableGauge(_name, _options) {
      return exports.NOOP_OBSERVABLE_GAUGE_METRIC;
    }
    createObservableCounter(_name, _options) {
      return exports.NOOP_OBSERVABLE_COUNTER_METRIC;
    }
    createObservableUpDownCounter(_name, _options) {
      return exports.NOOP_OBSERVABLE_UP_DOWN_COUNTER_METRIC;
    }
    addBatchObservableCallback(_callback, _observables) {}
    removeBatchObservableCallback(_callback) {}
  }
  exports.NoopMeter = NoopMeter;

  class NoopMetric {
  }
  exports.NoopMetric = NoopMetric;

  class NoopCounterMetric extends NoopMetric {
    add(_value, _attributes) {}
  }
  exports.NoopCounterMetric = NoopCounterMetric;

  class NoopUpDownCounterMetric extends NoopMetric {
    add(_value, _attributes) {}
  }
  exports.NoopUpDownCounterMetric = NoopUpDownCounterMetric;

  class NoopGaugeMetric extends NoopMetric {
    record(_value, _attributes) {}
  }
  exports.NoopGaugeMetric = NoopGaugeMetric;

  class NoopHistogramMetric extends NoopMetric {
    record(_value, _attributes) {}
  }
  exports.NoopHistogramMetric = NoopHistogramMetric;

  class NoopObservableMetric {
    addCallback(_callback) {}
    removeCallback(_callback) {}
  }
  exports.NoopObservableMetric = NoopObservableMetric;

  class NoopObservableCounterMetric extends NoopObservableMetric {
  }
  exports.NoopObservableCounterMetric = NoopObservableCounterMetric;

  class NoopObservableGaugeMetric extends NoopObservableMetric {
  }
  exports.NoopObservableGaugeMetric = NoopObservableGaugeMetric;

  class NoopObservableUpDownCounterMetric extends NoopObservableMetric {
  }
  exports.NoopObservableUpDownCounterMetric = NoopObservableUpDownCounterMetric;
  exports.NOOP_METER = new NoopMeter;
  exports.NOOP_COUNTER_METRIC = new NoopCounterMetric;
  exports.NOOP_GAUGE_METRIC = new NoopGaugeMetric;
  exports.NOOP_HISTOGRAM_METRIC = new NoopHistogramMetric;
  exports.NOOP_UP_DOWN_COUNTER_METRIC = new NoopUpDownCounterMetric;
  exports.NOOP_OBSERVABLE_COUNTER_METRIC = new NoopObservableCounterMetric;
  exports.NOOP_OBSERVABLE_GAUGE_METRIC = new NoopObservableGaugeMetric;
  exports.NOOP_OBSERVABLE_UP_DOWN_COUNTER_METRIC = new NoopObservableUpDownCounterMetric;
  function createNoopMeter() {
    return exports.NOOP_METER;
  }
  exports.createNoopMeter = createNoopMeter;
});

// node_modules/@opentelemetry/api/build/src/metrics/Metric.js
var require_Metric = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.ValueType = undefined;
  var ValueType;
  (function(ValueType2) {
    ValueType2[ValueType2["INT"] = 0] = "INT";
    ValueType2[ValueType2["DOUBLE"] = 1] = "DOUBLE";
  })(ValueType = exports.ValueType || (exports.ValueType = {}));
});

// node_modules/@opentelemetry/api/build/src/propagation/TextMapPropagator.js
var require_TextMapPropagator = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.defaultTextMapSetter = exports.defaultTextMapGetter = undefined;
  exports.defaultTextMapGetter = {
    get(carrier, key) {
      if (carrier == null) {
        return;
      }
      return carrier[key];
    },
    keys(carrier) {
      if (carrier == null) {
        return [];
      }
      return Object.keys(carrier);
    }
  };
  exports.defaultTextMapSetter = {
    set(carrier, key, value) {
      if (carrier == null) {
        return;
      }
      carrier[key] = value;
    }
  };
});

// node_modules/@opentelemetry/api/build/src/context/NoopContextManager.js
var require_NoopContextManager = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.NoopContextManager = undefined;
  var context_1 = require_context();

  class NoopContextManager {
    active() {
      return context_1.ROOT_CONTEXT;
    }
    with(_context, fn, thisArg, ...args) {
      return fn.call(thisArg, ...args);
    }
    bind(_context, target) {
      return target;
    }
    enable() {
      return this;
    }
    disable() {
      return this;
    }
  }
  exports.NoopContextManager = NoopContextManager;
});

// node_modules/@opentelemetry/api/build/src/api/context.js
var require_context2 = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.ContextAPI = undefined;
  var NoopContextManager_1 = require_NoopContextManager();
  var global_utils_1 = require_global_utils();
  var diag_1 = require_diag();
  var API_NAME = "context";
  var NOOP_CONTEXT_MANAGER = new NoopContextManager_1.NoopContextManager;

  class ContextAPI {
    constructor() {}
    static getInstance() {
      if (!this._instance) {
        this._instance = new ContextAPI;
      }
      return this._instance;
    }
    setGlobalContextManager(contextManager) {
      return (0, global_utils_1.registerGlobal)(API_NAME, contextManager, diag_1.DiagAPI.instance());
    }
    active() {
      return this._getContextManager().active();
    }
    with(context, fn, thisArg, ...args) {
      return this._getContextManager().with(context, fn, thisArg, ...args);
    }
    bind(context, target) {
      return this._getContextManager().bind(context, target);
    }
    _getContextManager() {
      return (0, global_utils_1.getGlobal)(API_NAME) || NOOP_CONTEXT_MANAGER;
    }
    disable() {
      this._getContextManager().disable();
      (0, global_utils_1.unregisterGlobal)(API_NAME, diag_1.DiagAPI.instance());
    }
  }
  exports.ContextAPI = ContextAPI;
});

// node_modules/@opentelemetry/api/build/src/trace/trace_flags.js
var require_trace_flags = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.TraceFlags = undefined;
  var TraceFlags;
  (function(TraceFlags2) {
    TraceFlags2[TraceFlags2["NONE"] = 0] = "NONE";
    TraceFlags2[TraceFlags2["SAMPLED"] = 1] = "SAMPLED";
  })(TraceFlags = exports.TraceFlags || (exports.TraceFlags = {}));
});

// node_modules/@opentelemetry/api/build/src/trace/invalid-span-constants.js
var require_invalid_span_constants = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.INVALID_SPAN_CONTEXT = exports.INVALID_TRACEID = exports.INVALID_SPANID = undefined;
  var trace_flags_1 = require_trace_flags();
  exports.INVALID_SPANID = "0000000000000000";
  exports.INVALID_TRACEID = "00000000000000000000000000000000";
  exports.INVALID_SPAN_CONTEXT = {
    traceId: exports.INVALID_TRACEID,
    spanId: exports.INVALID_SPANID,
    traceFlags: trace_flags_1.TraceFlags.NONE
  };
});

// node_modules/@opentelemetry/api/build/src/trace/NonRecordingSpan.js
var require_NonRecordingSpan = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.NonRecordingSpan = undefined;
  var invalid_span_constants_1 = require_invalid_span_constants();

  class NonRecordingSpan {
    constructor(spanContext = invalid_span_constants_1.INVALID_SPAN_CONTEXT) {
      this._spanContext = spanContext;
    }
    spanContext() {
      return this._spanContext;
    }
    setAttribute(_key, _value) {
      return this;
    }
    setAttributes(_attributes) {
      return this;
    }
    addEvent(_name, _attributes) {
      return this;
    }
    addLink(_link) {
      return this;
    }
    addLinks(_links) {
      return this;
    }
    setStatus(_status) {
      return this;
    }
    updateName(_name) {
      return this;
    }
    end(_endTime) {}
    isRecording() {
      return false;
    }
    recordException(_exception, _time) {}
  }
  exports.NonRecordingSpan = NonRecordingSpan;
});

// node_modules/@opentelemetry/api/build/src/trace/context-utils.js
var require_context_utils = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.getSpanContext = exports.setSpanContext = exports.deleteSpan = exports.setSpan = exports.getActiveSpan = exports.getSpan = undefined;
  var context_1 = require_context();
  var NonRecordingSpan_1 = require_NonRecordingSpan();
  var context_2 = require_context2();
  var SPAN_KEY = (0, context_1.createContextKey)("OpenTelemetry Context Key SPAN");
  function getSpan(context) {
    return context.getValue(SPAN_KEY) || undefined;
  }
  exports.getSpan = getSpan;
  function getActiveSpan() {
    return getSpan(context_2.ContextAPI.getInstance().active());
  }
  exports.getActiveSpan = getActiveSpan;
  function setSpan(context, span) {
    return context.setValue(SPAN_KEY, span);
  }
  exports.setSpan = setSpan;
  function deleteSpan(context) {
    return context.deleteValue(SPAN_KEY);
  }
  exports.deleteSpan = deleteSpan;
  function setSpanContext(context, spanContext) {
    return setSpan(context, new NonRecordingSpan_1.NonRecordingSpan(spanContext));
  }
  exports.setSpanContext = setSpanContext;
  function getSpanContext(context) {
    var _a;
    return (_a = getSpan(context)) === null || _a === undefined ? undefined : _a.spanContext();
  }
  exports.getSpanContext = getSpanContext;
});

// node_modules/@opentelemetry/api/build/src/trace/spancontext-utils.js
var require_spancontext_utils = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.wrapSpanContext = exports.isSpanContextValid = exports.isValidSpanId = exports.isValidTraceId = undefined;
  var invalid_span_constants_1 = require_invalid_span_constants();
  var NonRecordingSpan_1 = require_NonRecordingSpan();
  var isHex = new Uint8Array([
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    1,
    1,
    1,
    1,
    1,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    1,
    1,
    1,
    1,
    1
  ]);
  function isValidHex(id, length) {
    if (typeof id !== "string" || id.length !== length)
      return false;
    let r = 0;
    for (let i = 0;i < id.length; i += 4) {
      r += (isHex[id.charCodeAt(i)] | 0) + (isHex[id.charCodeAt(i + 1)] | 0) + (isHex[id.charCodeAt(i + 2)] | 0) + (isHex[id.charCodeAt(i + 3)] | 0);
    }
    return r === length;
  }
  function isValidTraceId(traceId) {
    return isValidHex(traceId, 32) && traceId !== invalid_span_constants_1.INVALID_TRACEID;
  }
  exports.isValidTraceId = isValidTraceId;
  function isValidSpanId(spanId) {
    return isValidHex(spanId, 16) && spanId !== invalid_span_constants_1.INVALID_SPANID;
  }
  exports.isValidSpanId = isValidSpanId;
  function isSpanContextValid(spanContext) {
    return isValidTraceId(spanContext.traceId) && isValidSpanId(spanContext.spanId);
  }
  exports.isSpanContextValid = isSpanContextValid;
  function wrapSpanContext(spanContext) {
    return new NonRecordingSpan_1.NonRecordingSpan(spanContext);
  }
  exports.wrapSpanContext = wrapSpanContext;
});

// node_modules/@opentelemetry/api/build/src/trace/NoopTracer.js
var require_NoopTracer = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.NoopTracer = undefined;
  var context_1 = require_context2();
  var context_utils_1 = require_context_utils();
  var NonRecordingSpan_1 = require_NonRecordingSpan();
  var spancontext_utils_1 = require_spancontext_utils();
  var contextApi = context_1.ContextAPI.getInstance();

  class NoopTracer {
    startSpan(name, options, context = contextApi.active()) {
      const root = Boolean(options === null || options === undefined ? undefined : options.root);
      if (root) {
        return new NonRecordingSpan_1.NonRecordingSpan;
      }
      const parentFromContext = context && (0, context_utils_1.getSpanContext)(context);
      if (isSpanContext(parentFromContext) && (0, spancontext_utils_1.isSpanContextValid)(parentFromContext)) {
        return new NonRecordingSpan_1.NonRecordingSpan(parentFromContext);
      } else {
        return new NonRecordingSpan_1.NonRecordingSpan;
      }
    }
    startActiveSpan(name, arg2, arg3, arg4) {
      let opts;
      let ctx;
      let fn;
      if (arguments.length < 2) {
        return;
      } else if (arguments.length === 2) {
        fn = arg2;
      } else if (arguments.length === 3) {
        opts = arg2;
        fn = arg3;
      } else {
        opts = arg2;
        ctx = arg3;
        fn = arg4;
      }
      const parentContext = ctx !== null && ctx !== undefined ? ctx : contextApi.active();
      const span = this.startSpan(name, opts, parentContext);
      const contextWithSpanSet = (0, context_utils_1.setSpan)(parentContext, span);
      return contextApi.with(contextWithSpanSet, fn, undefined, span);
    }
  }
  exports.NoopTracer = NoopTracer;
  function isSpanContext(spanContext) {
    return spanContext !== null && typeof spanContext === "object" && "spanId" in spanContext && typeof spanContext["spanId"] === "string" && "traceId" in spanContext && typeof spanContext["traceId"] === "string" && "traceFlags" in spanContext && typeof spanContext["traceFlags"] === "number";
  }
});

// node_modules/@opentelemetry/api/build/src/trace/ProxyTracer.js
var require_ProxyTracer = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.ProxyTracer = undefined;
  var NoopTracer_1 = require_NoopTracer();
  var NOOP_TRACER = new NoopTracer_1.NoopTracer;

  class ProxyTracer {
    constructor(provider, name, version, options) {
      this._provider = provider;
      this.name = name;
      this.version = version;
      this.options = options;
    }
    startSpan(name, options, context) {
      return this._getTracer().startSpan(name, options, context);
    }
    startActiveSpan(_name, _options, _context, _fn) {
      const tracer = this._getTracer();
      return Reflect.apply(tracer.startActiveSpan, tracer, arguments);
    }
    _getTracer() {
      if (this._delegate) {
        return this._delegate;
      }
      const tracer = this._provider.getDelegateTracer(this.name, this.version, this.options);
      if (!tracer) {
        return NOOP_TRACER;
      }
      this._delegate = tracer;
      return this._delegate;
    }
  }
  exports.ProxyTracer = ProxyTracer;
});

// node_modules/@opentelemetry/api/build/src/trace/NoopTracerProvider.js
var require_NoopTracerProvider = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.NoopTracerProvider = undefined;
  var NoopTracer_1 = require_NoopTracer();

  class NoopTracerProvider {
    getTracer(_name, _version, _options) {
      return new NoopTracer_1.NoopTracer;
    }
  }
  exports.NoopTracerProvider = NoopTracerProvider;
});

// node_modules/@opentelemetry/api/build/src/trace/ProxyTracerProvider.js
var require_ProxyTracerProvider = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.ProxyTracerProvider = undefined;
  var ProxyTracer_1 = require_ProxyTracer();
  var NoopTracerProvider_1 = require_NoopTracerProvider();
  var NOOP_TRACER_PROVIDER = new NoopTracerProvider_1.NoopTracerProvider;

  class ProxyTracerProvider {
    getTracer(name, version, options) {
      var _a;
      return (_a = this.getDelegateTracer(name, version, options)) !== null && _a !== undefined ? _a : new ProxyTracer_1.ProxyTracer(this, name, version, options);
    }
    getDelegate() {
      var _a;
      return (_a = this._delegate) !== null && _a !== undefined ? _a : NOOP_TRACER_PROVIDER;
    }
    setDelegate(delegate) {
      this._delegate = delegate;
    }
    getDelegateTracer(name, version, options) {
      var _a;
      return (_a = this._delegate) === null || _a === undefined ? undefined : _a.getTracer(name, version, options);
    }
  }
  exports.ProxyTracerProvider = ProxyTracerProvider;
});

// node_modules/@opentelemetry/api/build/src/trace/SamplingResult.js
var require_SamplingResult = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.SamplingDecision = undefined;
  var SamplingDecision;
  (function(SamplingDecision2) {
    SamplingDecision2[SamplingDecision2["NOT_RECORD"] = 0] = "NOT_RECORD";
    SamplingDecision2[SamplingDecision2["RECORD"] = 1] = "RECORD";
    SamplingDecision2[SamplingDecision2["RECORD_AND_SAMPLED"] = 2] = "RECORD_AND_SAMPLED";
  })(SamplingDecision = exports.SamplingDecision || (exports.SamplingDecision = {}));
});

// node_modules/@opentelemetry/api/build/src/trace/span_kind.js
var require_span_kind = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.SpanKind = undefined;
  var SpanKind;
  (function(SpanKind2) {
    SpanKind2[SpanKind2["INTERNAL"] = 0] = "INTERNAL";
    SpanKind2[SpanKind2["SERVER"] = 1] = "SERVER";
    SpanKind2[SpanKind2["CLIENT"] = 2] = "CLIENT";
    SpanKind2[SpanKind2["PRODUCER"] = 3] = "PRODUCER";
    SpanKind2[SpanKind2["CONSUMER"] = 4] = "CONSUMER";
  })(SpanKind = exports.SpanKind || (exports.SpanKind = {}));
});

// node_modules/@opentelemetry/api/build/src/trace/status.js
var require_status = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.SpanStatusCode = undefined;
  var SpanStatusCode;
  (function(SpanStatusCode2) {
    SpanStatusCode2[SpanStatusCode2["UNSET"] = 0] = "UNSET";
    SpanStatusCode2[SpanStatusCode2["OK"] = 1] = "OK";
    SpanStatusCode2[SpanStatusCode2["ERROR"] = 2] = "ERROR";
  })(SpanStatusCode = exports.SpanStatusCode || (exports.SpanStatusCode = {}));
});

// node_modules/@opentelemetry/api/build/src/trace/internal/tracestate-validators.js
var require_tracestate_validators = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.validateValue = exports.validateKey = undefined;
  var VALID_KEY_CHAR_RANGE = "[_0-9a-z-*/]";
  var VALID_KEY = `[a-z]${VALID_KEY_CHAR_RANGE}{0,255}`;
  var VALID_VENDOR_KEY = `[a-z0-9]${VALID_KEY_CHAR_RANGE}{0,240}@[a-z]${VALID_KEY_CHAR_RANGE}{0,13}`;
  var VALID_KEY_REGEX = new RegExp(`^(?:${VALID_KEY}|${VALID_VENDOR_KEY})$`);
  var VALID_VALUE_BASE_REGEX = /^[ -~]{0,255}[!-~]$/;
  var INVALID_VALUE_COMMA_EQUAL_REGEX = /,|=/;
  function validateKey(key) {
    return VALID_KEY_REGEX.test(key);
  }
  exports.validateKey = validateKey;
  function validateValue(value) {
    return VALID_VALUE_BASE_REGEX.test(value) && !INVALID_VALUE_COMMA_EQUAL_REGEX.test(value);
  }
  exports.validateValue = validateValue;
});

// node_modules/@opentelemetry/api/build/src/trace/internal/tracestate-impl.js
var require_tracestate_impl = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.TraceStateImpl = undefined;
  var tracestate_validators_1 = require_tracestate_validators();
  var MAX_TRACE_STATE_ITEMS = 32;
  var MAX_TRACE_STATE_LEN = 512;
  var LIST_MEMBERS_SEPARATOR = ",";
  var LIST_MEMBER_KEY_VALUE_SPLITTER = "=";

  class TraceStateImpl {
    constructor(rawTraceState) {
      this._internalState = new Map;
      if (rawTraceState)
        this._parse(rawTraceState);
    }
    set(key, value) {
      const traceState = this._clone();
      if (traceState._internalState.has(key)) {
        traceState._internalState.delete(key);
      }
      traceState._internalState.set(key, value);
      return traceState;
    }
    unset(key) {
      const traceState = this._clone();
      traceState._internalState.delete(key);
      return traceState;
    }
    get(key) {
      return this._internalState.get(key);
    }
    serialize() {
      return Array.from(this._internalState.keys()).reduceRight((agg, key) => {
        agg.push(key + LIST_MEMBER_KEY_VALUE_SPLITTER + this.get(key));
        return agg;
      }, []).join(LIST_MEMBERS_SEPARATOR);
    }
    _parse(rawTraceState) {
      if (rawTraceState.length > MAX_TRACE_STATE_LEN)
        return;
      this._internalState = rawTraceState.split(LIST_MEMBERS_SEPARATOR).reduceRight((agg, part) => {
        const listMember = part.trim();
        const i = listMember.indexOf(LIST_MEMBER_KEY_VALUE_SPLITTER);
        if (i !== -1) {
          const key = listMember.slice(0, i);
          const value = listMember.slice(i + 1, part.length);
          if ((0, tracestate_validators_1.validateKey)(key) && (0, tracestate_validators_1.validateValue)(value)) {
            agg.set(key, value);
          }
        }
        return agg;
      }, new Map);
      if (this._internalState.size > MAX_TRACE_STATE_ITEMS) {
        this._internalState = new Map(Array.from(this._internalState.entries()).reverse().slice(0, MAX_TRACE_STATE_ITEMS));
      }
    }
    _keys() {
      return Array.from(this._internalState.keys()).reverse();
    }
    _clone() {
      const traceState = new TraceStateImpl;
      traceState._internalState = new Map(this._internalState);
      return traceState;
    }
  }
  exports.TraceStateImpl = TraceStateImpl;
});

// node_modules/@opentelemetry/api/build/src/trace/internal/utils.js
var require_utils2 = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.createTraceState = undefined;
  var tracestate_impl_1 = require_tracestate_impl();
  function createTraceState(rawTraceState) {
    return new tracestate_impl_1.TraceStateImpl(rawTraceState);
  }
  exports.createTraceState = createTraceState;
});

// node_modules/@opentelemetry/api/build/src/context-api.js
var require_context_api = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.context = undefined;
  var context_1 = require_context2();
  exports.context = context_1.ContextAPI.getInstance();
});

// node_modules/@opentelemetry/api/build/src/diag-api.js
var require_diag_api = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.diag = undefined;
  var diag_1 = require_diag();
  exports.diag = diag_1.DiagAPI.instance();
});

// node_modules/@opentelemetry/api/build/src/metrics/NoopMeterProvider.js
var require_NoopMeterProvider = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.NOOP_METER_PROVIDER = exports.NoopMeterProvider = undefined;
  var NoopMeter_1 = require_NoopMeter();

  class NoopMeterProvider {
    getMeter(_name, _version, _options) {
      return NoopMeter_1.NOOP_METER;
    }
  }
  exports.NoopMeterProvider = NoopMeterProvider;
  exports.NOOP_METER_PROVIDER = new NoopMeterProvider;
});

// node_modules/@opentelemetry/api/build/src/api/metrics.js
var require_metrics = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.MetricsAPI = undefined;
  var NoopMeterProvider_1 = require_NoopMeterProvider();
  var global_utils_1 = require_global_utils();
  var diag_1 = require_diag();
  var API_NAME = "metrics";

  class MetricsAPI {
    constructor() {}
    static getInstance() {
      if (!this._instance) {
        this._instance = new MetricsAPI;
      }
      return this._instance;
    }
    setGlobalMeterProvider(provider) {
      return (0, global_utils_1.registerGlobal)(API_NAME, provider, diag_1.DiagAPI.instance());
    }
    getMeterProvider() {
      return (0, global_utils_1.getGlobal)(API_NAME) || NoopMeterProvider_1.NOOP_METER_PROVIDER;
    }
    getMeter(name, version, options) {
      return this.getMeterProvider().getMeter(name, version, options);
    }
    disable() {
      (0, global_utils_1.unregisterGlobal)(API_NAME, diag_1.DiagAPI.instance());
    }
  }
  exports.MetricsAPI = MetricsAPI;
});

// node_modules/@opentelemetry/api/build/src/metrics-api.js
var require_metrics_api = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.metrics = undefined;
  var metrics_1 = require_metrics();
  exports.metrics = metrics_1.MetricsAPI.getInstance();
});

// node_modules/@opentelemetry/api/build/src/propagation/NoopTextMapPropagator.js
var require_NoopTextMapPropagator = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.NoopTextMapPropagator = undefined;

  class NoopTextMapPropagator {
    inject(_context, _carrier) {}
    extract(context, _carrier) {
      return context;
    }
    fields() {
      return [];
    }
  }
  exports.NoopTextMapPropagator = NoopTextMapPropagator;
});

// node_modules/@opentelemetry/api/build/src/baggage/context-helpers.js
var require_context_helpers = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.deleteBaggage = exports.setBaggage = exports.getActiveBaggage = exports.getBaggage = undefined;
  var context_1 = require_context2();
  var context_2 = require_context();
  var BAGGAGE_KEY = (0, context_2.createContextKey)("OpenTelemetry Baggage Key");
  function getBaggage(context) {
    return context.getValue(BAGGAGE_KEY) || undefined;
  }
  exports.getBaggage = getBaggage;
  function getActiveBaggage() {
    return getBaggage(context_1.ContextAPI.getInstance().active());
  }
  exports.getActiveBaggage = getActiveBaggage;
  function setBaggage(context, baggage) {
    return context.setValue(BAGGAGE_KEY, baggage);
  }
  exports.setBaggage = setBaggage;
  function deleteBaggage(context) {
    return context.deleteValue(BAGGAGE_KEY);
  }
  exports.deleteBaggage = deleteBaggage;
});

// node_modules/@opentelemetry/api/build/src/api/propagation.js
var require_propagation = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.PropagationAPI = undefined;
  var global_utils_1 = require_global_utils();
  var NoopTextMapPropagator_1 = require_NoopTextMapPropagator();
  var TextMapPropagator_1 = require_TextMapPropagator();
  var context_helpers_1 = require_context_helpers();
  var utils_1 = require_utils();
  var diag_1 = require_diag();
  var API_NAME = "propagation";
  var NOOP_TEXT_MAP_PROPAGATOR = new NoopTextMapPropagator_1.NoopTextMapPropagator;

  class PropagationAPI {
    constructor() {
      this.createBaggage = utils_1.createBaggage;
      this.getBaggage = context_helpers_1.getBaggage;
      this.getActiveBaggage = context_helpers_1.getActiveBaggage;
      this.setBaggage = context_helpers_1.setBaggage;
      this.deleteBaggage = context_helpers_1.deleteBaggage;
    }
    static getInstance() {
      if (!this._instance) {
        this._instance = new PropagationAPI;
      }
      return this._instance;
    }
    setGlobalPropagator(propagator) {
      return (0, global_utils_1.registerGlobal)(API_NAME, propagator, diag_1.DiagAPI.instance());
    }
    inject(context, carrier, setter = TextMapPropagator_1.defaultTextMapSetter) {
      return this._getGlobalPropagator().inject(context, carrier, setter);
    }
    extract(context, carrier, getter = TextMapPropagator_1.defaultTextMapGetter) {
      return this._getGlobalPropagator().extract(context, carrier, getter);
    }
    fields() {
      return this._getGlobalPropagator().fields();
    }
    disable() {
      (0, global_utils_1.unregisterGlobal)(API_NAME, diag_1.DiagAPI.instance());
    }
    _getGlobalPropagator() {
      return (0, global_utils_1.getGlobal)(API_NAME) || NOOP_TEXT_MAP_PROPAGATOR;
    }
  }
  exports.PropagationAPI = PropagationAPI;
});

// node_modules/@opentelemetry/api/build/src/propagation-api.js
var require_propagation_api = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.propagation = undefined;
  var propagation_1 = require_propagation();
  exports.propagation = propagation_1.PropagationAPI.getInstance();
});

// node_modules/@opentelemetry/api/build/src/api/trace.js
var require_trace = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.TraceAPI = undefined;
  var global_utils_1 = require_global_utils();
  var ProxyTracerProvider_1 = require_ProxyTracerProvider();
  var spancontext_utils_1 = require_spancontext_utils();
  var context_utils_1 = require_context_utils();
  var diag_1 = require_diag();
  var API_NAME = "trace";

  class TraceAPI {
    constructor() {
      this._proxyTracerProvider = new ProxyTracerProvider_1.ProxyTracerProvider;
      this.wrapSpanContext = spancontext_utils_1.wrapSpanContext;
      this.isSpanContextValid = spancontext_utils_1.isSpanContextValid;
      this.deleteSpan = context_utils_1.deleteSpan;
      this.getSpan = context_utils_1.getSpan;
      this.getActiveSpan = context_utils_1.getActiveSpan;
      this.getSpanContext = context_utils_1.getSpanContext;
      this.setSpan = context_utils_1.setSpan;
      this.setSpanContext = context_utils_1.setSpanContext;
    }
    static getInstance() {
      if (!this._instance) {
        this._instance = new TraceAPI;
      }
      return this._instance;
    }
    setGlobalTracerProvider(provider) {
      const success = (0, global_utils_1.registerGlobal)(API_NAME, this._proxyTracerProvider, diag_1.DiagAPI.instance());
      if (success) {
        this._proxyTracerProvider.setDelegate(provider);
      }
      return success;
    }
    getTracerProvider() {
      return (0, global_utils_1.getGlobal)(API_NAME) || this._proxyTracerProvider;
    }
    getTracer(name, version) {
      return this.getTracerProvider().getTracer(name, version);
    }
    disable() {
      (0, global_utils_1.unregisterGlobal)(API_NAME, diag_1.DiagAPI.instance());
      this._proxyTracerProvider = new ProxyTracerProvider_1.ProxyTracerProvider;
    }
  }
  exports.TraceAPI = TraceAPI;
});

// node_modules/@opentelemetry/api/build/src/trace-api.js
var require_trace_api = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.trace = undefined;
  var trace_1 = require_trace();
  exports.trace = trace_1.TraceAPI.getInstance();
});

// node_modules/@opentelemetry/api/build/src/index.js
var require_src = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.trace = exports.propagation = exports.metrics = exports.diag = exports.context = exports.INVALID_SPAN_CONTEXT = exports.INVALID_TRACEID = exports.INVALID_SPANID = exports.isValidSpanId = exports.isValidTraceId = exports.isSpanContextValid = exports.createTraceState = exports.TraceFlags = exports.SpanStatusCode = exports.SpanKind = exports.SamplingDecision = exports.ProxyTracerProvider = exports.ProxyTracer = exports.defaultTextMapSetter = exports.defaultTextMapGetter = exports.ValueType = exports.createNoopMeter = exports.DiagLogLevel = exports.DiagConsoleLogger = exports.ROOT_CONTEXT = exports.createContextKey = exports.baggageEntryMetadataFromString = undefined;
  var utils_1 = require_utils();
  Object.defineProperty(exports, "baggageEntryMetadataFromString", { enumerable: true, get: function() {
    return utils_1.baggageEntryMetadataFromString;
  } });
  var context_1 = require_context();
  Object.defineProperty(exports, "createContextKey", { enumerable: true, get: function() {
    return context_1.createContextKey;
  } });
  Object.defineProperty(exports, "ROOT_CONTEXT", { enumerable: true, get: function() {
    return context_1.ROOT_CONTEXT;
  } });
  var consoleLogger_1 = require_consoleLogger();
  Object.defineProperty(exports, "DiagConsoleLogger", { enumerable: true, get: function() {
    return consoleLogger_1.DiagConsoleLogger;
  } });
  var types_1 = require_types();
  Object.defineProperty(exports, "DiagLogLevel", { enumerable: true, get: function() {
    return types_1.DiagLogLevel;
  } });
  var NoopMeter_1 = require_NoopMeter();
  Object.defineProperty(exports, "createNoopMeter", { enumerable: true, get: function() {
    return NoopMeter_1.createNoopMeter;
  } });
  var Metric_1 = require_Metric();
  Object.defineProperty(exports, "ValueType", { enumerable: true, get: function() {
    return Metric_1.ValueType;
  } });
  var TextMapPropagator_1 = require_TextMapPropagator();
  Object.defineProperty(exports, "defaultTextMapGetter", { enumerable: true, get: function() {
    return TextMapPropagator_1.defaultTextMapGetter;
  } });
  Object.defineProperty(exports, "defaultTextMapSetter", { enumerable: true, get: function() {
    return TextMapPropagator_1.defaultTextMapSetter;
  } });
  var ProxyTracer_1 = require_ProxyTracer();
  Object.defineProperty(exports, "ProxyTracer", { enumerable: true, get: function() {
    return ProxyTracer_1.ProxyTracer;
  } });
  var ProxyTracerProvider_1 = require_ProxyTracerProvider();
  Object.defineProperty(exports, "ProxyTracerProvider", { enumerable: true, get: function() {
    return ProxyTracerProvider_1.ProxyTracerProvider;
  } });
  var SamplingResult_1 = require_SamplingResult();
  Object.defineProperty(exports, "SamplingDecision", { enumerable: true, get: function() {
    return SamplingResult_1.SamplingDecision;
  } });
  var span_kind_1 = require_span_kind();
  Object.defineProperty(exports, "SpanKind", { enumerable: true, get: function() {
    return span_kind_1.SpanKind;
  } });
  var status_1 = require_status();
  Object.defineProperty(exports, "SpanStatusCode", { enumerable: true, get: function() {
    return status_1.SpanStatusCode;
  } });
  var trace_flags_1 = require_trace_flags();
  Object.defineProperty(exports, "TraceFlags", { enumerable: true, get: function() {
    return trace_flags_1.TraceFlags;
  } });
  var utils_2 = require_utils2();
  Object.defineProperty(exports, "createTraceState", { enumerable: true, get: function() {
    return utils_2.createTraceState;
  } });
  var spancontext_utils_1 = require_spancontext_utils();
  Object.defineProperty(exports, "isSpanContextValid", { enumerable: true, get: function() {
    return spancontext_utils_1.isSpanContextValid;
  } });
  Object.defineProperty(exports, "isValidTraceId", { enumerable: true, get: function() {
    return spancontext_utils_1.isValidTraceId;
  } });
  Object.defineProperty(exports, "isValidSpanId", { enumerable: true, get: function() {
    return spancontext_utils_1.isValidSpanId;
  } });
  var invalid_span_constants_1 = require_invalid_span_constants();
  Object.defineProperty(exports, "INVALID_SPANID", { enumerable: true, get: function() {
    return invalid_span_constants_1.INVALID_SPANID;
  } });
  Object.defineProperty(exports, "INVALID_TRACEID", { enumerable: true, get: function() {
    return invalid_span_constants_1.INVALID_TRACEID;
  } });
  Object.defineProperty(exports, "INVALID_SPAN_CONTEXT", { enumerable: true, get: function() {
    return invalid_span_constants_1.INVALID_SPAN_CONTEXT;
  } });
  var context_api_1 = require_context_api();
  Object.defineProperty(exports, "context", { enumerable: true, get: function() {
    return context_api_1.context;
  } });
  var diag_api_1 = require_diag_api();
  Object.defineProperty(exports, "diag", { enumerable: true, get: function() {
    return diag_api_1.diag;
  } });
  var metrics_api_1 = require_metrics_api();
  Object.defineProperty(exports, "metrics", { enumerable: true, get: function() {
    return metrics_api_1.metrics;
  } });
  var propagation_api_1 = require_propagation_api();
  Object.defineProperty(exports, "propagation", { enumerable: true, get: function() {
    return propagation_api_1.propagation;
  } });
  var trace_api_1 = require_trace_api();
  Object.defineProperty(exports, "trace", { enumerable: true, get: function() {
    return trace_api_1.trace;
  } });
  exports.default = {
    context: context_api_1.context,
    diag: diag_api_1.diag,
    metrics: metrics_api_1.metrics,
    propagation: propagation_api_1.propagation,
    trace: trace_api_1.trace
  };
});

// node_modules/tinyduration/dist/index.js
var require_dist = __commonJS(function(exports) {
  Object.defineProperty(exports, "__esModule", { value: true });
  exports.serialize = exports.parse = exports.MultipleFractionsError = exports.InvalidDurationError = undefined;
  var DEFAULT_PARSE_CONFIG = {
    allowMultipleFractions: true
  };
  var units = [
    { unit: "years", symbol: "Y" },
    { unit: "months", symbol: "M" },
    { unit: "weeks", symbol: "W" },
    { unit: "days", symbol: "D" },
    { unit: "hours", symbol: "H" },
    { unit: "minutes", symbol: "M" },
    { unit: "seconds", symbol: "S" }
  ];
  var r = (name, unit) => `((?<${name}>-?\\d*[\\.,]?\\d+)${unit})?`;
  var durationRegex = new RegExp([
    "(?<negative>-)?P",
    r("years", "Y"),
    r("months", "M"),
    r("weeks", "W"),
    r("days", "D"),
    "(T",
    r("hours", "H"),
    r("minutes", "M"),
    r("seconds", "S"),
    ")?"
  ].join(""));
  function parseNum(s2) {
    if (s2 === "" || s2 === undefined || s2 === null) {
      return;
    }
    return parseFloat(s2.replace(",", "."));
  }
  exports.InvalidDurationError = new Error("Invalid duration");
  exports.MultipleFractionsError = new Error("Multiple fractions specified");
  function parse(durationStr, config = DEFAULT_PARSE_CONFIG) {
    const match = durationRegex.exec(durationStr);
    if (!match || !match.groups) {
      throw exports.InvalidDurationError;
    }
    let empty = true;
    let decimalFractionCount = 0;
    const values = {};
    for (const { unit } of units) {
      if (match.groups[unit]) {
        empty = false;
        values[unit] = parseNum(match.groups[unit]);
        if (!config.allowMultipleFractions && !Number.isInteger(values[unit])) {
          decimalFractionCount++;
          if (decimalFractionCount > 1) {
            throw exports.MultipleFractionsError;
          }
        }
      }
    }
    if (empty) {
      throw exports.InvalidDurationError;
    }
    const duration = values;
    if (match.groups.negative) {
      duration.negative = true;
    }
    return duration;
  }
  exports.parse = parse;
  var s = (number, component) => {
    if (!number) {
      return;
    }
    let numberAsString = number.toString();
    const exponentIndex = numberAsString.indexOf("e");
    if (exponentIndex > -1) {
      const magnitude = parseInt(numberAsString.slice(exponentIndex + 2), 10);
      numberAsString = number.toFixed(magnitude + exponentIndex - 2);
    }
    return numberAsString + component;
  };
  function serialize(duration) {
    if (!duration.years && !duration.months && !duration.weeks && !duration.days && !duration.hours && !duration.minutes && !duration.seconds) {
      return "PT0S";
    }
    return [
      duration.negative && "-",
      "P",
      s(duration.years, "Y"),
      s(duration.months, "M"),
      s(duration.weeks, "W"),
      s(duration.days, "D"),
      (duration.hours || duration.minutes || duration.seconds) && "T",
      s(duration.hours, "H"),
      s(duration.minutes, "M"),
      s(duration.seconds, "S")
    ].filter(Boolean).join("");
  }
  exports.serialize = serialize;
});
// node_modules/@microsoft/kiota-abstractions/dist/es/src/serialization/parseNodeFactoryRegistry.js
class ParseNodeFactoryRegistry {
  constructor() {
    this.jsonContentType = "application/json";
    this.contentTypeAssociatedFactories = new Map;
  }
  getValidContentType() {
    throw new Error("The registry supports multiple content types. Get the registered factory instead.");
  }
  getRootParseNode(contentType, content) {
    if (!contentType) {
      throw new Error("content type cannot be undefined or empty");
    }
    if (!content) {
      throw new Error("content cannot be undefined or empty");
    }
    const vendorSpecificContentType = contentType.split(";")[0];
    let factory = this.contentTypeAssociatedFactories.get(vendorSpecificContentType);
    if (factory) {
      return factory.getRootParseNode(vendorSpecificContentType, content);
    }
    const cleanedContentType = vendorSpecificContentType.replace(/[^/]+\+/gi, "");
    factory = this.contentTypeAssociatedFactories.get(cleanedContentType);
    if (factory) {
      return factory.getRootParseNode(cleanedContentType, content);
    }
    throw new Error(`Content type ${cleanedContentType} does not have a factory registered to be parsed`);
  }
  registerDefaultDeserializer(type, backingStoreFactory) {
    if (!type)
      throw new Error("Type is required");
    const deserializer = new type(backingStoreFactory);
    this.contentTypeAssociatedFactories.set(deserializer.getValidContentType(), deserializer);
  }
  deserializeFromJson(bufferOrString, factory) {
    return this.deserialize(this.jsonContentType, bufferOrString, factory);
  }
  deserializeCollectionFromJson(bufferOrString, factory) {
    return this.deserializeCollection(this.jsonContentType, bufferOrString, factory);
  }
  deserialize(contentType, bufferOrString, factory) {
    if (typeof bufferOrString === "string") {
      bufferOrString = this.getBufferFromString(bufferOrString);
    }
    const reader = this.getParseNode(contentType, bufferOrString, factory);
    return reader.getObjectValue(factory);
  }
  getParseNode(contentType, buffer, factory) {
    if (!contentType) {
      throw new Error("content type cannot be undefined or empty");
    }
    if (!buffer) {
      throw new Error("buffer cannot be undefined");
    }
    if (!factory) {
      throw new Error("factory cannot be undefined");
    }
    return this.getRootParseNode(contentType, buffer);
  }
  deserializeCollection(contentType, bufferOrString, factory) {
    if (typeof bufferOrString === "string") {
      bufferOrString = this.getBufferFromString(bufferOrString);
    }
    const reader = this.getParseNode(contentType, bufferOrString, factory);
    return reader.getCollectionOfObjectValues(factory);
  }
  getBufferFromString(value) {
    const encoder = new TextEncoder;
    return encoder.encode(value).buffer;
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/serialization/parseNodeProxyFactory.js
class ParseNodeProxyFactory {
  getValidContentType() {
    return this._concrete.getValidContentType();
  }
  constructor(_concrete, _onBefore, _onAfter) {
    this._concrete = _concrete;
    this._onBefore = _onBefore;
    this._onAfter = _onAfter;
    if (!_concrete) {
      throw new Error("_concrete cannot be undefined");
    }
  }
  getRootParseNode(contentType, content) {
    const node = this._concrete.getRootParseNode(contentType, content);
    const originalBefore = node.onBeforeAssignFieldValues;
    const originalAfter = node.onAfterAssignFieldValues;
    node.onBeforeAssignFieldValues = (value) => {
      if (this._onBefore)
        this._onBefore(value);
      if (originalBefore)
        originalBefore(value);
    };
    node.onAfterAssignFieldValues = (value) => {
      if (this._onAfter)
        this._onAfter(value);
      if (originalAfter)
        originalAfter(value);
    };
    return node;
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/serialization/serializationWriterFactoryRegistry.js
class SerializationWriterFactoryRegistry {
  constructor() {
    this.jsonContentType = "application/json";
    this.contentTypeAssociatedFactories = new Map;
  }
  getValidContentType() {
    throw new Error("The registry supports multiple content types. Get the registered factory instead.");
  }
  getSerializationWriter(contentType) {
    if (!contentType) {
      throw new Error("content type cannot be undefined or empty");
    }
    const vendorSpecificContentType = contentType.split(";")[0];
    let factory = this.contentTypeAssociatedFactories.get(vendorSpecificContentType);
    if (factory) {
      return factory.getSerializationWriter(vendorSpecificContentType);
    }
    const cleanedContentType = vendorSpecificContentType.replace(/[^/]+\+/gi, "");
    factory = this.contentTypeAssociatedFactories.get(cleanedContentType);
    if (factory) {
      return factory.getSerializationWriter(cleanedContentType);
    }
    throw new Error(`Content type ${cleanedContentType} does not have a factory registered to be serialized`);
  }
  registerDefaultSerializer(type) {
    if (!type)
      throw new Error("Type is required");
    const serializer = new type;
    this.contentTypeAssociatedFactories.set(serializer.getValidContentType(), serializer);
  }
  serializeToJson(value, serializationFunction) {
    return this.serialize(this.jsonContentType, value, serializationFunction);
  }
  serializeToJsonAsString(value, serializationFunction) {
    return this.serializeToString(this.jsonContentType, value, serializationFunction);
  }
  serializeCollectionToJson(values, serializationFunction) {
    return this.serializeCollection(this.jsonContentType, values, serializationFunction);
  }
  serializeCollectionToJsonAsString(values, serializationFunction) {
    return this.serializeCollectionToString(this.jsonContentType, values, serializationFunction);
  }
  serialize(contentType, value, serializationFunction) {
    const writer = this.getSerializationFactoryWriter(contentType, value, serializationFunction);
    writer.writeObjectValue(undefined, value, serializationFunction);
    return writer.getSerializedContent();
  }
  serializeToString(contentType, value, serializationFunction) {
    const buffer = this.serialize(contentType, value, serializationFunction);
    return this.getStringValueFromBuffer(buffer);
  }
  serializeCollection(contentType, values, serializationFunction) {
    const writer = this.getSerializationFactoryWriter(contentType, values, serializationFunction);
    writer.writeCollectionOfObjectValues(undefined, values, serializationFunction);
    return writer.getSerializedContent();
  }
  serializeCollectionToString(contentType, values, serializationFunction) {
    const buffer = this.serializeCollection(contentType, values, serializationFunction);
    return this.getStringValueFromBuffer(buffer);
  }
  getSerializationFactoryWriter(contentType, value, serializationFunction) {
    if (!contentType) {
      throw new Error("content type cannot be undefined or empty");
    }
    if (!value) {
      throw new Error("value cannot be undefined");
    }
    if (!serializationFunction) {
      throw new Error("serializationFunction cannot be undefined");
    }
    return this.getSerializationWriter(contentType);
  }
  getStringValueFromBuffer(buffer) {
    const decoder = new TextDecoder;
    return decoder.decode(buffer);
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/serialization/serializationWriterProxyFactory.js
class SerializationWriterProxyFactory {
  getValidContentType() {
    return this._concrete.getValidContentType();
  }
  constructor(_concrete, _onBefore, _onAfter, _onStart) {
    this._concrete = _concrete;
    this._onBefore = _onBefore;
    this._onAfter = _onAfter;
    this._onStart = _onStart;
    if (!_concrete) {
      throw new Error("_concrete cannot be undefined");
    }
  }
  getSerializationWriter(contentType) {
    const writer = this._concrete.getSerializationWriter(contentType);
    const originalBefore = writer.onBeforeObjectSerialization;
    const originalAfter = writer.onAfterObjectSerialization;
    const originalStart = writer.onStartObjectSerialization;
    writer.onBeforeObjectSerialization = (value) => {
      if (this._onBefore)
        this._onBefore(value);
      if (originalBefore)
        originalBefore(value);
    };
    writer.onAfterObjectSerialization = (value) => {
      if (this._onAfter)
        this._onAfter(value);
      if (originalAfter)
        originalAfter(value);
    };
    writer.onStartObjectSerialization = (value, writer_) => {
      if (this._onStart)
        this._onStart(value, writer_);
      if (originalStart)
        originalStart(value, writer_);
    };
    return writer;
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/store/backingStoreParseNodeFactory.js
class BackingStoreParseNodeFactory extends ParseNodeProxyFactory {
  constructor(concrete) {
    super(concrete, (value) => {
      const backedModel = value;
      if (backedModel === null || backedModel === undefined ? undefined : backedModel.backingStore) {
        backedModel.backingStore.initializationCompleted = false;
      }
    }, (value) => {
      const backedModel = value;
      if (backedModel === null || backedModel === undefined ? undefined : backedModel.backingStore) {
        backedModel.backingStore.initializationCompleted = true;
      }
    });
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/store/backingStoreSerializationWriterProxyFactory.js
class BackingStoreSerializationWriterProxyFactory extends SerializationWriterProxyFactory {
  constructor(concrete) {
    super(concrete, (value) => {
      const backedModel = value;
      if (backedModel === null || backedModel === undefined ? undefined : backedModel.backingStore) {
        backedModel.backingStore.returnOnlyChangedValues = true;
      }
    }, (value) => {
      const backedModel = value;
      if (backedModel === null || backedModel === undefined ? undefined : backedModel.backingStore) {
        backedModel.backingStore.returnOnlyChangedValues = false;
        backedModel.backingStore.initializationCompleted = true;
      }
    }, (value, writer) => {
      const backedModel = value;
      if (backedModel === null || backedModel === undefined ? undefined : backedModel.backingStore) {
        const keys = backedModel.backingStore.enumerateKeysForValuesChangedToNull();
        for (const key of keys) {
          writer.writeNullValue(key);
        }
      }
    });
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/utils/guidUtils.js
var createGuid = () => [gen(2), gen(1), gen(1), gen(1), gen(3)].join("-");
var gen = (count) => {
  let out = "";
  for (let i = 0;i < count; i++) {
    out += ((1 + Math.random()) * 65536 | 0).toString(16).substring(1);
  }
  return out;
};
// node_modules/@microsoft/kiota-abstractions/dist/es/src/utils/inNodeEnv.js
var inNodeEnv = () => {
  try {
    return !!Buffer && !!process;
  } catch (err) {
    return !(err instanceof ReferenceError);
  }
};
// node_modules/@microsoft/kiota-abstractions/dist/es/src/store/inMemoryBackingStore.js
class InMemoryBackingStore {
  constructor() {
    this.subscriptions = new Map;
    this.store = new Map;
    this.returnOnlyChangedValues = false;
    this._initializationCompleted = true;
  }
  get(key) {
    const wrapper = this.store.get(key);
    if (wrapper && (this.returnOnlyChangedValues && wrapper.changed || !this.returnOnlyChangedValues)) {
      return wrapper.value;
    }
    return;
  }
  set(key, value) {
    const oldValueWrapper = this.store.get(key);
    const oldValue = oldValueWrapper === null || oldValueWrapper === undefined ? undefined : oldValueWrapper.value;
    if (oldValueWrapper) {
      oldValueWrapper.value = value;
      oldValueWrapper.changed = this.initializationCompleted;
    } else {
      this.store.set(key, {
        changed: this.initializationCompleted,
        value
      });
    }
    this.subscriptions.forEach((sub) => {
      sub(key, oldValue, value);
    });
  }
  enumerate() {
    let filterableArray = [...this.store.entries()];
    if (this.returnOnlyChangedValues) {
      filterableArray = filterableArray.filter(([_, v]) => v.changed);
    }
    return filterableArray.map(([key, value]) => {
      return { key, value };
    });
  }
  enumerateKeysForValuesChangedToNull() {
    const keys = [];
    for (const [key, entry] of this.store) {
      if (entry.changed && !entry.value) {
        keys.push(key);
      }
    }
    return keys;
  }
  subscribe(callback, subscriptionId) {
    if (!callback) {
      throw new Error("callback cannot be undefined");
    }
    subscriptionId = subscriptionId !== null && subscriptionId !== undefined ? subscriptionId : createGuid();
    this.subscriptions.set(subscriptionId, callback);
    return subscriptionId;
  }
  unsubscribe(subscriptionId) {
    this.subscriptions.delete(subscriptionId);
  }
  clear() {
    this.store.clear();
  }
  set initializationCompleted(value) {
    this._initializationCompleted = value;
    this.store.forEach((v) => {
      v.changed = !value;
    });
  }
  get initializationCompleted() {
    return this._initializationCompleted;
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/store/inMemoryBackingStoreFactory.js
class InMemoryBackingStoreFactory {
  createBackingStore() {
    return new InMemoryBackingStore;
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/apiClientBuilder.js
var enableBackingStoreForSerializationWriterFactory = (serializationWriterFactoryRegistry2, parseNodeFactoryRegistry2, original) => {
  if (!original)
    throw new Error("Original must be specified");
  let result = original;
  if (original instanceof SerializationWriterFactoryRegistry) {
    enableBackingStoreForSerializationRegistry(original);
  } else {
    result = new BackingStoreSerializationWriterProxyFactory(original);
  }
  enableBackingStoreForSerializationRegistry(serializationWriterFactoryRegistry2);
  enableBackingStoreForParseNodeRegistry(parseNodeFactoryRegistry2);
  return result;
};
var enableBackingStoreForParseNodeFactory = (parseNodeFactoryRegistry2, original) => {
  if (!original)
    throw new Error("Original must be specified");
  let result = original;
  if (original instanceof ParseNodeFactoryRegistry) {
    enableBackingStoreForParseNodeRegistry(original);
  } else {
    result = new BackingStoreParseNodeFactory(original);
  }
  enableBackingStoreForParseNodeRegistry(parseNodeFactoryRegistry2);
  return result;
};
var enableBackingStoreForParseNodeRegistry = (registry) => {
  for (const [k, v] of registry.contentTypeAssociatedFactories) {
    if (!(v instanceof BackingStoreParseNodeFactory || v instanceof ParseNodeFactoryRegistry)) {
      registry.contentTypeAssociatedFactories.set(k, new BackingStoreParseNodeFactory(v));
    }
  }
};
var enableBackingStoreForSerializationRegistry = (registry) => {
  for (const [k, v] of registry.contentTypeAssociatedFactories) {
    if (!(v instanceof BackingStoreSerializationWriterProxyFactory || v instanceof SerializationWriterFactoryRegistry)) {
      registry.contentTypeAssociatedFactories.set(k, new BackingStoreSerializationWriterProxyFactory(v));
    }
  }
};
// node_modules/@microsoft/kiota-abstractions/dist/es/src/requestInformation.js
var import_api = __toESM(require_src(), 1);

// node_modules/@std-uritemplate/std-uritemplate/dist/index.mjs
class StdUriTemplate {
  static expand(template, substitutions) {
    return StdUriTemplate.expandImpl(template, substitutions);
  }
  static validateLiteral(c, col) {
    switch (c) {
      case "+":
      case "#":
      case "/":
      case ";":
      case "?":
      case "&":
      case " ":
      case "!":
      case "=":
      case "$":
      case "|":
      case "*":
      case ":":
      case "~":
      case "-":
        throw new Error(`Illegal character identified in the token at col: ${col}`);
      default:
        break;
    }
  }
  static getMaxChar(buffer, col) {
    if (!buffer) {
      return -1;
    } else {
      const value = buffer.join("");
      if (value.length === 0) {
        return -1;
      } else {
        if (value.startsWith("0")) {
          throw new Error(`Cannot parse max chars at col: ${col}`);
        }
        const parsed = parseInt(value, 10);
        if (isNaN(parsed) || parsed < 1 || parsed > 9999) {
          throw new Error(`Cannot parse max chars at col: ${col}`);
        }
        return parsed;
      }
    }
  }
  static getOperator(c, token, col) {
    switch (c) {
      case "+":
        return 1;
      case "#":
        return 2;
      case ".":
        return 3;
      case "/":
        return 4;
      case ";":
        return 5;
      case "?":
        return 6;
      case "&":
        return 7;
      default:
        StdUriTemplate.validateLiteral(c, col);
        token.push(c);
        return 0;
    }
  }
  static expandImpl(str, substitutions) {
    const result = [];
    let token = null;
    let operator = null;
    let composite = false;
    let maxCharBuffer = null;
    let firstToken = true;
    for (let i = 0;i < str.length; i++) {
      const character = str.charAt(i);
      switch (character) {
        case "{":
          token = [];
          firstToken = true;
          break;
        case "}":
          if (token !== null) {
            if (maxCharBuffer !== null && maxCharBuffer.length === 0) {
              throw new Error(`Found an empty prefix at col: ${i}`);
            }
            const expanded = StdUriTemplate.expandToken(operator, token.join(""), composite, StdUriTemplate.getMaxChar(maxCharBuffer, i), firstToken, substitutions, result, i);
            if (expanded && firstToken) {
              firstToken = false;
            }
            token = null;
            operator = null;
            composite = false;
            maxCharBuffer = null;
          } else {
            throw new Error(`Failed to expand token, invalid at col: ${i}`);
          }
          break;
        case ",":
          if (token !== null) {
            if (maxCharBuffer !== null && maxCharBuffer.length === 0) {
              throw new Error(`Found an empty prefix at col: ${i}`);
            }
            const expanded = StdUriTemplate.expandToken(operator, token.join(""), composite, StdUriTemplate.getMaxChar(maxCharBuffer, i), firstToken, substitutions, result, i);
            if (expanded && firstToken) {
              firstToken = false;
            }
            token = [];
            composite = false;
            maxCharBuffer = null;
            break;
          }
        default:
          if (token !== null) {
            if (operator === null) {
              operator = StdUriTemplate.getOperator(character, token, i);
            } else if (maxCharBuffer !== null) {
              if (character >= "0" && character <= "9") {
                maxCharBuffer.push(character);
              } else {
                throw new Error(`Illegal character identified in the token at col: ${i}`);
              }
            } else {
              if (character === ":") {
                maxCharBuffer = [];
              } else if (character === "*") {
                composite = true;
              } else {
                StdUriTemplate.validateLiteral(character, i);
                token.push(character);
              }
            }
          } else {
            const cp = character.codePointAt(0);
            if (cp > 127 || cp >= 55296 && cp <= 56319) {
              let toEncode;
              if (cp >= 55296 && cp <= 56319 && i + 1 < str.length) {
                toEncode = character + str.charAt(++i);
              } else {
                toEncode = character;
              }
              result.push(encodeURIComponent(toEncode));
            } else {
              result.push(character);
            }
          }
          break;
      }
    }
    if (token === null) {
      return result.join("");
    } else {
      throw new Error("Unterminated token");
    }
  }
  static addPrefix(op, result) {
    switch (op) {
      case 2:
        result.push("#");
        break;
      case 3:
        result.push(".");
        break;
      case 4:
        result.push("/");
        break;
      case 5:
        result.push(";");
        break;
      case 6:
        result.push("?");
        break;
      case 7:
        result.push("&");
        break;
      default:
        return;
    }
  }
  static addSeparator(op, result) {
    switch (op) {
      case 3:
        result.push(".");
        break;
      case 4:
        result.push("/");
        break;
      case 5:
        result.push(";");
        break;
      case 6:
      case 7:
        result.push("&");
        break;
      default:
        result.push(",");
        return;
    }
  }
  static addValue(op, token, value, result, maxChar) {
    switch (op) {
      case 1:
      case 2:
        StdUriTemplate.addExpandedValue(null, value, result, maxChar, false);
        break;
      case 6:
      case 7:
        result.push(`${token}=`);
        StdUriTemplate.addExpandedValue(null, value, result, maxChar, true);
        break;
      case 5:
        result.push(token);
        StdUriTemplate.addExpandedValue("=", value, result, maxChar, true);
        break;
      case 3:
      case 4:
      case 0:
        StdUriTemplate.addExpandedValue(null, value, result, maxChar, true);
        break;
    }
  }
  static addValueElement(op, token, value, result, maxChar) {
    switch (op) {
      case 1:
      case 2:
        StdUriTemplate.addExpandedValue(null, value, result, maxChar, false);
        break;
      case 6:
      case 7:
      case 5:
      case 3:
      case 4:
      case 0:
        StdUriTemplate.addExpandedValue(null, value, result, maxChar, true);
        break;
    }
  }
  static isSurrogate(cp) {
    const codeUnit = cp.charCodeAt(0);
    return codeUnit >= 55296 && codeUnit <= 56319;
  }
  static isIprivate(cp) {
    return 57344 <= cp.charCodeAt(0) && cp.charCodeAt(0) <= 63743;
  }
  static isUcschar(cp) {
    const codePoint = cp.codePointAt(0) || 0;
    return 160 <= codePoint && codePoint <= 55295 || 63744 <= codePoint && codePoint <= 64975 || 65008 <= codePoint && codePoint <= 65519;
  }
  static addExpandedValue(prefix, value, result, maxChar, replaceReserved) {
    const stringValue = StdUriTemplate.convertNativeTypes(value);
    const codePoints = Array.from(stringValue);
    const max = maxChar !== -1 ? Math.min(maxChar, codePoints.length) : codePoints.length;
    let reservedBuffer = undefined;
    if (max > 0 && prefix != null) {
      result.push(prefix);
    }
    for (let i = 0;i < max; i++) {
      const character = codePoints[i];
      if (character === "%" && !replaceReserved) {
        reservedBuffer = [];
      }
      let toAppend = character;
      const cp = character.codePointAt(0) || 0;
      if (cp > 65535) {
        toAppend = encodeURIComponent(character);
      } else if (replaceReserved || StdUriTemplate.isUcschar(character) || StdUriTemplate.isIprivate(character)) {
        if (character === "!") {
          toAppend = "%21";
        } else {
          toAppend = encodeURIComponent(toAppend);
        }
      }
      if (reservedBuffer) {
        reservedBuffer.push(toAppend);
        if (reservedBuffer.length === 3) {
          let isEncoded = false;
          try {
            const reserved = reservedBuffer.join("");
            const decoded = decodeURIComponent(reservedBuffer.join(""));
            isEncoded = reserved !== decoded;
          } catch (e) {}
          if (isEncoded) {
            result.push(reservedBuffer.join(""));
          } else {
            result.push("%25");
            result.push(reservedBuffer.slice(1).join(""));
          }
          reservedBuffer = undefined;
        }
      } else {
        if (character === " ") {
          result.push("%20");
        } else if (character === "%") {
          result.push("%25");
        } else {
          result.push(toAppend);
        }
      }
    }
    if (reservedBuffer) {
      result.push("%25");
      result.push(reservedBuffer.slice(1).join(""));
    }
  }
  static isList(value) {
    return Array.isArray(value) || value instanceof Set;
  }
  static isMap(value) {
    return value instanceof Map || typeof value === "object";
  }
  static getSubstitutionType(value, col) {
    if (value === undefined || value === null) {
      return 0;
    } else if (StdUriTemplate.isNativeType(value)) {
      return 1;
    } else if (StdUriTemplate.isList(value)) {
      return 2;
    } else if (StdUriTemplate.isMap(value)) {
      return 3;
    } else {
      throw new Error(`Illegal class passed as substitution, found ${typeof value} at col: ${col}`);
    }
  }
  static isEmpty(substType, value) {
    if (value === undefined || value === null) {
      return true;
    } else {
      switch (substType) {
        case 1:
          return false;
        case 2:
          return value.length === 0;
        case 3:
          return Object.keys(value).length === 0;
        default:
          return true;
      }
    }
  }
  static isNativeType(value) {
    return typeof value === "string" || typeof value === "number" || typeof value === "boolean";
  }
  static convertNativeTypes(value) {
    if (typeof value === "string") {
      return value;
    } else if (typeof value === "number" || typeof value === "boolean") {
      return value.toString();
    } else {
      throw new Error(`Illegal class passed as substitution, found ${typeof value}`);
    }
  }
  static isHexDigit(c) {
    return c >= "0" && c <= "9" || c >= "A" && c <= "F" || c >= "a" && c <= "f";
  }
  static checkVarname(token, col) {
    if (token.startsWith(".") || token.endsWith(".")) {
      throw new Error(`Invalid variable name at col: ${col}`);
    }
    if (token.indexOf("..") !== -1) {
      throw new Error(`Invalid variable name at col: ${col}`);
    }
    for (let i = 0;i < token.length; i++) {
      if (token.charAt(i) === "%") {
        if (i + 2 >= token.length || !StdUriTemplate.isHexDigit(token.charAt(i + 1)) || !StdUriTemplate.isHexDigit(token.charAt(i + 2))) {
          throw new Error(`Invalid variable name at col: ${col}`);
        }
      }
    }
  }
  static expandToken(operator, token, composite, maxChar, firstToken, substitutions, result, col) {
    if (token.length === 0) {
      throw new Error(`Found an empty token at col: ${col}`);
    }
    StdUriTemplate.checkVarname(token, col);
    const value = substitutions[token];
    const substType = StdUriTemplate.getSubstitutionType(value, col);
    if (substType === 0 || StdUriTemplate.isEmpty(substType, value)) {
      return false;
    }
    if (firstToken) {
      StdUriTemplate.addPrefix(operator, result);
    } else {
      StdUriTemplate.addSeparator(operator, result);
    }
    switch (substType) {
      case 1:
        StdUriTemplate.addStringValue(operator, token, value, result, maxChar);
        break;
      case 2:
        StdUriTemplate.addListValue(operator, token, value, result, maxChar, composite);
        break;
      case 3:
        StdUriTemplate.addMapValue(operator, token, value, result, maxChar, composite);
        break;
    }
    return true;
  }
  static addStringValue(operator, token, value, result, maxChar) {
    StdUriTemplate.addValue(operator, token, value, result, maxChar);
  }
  static addListValue(operator, token, value, result, maxChar, composite) {
    let first = true;
    for (const v of value) {
      if (first) {
        StdUriTemplate.addValue(operator, token, v, result, maxChar);
        first = false;
      } else {
        if (composite) {
          StdUriTemplate.addSeparator(operator, result);
          StdUriTemplate.addValue(operator, token, v, result, maxChar);
        } else {
          result.push(",");
          StdUriTemplate.addValueElement(operator, token, v, result, maxChar);
        }
      }
    }
  }
  static addMapValue(operator, token, value, result, maxChar, composite) {
    let first = true;
    if (maxChar !== -1) {
      throw new Error("Value trimming is not allowed on Maps");
    }
    for (const key in value) {
      const v = value[key];
      if (composite) {
        if (!first) {
          StdUriTemplate.addSeparator(operator, result);
        }
        StdUriTemplate.addValueElement(operator, token, key, result, maxChar);
        result.push("=");
      } else {
        if (first) {
          StdUriTemplate.addValue(operator, token, key, result, maxChar);
        } else {
          result.push(",");
          StdUriTemplate.addValueElement(operator, token, key, result, maxChar);
        }
        result.push(",");
      }
      StdUriTemplate.addValueElement(operator, token, v, result, maxChar);
      first = false;
    }
  }
}

// node_modules/@microsoft/kiota-abstractions/dist/es/src/dateOnly.js
class DateOnly {
  constructor({ year = 0, month = 1, day = 1 }) {
    this.day = day;
    this.month = month;
    this.year = year;
  }
  static fromDate(date) {
    if (!date) {
      throw new Error("Date cannot be undefined");
    }
    const result = new DateOnly({
      year: date.getFullYear(),
      month: date.getMonth() + 1,
      day: date.getDate()
    });
    return result;
  }
  static parse(value) {
    if (!value || value.length === 0) {
      return;
    }
    const exec = /^(\d{4,})-(0[1-9]|1[012])-(0[1-9]|[12]\d|3[01])$/gi.exec(value);
    if (exec) {
      const year = parseInt(exec[1], 10);
      const month = parseInt(exec[2], 10);
      const day = parseInt(exec[3], 10);
      return new DateOnly({ year, month, day });
    }
    const ticks = Date.parse(value);
    if (!isNaN(ticks)) {
      const date = new Date(ticks);
      return this.fromDate(date);
    }
    throw new Error(`Value is not a valid date-only representation: ${value}`);
  }
  toString() {
    return `${formatSegment(this.year, 4)}-${formatSegment(this.month)}-${formatSegment(this.day)}`;
  }
}
var formatSegment = (segment, digits = 2) => {
  return segment.toString().padStart(digits, "0");
};

// node_modules/@microsoft/kiota-abstractions/dist/es/src/duration.js
var import_tinyduration = __toESM(require_dist(), 1);

class Duration {
  constructor({ years = 0, months = 0, weeks = 0, days = 0, hours = 0, minutes = 0, seconds = 0, negative = false }) {
    if (years < 0 || years > 9999) {
      throw new Error("Year must be between 0 and 9999");
    }
    if (months < 0) {
      throw new Error("Month must be greater or equal to 0");
    }
    if (weeks < 0) {
      throw new Error("Week must be greater or equal to 0");
    }
    if (days < 0) {
      throw new Error("Day must be greater or equal to 0");
    }
    if (hours < 0) {
      throw new Error("Hour must be greater or equal to 0");
    }
    if (minutes < 0) {
      throw new Error("Minute must be greater or equal to 0");
    }
    if (seconds < 0) {
      throw new Error("Second must be greater or equal to 0");
    }
    if (weeks > 0 && (days > 0 || hours > 0 || minutes > 0 || seconds > 0)) {
      throw new Error("Cannot have weeks and days or hours or minutes or seconds");
    }
    if ((years > 0 || months > 0) && weeks > 0) {
      throw new Error("Cannot have weeks and months or weeks and years");
    }
    this.years = years;
    this.months = months;
    this.weeks = weeks;
    this.days = days;
    this.hours = hours;
    this.minutes = minutes;
    this.seconds = seconds;
    this.negative = negative;
  }
  static parse(value) {
    var _a, _b, _c, _d, _e, _f, _g, _h;
    if (!value || value.length === 0) {
      return;
    }
    const duration = import_tinyduration.parse(value);
    return new Duration({
      years: (_a = duration.years) !== null && _a !== undefined ? _a : 0,
      months: (_b = duration.months) !== null && _b !== undefined ? _b : 0,
      weeks: (_c = duration.weeks) !== null && _c !== undefined ? _c : 0,
      days: (_d = duration.days) !== null && _d !== undefined ? _d : 0,
      hours: (_e = duration.hours) !== null && _e !== undefined ? _e : 0,
      minutes: (_f = duration.minutes) !== null && _f !== undefined ? _f : 0,
      seconds: (_g = duration.seconds) !== null && _g !== undefined ? _g : 0,
      negative: (_h = duration.negative) !== null && _h !== undefined ? _h : false
    });
  }
  toString() {
    return import_tinyduration.serialize(this);
  }
}

// node_modules/@microsoft/kiota-abstractions/dist/es/src/recordWithCaseInsensitiveKeys.js
var dictionaryWithCanonicalKeys = (canon) => {
  const keysNormalizationMap = new Map;
  return new Proxy({}, {
    get: (target, prop) => {
      const normalKey = canon(prop);
      return Reflect.get(target, normalKey);
    },
    set: (target, prop, value) => {
      const nonNormalKey = prop.toString();
      const normalKey = canon(prop);
      keysNormalizationMap.set(normalKey, nonNormalKey);
      return Reflect.set(target, normalKey, value);
    },
    has: (_, prop) => {
      const normalKey = canon(prop);
      return keysNormalizationMap.has(normalKey);
    },
    defineProperty: (target, prop, attribs) => {
      const nonNormalKey = prop.toString();
      const normalKey = canon(prop);
      keysNormalizationMap.set(normalKey, nonNormalKey);
      return Reflect.defineProperty(target, normalKey, attribs);
    },
    deleteProperty: (target, prop) => {
      const normalKey = canon(prop);
      keysNormalizationMap.delete(normalKey);
      return Reflect.deleteProperty(target, normalKey);
    },
    getOwnPropertyDescriptor: (target, prop) => {
      return Reflect.getOwnPropertyDescriptor(target, canon(prop));
    },
    ownKeys: () => {
      return [...keysNormalizationMap.values()];
    }
  });
};
var createRecordWithCaseInsensitiveKeys = () => {
  const record = dictionaryWithCanonicalKeys((p) => typeof p === "string" ? p.toLowerCase() : p.toString().toLowerCase());
  return record;
};

// node_modules/@microsoft/kiota-abstractions/dist/es/src/headers.js
class Headers extends Map {
  constructor(entries) {
    super();
    this.headers = createRecordWithCaseInsensitiveKeys();
    this.singleValueHeaders = new Set(["Content-Type", "Content-Encoding", "Content-Length"]);
    if (entries) {
      entries.forEach(([key, value]) => {
        this.headers[key] = value;
      });
    }
  }
  set(headerName, headerValue) {
    this.add(headerName, ...headerValue);
    return this;
  }
  get(headerName) {
    if (!headerName) {
      throw new Error("headerName cannot be null or empty");
    }
    return this.headers[headerName];
  }
  has(key) {
    return !!key && !!this.headers[key];
  }
  delete(headerName) {
    if (!headerName) {
      throw new Error("headerName cannot be null or empty");
    }
    if (this.headers[headerName]) {
      delete this.headers[headerName];
      return true;
    }
    return false;
  }
  clear() {
    for (const header in this.headers) {
      if (Object.prototype.hasOwnProperty.call(this.headers, header)) {
        delete this.headers[header];
      }
    }
  }
  forEach(callbackfn, thisArg) {
    for (const header in this.headers) {
      if (Object.prototype.hasOwnProperty.call(this.headers, header)) {
        callbackfn.call(thisArg, this.headers[header], header, this);
      }
    }
  }
  add(headerName, ...headerValues) {
    if (!headerName) {
      console.error("headerName cannot be null or empty");
      return false;
    }
    if (!headerValues) {
      console.error("headerValues cannot be null");
      return false;
    }
    if (headerValues.length === 0) {
      return false;
    }
    if (this.singleValueHeaders.has(headerName)) {
      this.headers[headerName] = new Set([headerValues[0]]);
    } else if (this.headers[headerName]) {
      headerValues.forEach((headerValue) => this.headers[headerName].add(headerValue));
    } else {
      this.headers[headerName] = new Set(headerValues);
    }
    return true;
  }
  tryAdd(headerName, headerValue) {
    if (!headerName) {
      throw new Error("headerName cannot be null or empty");
    }
    if (!headerValue) {
      throw new Error("headerValue cannot be null");
    }
    if (!this.headers[headerName]) {
      this.headers[headerName] = new Set([headerValue]);
      return true;
    }
    return false;
  }
  remove(headerName, headerValue) {
    if (!headerName) {
      throw new Error("headerName cannot be null or empty");
    }
    if (!headerValue) {
      throw new Error("headerValue cannot be null");
    }
    if (this.headers[headerName]) {
      const result = this.headers[headerName].delete(headerValue);
      if (this.headers[headerName].size === 0) {
        delete this.headers[headerName];
      }
      return result;
    }
    return false;
  }
  addAll(headers) {
    if (!headers) {
      throw new Error("headers cannot be null");
    }
    for (const header in headers.headers) {
      if (Object.prototype.hasOwnProperty.call(headers.headers, header)) {
        headers.headers[header].forEach((value) => this.add(header, value));
      }
    }
  }
  addAllRaw(headers) {
    if (!headers) {
      throw new Error("headers cannot be null");
    }
    for (const header in headers) {
      if (Object.prototype.hasOwnProperty.call(headers, header)) {
        const headerValues = headers[header];
        if (Array.isArray(headerValues)) {
          this.add(header, ...headerValues);
        } else {
          this.add(header, headerValues);
        }
      }
    }
  }
  tryGetValue(key) {
    if (!key) {
      throw new Error("key cannot be null or empty");
    }
    return this.headers[key] ? Array.from(this.headers[key]) : null;
  }
  toString() {
    return JSON.stringify(this.headers, (_key, value) => value instanceof Set ? [...value] : value);
  }
  isEmpty() {
    return Object.keys(this.headers).length === 0;
  }
  keys() {
    return Object.keys(this.headers)[Symbol.iterator]();
  }
  entries() {
    return Object.entries(this.headers)[Symbol.iterator]();
  }
}

// node_modules/@microsoft/kiota-abstractions/dist/es/src/multipartBody.js
class MultipartBody {
  constructor() {
    this._parts = {};
    this._boundary = createGuid().replace(/-/g, "");
  }
  addOrReplacePart(partName, partContentType, content, serializationCallback, fileName) {
    if (!partName)
      throw new Error("partName cannot be undefined");
    if (!partContentType) {
      throw new Error("partContentType cannot be undefined");
    }
    if (!content)
      throw new Error("content cannot be undefined");
    const normalizePartName = this.normalizePartName(partName);
    this._parts[normalizePartName] = {
      contentType: partContentType,
      content,
      originalName: partName,
      fileName,
      serializationCallback
    };
  }
  getPartValue(partName) {
    if (!partName)
      throw new Error("partName cannot be undefined");
    const normalizePartName = this.normalizePartName(partName);
    const candidate = this._parts[normalizePartName];
    if (!candidate)
      return;
    return candidate.content;
  }
  removePart(partName) {
    if (!partName)
      throw new Error("partName cannot be undefined");
    const normalizePartName = this.normalizePartName(partName);
    if (!this._parts[normalizePartName])
      return false;
    delete this._parts[normalizePartName];
    return true;
  }
  getBoundary() {
    return this._boundary;
  }
  normalizePartName(original) {
    return original.toLocaleLowerCase();
  }
  listParts() {
    return this._parts;
  }
}

// node_modules/@microsoft/kiota-abstractions/dist/es/src/timeOnly.js
class TimeOnly {
  constructor({ hours = 0, minutes = 0, seconds = 0, picoseconds = 0 }) {
    if (hours < 0 || hours > 23) {
      throw new Error("Hour must be between 0 and 23");
    }
    if (minutes < 0 || minutes > 59) {
      throw new Error("Minute must be between 0 and 59");
    }
    if (seconds < 0 || seconds > 59) {
      throw new Error("Second must be between 0 and 59");
    }
    if (picoseconds < 0 || picoseconds > 9999999) {
      throw new Error("Millisecond must be between 0 and 9999999");
    }
    this.hours = hours;
    this.minutes = minutes;
    this.seconds = seconds;
    this.picoseconds = picoseconds;
  }
  static fromDate(date) {
    if (!date) {
      throw new Error("Date cannot be undefined");
    }
    return new TimeOnly({
      hours: date.getHours(),
      minutes: date.getMinutes(),
      seconds: date.getSeconds(),
      picoseconds: date.getMilliseconds() * 1e4
    });
  }
  static parse(value) {
    var _a, _b, _c, _d;
    if (!value || value.length === 0) {
      return;
    }
    const ticks = Date.parse(value);
    if (isNaN(ticks)) {
      const exec = /^([01]\d|2[0-3]):([0-5]\d):([0-5]\d)(?:[.](\d{1,12}))?$/gi.exec(value);
      if (exec) {
        const hours = parseInt((_a = exec[1]) !== null && _a !== undefined ? _a : "", 10);
        const minutes = parseInt((_b = exec[2]) !== null && _b !== undefined ? _b : "", 10);
        const seconds = parseInt((_c = exec[3]) !== null && _c !== undefined ? _c : "", 10);
        const milliseconds = parseInt((_d = exec[4]) !== null && _d !== undefined ? _d : "0", 10);
        return new TimeOnly({
          hours,
          minutes,
          seconds,
          picoseconds: milliseconds
        });
      } else {
        throw new Error("Value is not a valid time-only representation");
      }
    } else {
      const date = new Date(ticks);
      return this.fromDate(date);
    }
  }
  toString() {
    return `${formatSegment(this.hours, 2)}:${formatSegment(this.minutes, 2)}:${formatSegment(this.seconds, 2)}.${formatSegment(this.picoseconds, 7)}`;
  }
}

// node_modules/@microsoft/kiota-abstractions/dist/es/src/requestInformation.js
class RequestInformation {
  constructor(httpMethod, urlTemplate, pathParameters) {
    this.pathParameters = createRecordWithCaseInsensitiveKeys();
    this.queryParameters = createRecordWithCaseInsensitiveKeys();
    this.headers = new Headers;
    this._requestOptions = createRecordWithCaseInsensitiveKeys();
    this.setContentFromParsable = (requestAdapter, contentType, value, modelSerializerFunction) => {
      import_api.trace.getTracer(RequestInformation.tracerKey).startActiveSpan("setContentFromParsable", (span) => {
        try {
          const writer = this.getSerializationWriter(requestAdapter, contentType, value);
          if (value instanceof MultipartBody) {
            contentType += "; boundary=" + value.getBoundary();
          }
          if (!this.headers) {
            this.headers = new Headers;
          }
          if (Array.isArray(value)) {
            span.setAttribute(RequestInformation.requestTypeKey, "object[]");
            writer.writeCollectionOfObjectValues(undefined, value, modelSerializerFunction);
          } else {
            span.setAttribute(RequestInformation.requestTypeKey, "object");
            writer.writeObjectValue(undefined, value, modelSerializerFunction);
          }
          this.setContentAndContentType(writer, contentType);
        } finally {
          span.end();
        }
      });
    };
    this.setContentAndContentType = (writer, contentType) => {
      if (contentType) {
        this.headers.tryAdd(RequestInformation.contentTypeHeader, contentType);
      }
      this.content = writer.getSerializedContent();
    };
    this.getSerializationWriter = (requestAdapter, contentType, ...values) => {
      if (!requestAdapter)
        throw new Error("httpCore cannot be undefined");
      if (!contentType)
        throw new Error("contentType cannot be undefined");
      if (!values || values.length === 0) {
        throw new Error("values cannot be undefined or empty");
      }
      return requestAdapter.getSerializationWriterFactory().getSerializationWriter(contentType);
    };
    this.setContentFromScalar = (requestAdapter, contentType, value) => {
      import_api.trace.getTracer(RequestInformation.tracerKey).startActiveSpan("setContentFromScalar", (span) => {
        try {
          const writer = this.getSerializationWriter(requestAdapter, contentType, value);
          if (!this.headers) {
            this.headers = new Headers;
          }
          if (Array.isArray(value)) {
            span.setAttribute(RequestInformation.requestTypeKey, "[]");
            writer.writeCollectionOfPrimitiveValues(undefined, value);
          } else {
            const valueType = typeof value;
            span.setAttribute(RequestInformation.requestTypeKey, valueType);
            if (!value) {
              writer.writeNullValue(undefined);
            } else if (valueType === "boolean") {
              writer.writeBooleanValue(undefined, value);
            } else if (valueType === "string") {
              writer.writeStringValue(undefined, value);
            } else if (value instanceof Date) {
              writer.writeDateValue(undefined, value);
            } else if (value instanceof DateOnly) {
              writer.writeDateOnlyValue(undefined, value);
            } else if (value instanceof TimeOnly) {
              writer.writeTimeOnlyValue(undefined, value);
            } else if (value instanceof Duration) {
              writer.writeDurationValue(undefined, value);
            } else if (valueType === "number") {
              writer.writeNumberValue(undefined, value);
            } else if (Array.isArray(value)) {
              writer.writeCollectionOfPrimitiveValues(undefined, value);
            } else {
              throw new Error(`encountered unknown value type during serialization ${valueType}`);
            }
          }
          this.setContentAndContentType(writer, contentType);
        } finally {
          span.end();
        }
      });
    };
    this.setStreamContent = (value, contentType) => {
      if (!contentType) {
        contentType = RequestInformation.binaryContentType;
      }
      this.headers.tryAdd(RequestInformation.contentTypeHeader, contentType);
      this.content = value;
    };
    if (httpMethod) {
      this.httpMethod = httpMethod;
    }
    if (urlTemplate) {
      this.urlTemplate = urlTemplate;
    }
    if (pathParameters) {
      this.pathParameters = pathParameters;
    }
  }
  get URL() {
    const rawUrl = this.pathParameters[RequestInformation.raw_url_key];
    if (this.uri) {
      return this.uri;
    } else if (rawUrl) {
      this.URL = rawUrl;
      return rawUrl;
    } else if (!this.queryParameters) {
      throw new Error("queryParameters cannot be undefined");
    } else if (!this.pathParameters) {
      throw new Error("pathParameters cannot be undefined");
    } else if (!this.urlTemplate) {
      throw new Error("urlTemplate cannot be undefined");
    } else {
      const data = {};
      for (const key in this.queryParameters) {
        if (this.queryParameters[key] !== null && this.queryParameters[key] !== undefined) {
          data[key] = this.normalizeValue(this.queryParameters[key]);
        }
      }
      for (const key in this.pathParameters) {
        if (this.pathParameters[key] !== null && this.pathParameters[key] !== undefined) {
          data[key] = this.normalizeValue(this.pathParameters[key]);
        }
      }
      return StdUriTemplate.expand(this.urlTemplate, data);
    }
  }
  set URL(url) {
    if (!url)
      throw new Error("URL cannot be undefined");
    this.uri = url;
    this.queryParameters = {};
    this.pathParameters = {};
  }
  getRequestOptions() {
    return this._requestOptions;
  }
  addRequestHeaders(source) {
    if (source) {
      this.headers.addAllRaw(source);
    }
  }
  addRequestOptions(options) {
    if (!options || options.length === 0)
      return;
    options.forEach((option) => {
      this._requestOptions[option.getKey()] = option;
    });
  }
  removeRequestOptions(...options) {
    if (!options || options.length === 0)
      return;
    options.forEach((option) => {
      delete this._requestOptions[option.getKey()];
    });
  }
  normalizeValue(value) {
    if (value instanceof DateOnly || value instanceof TimeOnly || value instanceof Duration) {
      return value.toString();
    }
    if (value instanceof Date) {
      return value.toISOString();
    }
    if (Array.isArray(value)) {
      return value.map((val) => this.normalizeValue(val));
    }
    return value;
  }
  setQueryStringParametersFromRawObject(q, p) {
    if (q === null || q === undefined)
      return;
    Object.entries(q).forEach(([k, v]) => {
      let key = k;
      if (p) {
        const keyCandidate = p[key];
        if (keyCandidate) {
          key = keyCandidate;
        }
      }
      if (typeof v === "boolean" || typeof v === "number" || typeof v === "string" || Array.isArray(v))
        this.queryParameters[key] = v;
      else if (v instanceof DateOnly || v instanceof TimeOnly || v instanceof Duration)
        this.queryParameters[key] = v.toString();
      else if (v instanceof Date)
        this.queryParameters[key] = v.toISOString();
      else if (v === undefined)
        this.queryParameters[key] = undefined;
    });
  }
  configure(config, queryParametersMapper) {
    if (!config)
      return;
    this.addRequestHeaders(config.headers);
    this.setQueryStringParametersFromRawObject(config.queryParameters, queryParametersMapper);
    this.addRequestOptions(config.options);
  }
}
RequestInformation.raw_url_key = "request-raw-url";
RequestInformation.binaryContentType = "application/octet-stream";
RequestInformation.contentTypeHeader = "Content-Type";
RequestInformation.tracerKey = "@microsoft/kiota-abstractions";
RequestInformation.requestTypeKey = "com.microsoft.kiota.request.type";

// node_modules/@microsoft/kiota-abstractions/dist/es/src/httpMethod.js
var HttpMethod;
(function(HttpMethod2) {
  HttpMethod2["GET"] = "GET";
  HttpMethod2["POST"] = "POST";
  HttpMethod2["PATCH"] = "PATCH";
  HttpMethod2["DELETE"] = "DELETE";
  HttpMethod2["OPTIONS"] = "OPTIONS";
  HttpMethod2["CONNECT"] = "CONNECT";
  HttpMethod2["TRACE"] = "TRACE";
  HttpMethod2["HEAD"] = "HEAD";
  HttpMethod2["PUT"] = "PUT";
})(HttpMethod || (HttpMethod = {}));
// node_modules/@microsoft/kiota-abstractions/dist/es/src/apiError.js
class DefaultApiError extends Error {
  constructor(message) {
    super(message);
    this.responseHeaders = {};
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/authentication/validateProtocol.js
var localhostStrings = new Set(["localhost", "[::1]", "::1", "127.0.0.1"]);

// node_modules/@microsoft/kiota-abstractions/dist/es/src/authentication/apiKeyAuthenticationProvider.js
var ApiKeyLocation;
(function(ApiKeyLocation2) {
  ApiKeyLocation2[ApiKeyLocation2["QueryParameter"] = 0] = "QueryParameter";
  ApiKeyLocation2[ApiKeyLocation2["Header"] = 1] = "Header";
})(ApiKeyLocation || (ApiKeyLocation = {}));
// node_modules/@microsoft/kiota-abstractions/dist/es/src/authentication/baseBearerTokenAuthenticationProvider.js
class BaseBearerTokenAuthenticationProvider {
  constructor(accessTokenProvider) {
    this.accessTokenProvider = accessTokenProvider;
    this.authenticateRequest = async (request, additionalAuthenticationContext) => {
      var _a;
      if (!request) {
        throw new Error("request info cannot be null");
      }
      if ((additionalAuthenticationContext === null || additionalAuthenticationContext === undefined ? undefined : additionalAuthenticationContext.claims) && request.headers.has(BaseBearerTokenAuthenticationProvider.authorizationHeaderKey)) {
        request.headers.delete(BaseBearerTokenAuthenticationProvider.authorizationHeaderKey);
      }
      if (!((_a = request.headers) === null || _a === undefined ? undefined : _a.has(BaseBearerTokenAuthenticationProvider.authorizationHeaderKey))) {
        const token = await this.accessTokenProvider.getAuthorizationToken(request.URL, additionalAuthenticationContext);
        if (!request.headers) {
          request.headers = new Headers;
        }
        if (token) {
          request.headers.add(BaseBearerTokenAuthenticationProvider.authorizationHeaderKey, `Bearer ${token}`);
        }
      }
    };
  }
}
BaseBearerTokenAuthenticationProvider.authorizationHeaderKey = "Authorization";
// node_modules/@microsoft/kiota-abstractions/dist/es/src/nativeResponseHandler.js
class NativeResponseHandler {
  handleResponse(response, errorMappings) {
    this.value = response;
    this.errorMappings = errorMappings;
    return Promise.resolve(undefined);
  }
}
// node_modules/@microsoft/kiota-abstractions/dist/es/src/nativeResponseWrapper.js
var _a;

class NativeResponseWrapper {
}
_a = NativeResponseWrapper;
NativeResponseWrapper.CallAndGetNative = async (originalCall, q, h, o) => {
  const responseHandler = new NativeResponseHandler;
  await originalCall(q, h, o, responseHandler);
  return responseHandler.value;
};
NativeResponseWrapper.CallAndGetNativeWithBody = async (originalCall, requestBody, q, h, o) => {
  const responseHandler = new NativeResponseHandler;
  await originalCall(requestBody, q, h, o, responseHandler);
  return responseHandler.value;
};
// node_modules/@microsoft/kiota-abstractions/dist/es/src/responseHandlerOptions.js
var ResponseHandlerOptionKey = "ResponseHandlerOptionKey";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/fetchRequestAdapter.js
var import_api2 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/customFetchHandler.js
class CustomFetchHandler {
  constructor(customFetch) {
    this.customFetch = customFetch;
  }
  async execute(url, requestInit) {
    return await this.customFetch(url, requestInit);
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/httpClient.js
class HttpClient {
  constructor(customFetch, ...middlewares) {
    this.customFetch = customFetch;
    middlewares = (middlewares === null || middlewares === undefined ? undefined : middlewares.length) && middlewares[0] ? middlewares : MiddlewareFactory.getDefaultMiddlewares(customFetch);
    if (this.customFetch) {
      middlewares.push(new CustomFetchHandler(this.customFetch));
    }
    this.setMiddleware(...middlewares);
  }
  setMiddleware(...middleware) {
    for (let i = 0;i < middleware.length - 1; i++) {
      middleware[i].next = middleware[i + 1];
    }
    this.middleware = middleware[0];
  }
  async executeFetch(url, requestInit, requestOptions) {
    if (this.middleware) {
      return await this.middleware.execute(url, requestInit, requestOptions);
    } else if (this.customFetch) {
      return this.customFetch(url, requestInit);
    }
    throw new Error("Please provide middlewares or a custom fetch function to execute the request");
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/observabilityOptions.js
var ObservabilityOptionKey = "ObservabilityOptionKey";

class ObservabilityOptionsImpl {
  constructor(originalOptions) {
    this._originalOptions = originalOptions !== null && originalOptions !== undefined ? originalOptions : {};
  }
  getKey() {
    return ObservabilityOptionKey;
  }
  get includeEUIIAttributes() {
    return this._originalOptions.includeEUIIAttributes;
  }
  set includeEUIIAttributes(value) {
    this._originalOptions.includeEUIIAttributes = value;
  }
  getTracerInstrumentationName() {
    return "@microsoft/kiota-http-fetchlibrary";
  }
}
var getObservabilityOptionsFromRequest = (requestOptions) => {
  if (requestOptions) {
    const observabilityOptions = requestOptions[ObservabilityOptionKey];
    if (observabilityOptions instanceof ObservabilityOptionsImpl) {
      return observabilityOptions;
    }
  }
  return;
};

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/fetchRequestAdapter.js
class FetchRequestAdapter {
  getSerializationWriterFactory() {
    return this.serializationWriterFactory;
  }
  getParseNodeFactory() {
    return this.parseNodeFactory;
  }
  getBackingStoreFactory() {
    return this.backingStoreFactory;
  }
  constructor(authenticationProvider2, parseNodeFactory2 = new ParseNodeFactoryRegistry, serializationWriterFactory2 = new SerializationWriterFactoryRegistry, httpClient = new HttpClient, observabilityOptions = new ObservabilityOptionsImpl, backingStoreFactory2 = new InMemoryBackingStoreFactory) {
    this.authenticationProvider = authenticationProvider2;
    this.parseNodeFactory = parseNodeFactory2;
    this.serializationWriterFactory = serializationWriterFactory2;
    this.httpClient = httpClient;
    this.backingStoreFactory = backingStoreFactory2;
    this.baseUrl = "";
    this.getResponseContentType = (response) => {
      var _a2;
      const header = (_a2 = response.headers.get("content-type")) === null || _a2 === undefined ? undefined : _a2.toLowerCase();
      if (!header)
        return;
      const segments = header.split(";");
      if (segments.length === 0)
        return;
      else
        return segments[0];
    };
    this.getResponseHandler = (response) => {
      const options = response.getRequestOptions();
      const responseHandlerOption = options[ResponseHandlerOptionKey];
      return responseHandlerOption === null || responseHandlerOption === undefined ? undefined : responseHandlerOption.responseHandler;
    };
    this.sendCollectionOfPrimitive = (requestInfo, responseType, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "sendCollectionOfPrimitive", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        } else {
          try {
            await this.throwIfFailedResponse(response, errorMappings, span);
            if (this.shouldReturnUndefined(response))
              return;
            switch (responseType) {
              case "string":
              case "number":
              case "boolean":
              case "Date":
              case "Duration":
              case "DateOnly":
              case "TimeOnly":
                const rootNode = await this.getRootParseNode(response);
                return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan(`getCollectionOf${responseType}Value`, (deserializeSpan) => {
                  try {
                    span.setAttribute(FetchRequestAdapter.responseTypeAttributeKey, responseType);
                    if (responseType === "string") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else if (responseType === "number") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else if (responseType === "boolean") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else if (responseType === "Date") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else if (responseType === "Duration") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else if (responseType === "DateOnly") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else if (responseType === "TimeOnly") {
                      return rootNode.getCollectionOfPrimitiveValues(responseType);
                    } else {
                      throw new Error("unexpected type to deserialize");
                    }
                  } finally {
                    deserializeSpan.end();
                  }
                });
            }
          } finally {
            await this.purgeResponseBody(response);
          }
        }
      });
    };
    this.sendCollection = (requestInfo, deserialization, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "sendCollection", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        } else {
          try {
            await this.throwIfFailedResponse(response, errorMappings, span);
            if (this.shouldReturnUndefined(response))
              return;
            const rootNode = await this.getRootParseNode(response);
            return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getCollectionOfObjectValues", (deserializeSpan) => {
              try {
                const result = rootNode.getCollectionOfObjectValues(deserialization);
                span.setAttribute(FetchRequestAdapter.responseTypeAttributeKey, "object[]");
                return result;
              } finally {
                deserializeSpan.end();
              }
            });
          } finally {
            await this.purgeResponseBody(response);
          }
        }
      });
    };
    this.startTracingSpan = (requestInfo, methodName, callback) => {
      var _a2;
      const urlTemplate = decodeURIComponent((_a2 = requestInfo.urlTemplate) !== null && _a2 !== undefined ? _a2 : "");
      const telemetryPathValue = urlTemplate.replace(/\{\?[^}]+\}/gi, "");
      return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan(`${methodName} - ${telemetryPathValue}`, async (span) => {
        try {
          span.setAttribute("url.uri_template", urlTemplate);
          return await callback(span);
        } finally {
          span.end();
        }
      });
    };
    this.send = (requestInfo, deserializer, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "send", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        } else {
          try {
            await this.throwIfFailedResponse(response, errorMappings, span);
            if (this.shouldReturnUndefined(response))
              return;
            const rootNode = await this.getRootParseNode(response);
            return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getObjectValue", (deserializeSpan) => {
              try {
                span.setAttribute(FetchRequestAdapter.responseTypeAttributeKey, "object");
                const result = rootNode.getObjectValue(deserializer);
                return result;
              } finally {
                deserializeSpan.end();
              }
            });
          } finally {
            await this.purgeResponseBody(response);
          }
        }
      });
    };
    this.sendPrimitive = (requestInfo, responseType, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "sendPrimitive", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        } else {
          try {
            await this.throwIfFailedResponse(response, errorMappings, span);
            if (this.shouldReturnUndefined(response))
              return;
            switch (responseType) {
              case "ArrayBuffer":
                if (!response.body) {
                  return;
                }
                return await response.arrayBuffer();
              case "string":
              case "number":
              case "boolean":
              case "Date":
                const rootNode = await this.getRootParseNode(response);
                span.setAttribute(FetchRequestAdapter.responseTypeAttributeKey, responseType);
                return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan(`get${responseType}Value`, (deserializeSpan) => {
                  try {
                    if (responseType === "string") {
                      return rootNode.getStringValue();
                    } else if (responseType === "number") {
                      return rootNode.getNumberValue();
                    } else if (responseType === "boolean") {
                      return rootNode.getBooleanValue();
                    } else if (responseType === "Date") {
                      return rootNode.getDateValue();
                    } else if (responseType === "Duration") {
                      return rootNode.getDurationValue();
                    } else if (responseType === "DateOnly") {
                      return rootNode.getDateOnlyValue();
                    } else if (responseType === "TimeOnly") {
                      return rootNode.getTimeOnlyValue();
                    } else {
                      throw new Error("unexpected type to deserialize");
                    }
                  } finally {
                    deserializeSpan.end();
                  }
                });
            }
          } finally {
            await this.purgeResponseBody(response);
          }
        }
      });
    };
    this.sendNoResponseContent = (requestInfo, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "sendNoResponseContent", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        }
        try {
          await this.throwIfFailedResponse(response, errorMappings, span);
        } finally {
          await this.purgeResponseBody(response);
        }
      });
    };
    this.sendEnum = (requestInfo, enumObject, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "sendEnum", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        } else {
          try {
            await this.throwIfFailedResponse(response, errorMappings, span);
            if (this.shouldReturnUndefined(response))
              return;
            const rootNode = await this.getRootParseNode(response);
            return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getEnumValue", (deserializeSpan) => {
              try {
                span.setAttribute(FetchRequestAdapter.responseTypeAttributeKey, "enum");
                const result = rootNode.getEnumValue(enumObject);
                return result;
              } finally {
                deserializeSpan.end();
              }
            });
          } finally {
            await this.purgeResponseBody(response);
          }
        }
      });
    };
    this.sendCollectionOfEnum = (requestInfo, enumObject, errorMappings) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      return this.startTracingSpan(requestInfo, "sendCollectionOfEnum", async (span) => {
        const response = await this.getHttpResponseMessage(requestInfo, span);
        const responseHandler2 = this.getResponseHandler(requestInfo);
        if (responseHandler2) {
          span.addEvent(FetchRequestAdapter.eventResponseHandlerInvokedKey);
          return await responseHandler2.handleResponse(response, errorMappings);
        } else {
          try {
            await this.throwIfFailedResponse(response, errorMappings, span);
            if (this.shouldReturnUndefined(response))
              return;
            const rootNode = await this.getRootParseNode(response);
            return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getCollectionOfEnumValues", (deserializeSpan) => {
              try {
                const result = rootNode.getCollectionOfEnumValues(enumObject);
                span.setAttribute(FetchRequestAdapter.responseTypeAttributeKey, "enum[]");
                return result;
              } finally {
                deserializeSpan.end();
              }
            });
          } finally {
            await this.purgeResponseBody(response);
          }
        }
      });
    };
    this.enableBackingStore = (backingStoreFactory3) => {
      if (this.parseNodeFactory instanceof ParseNodeFactoryRegistry) {
        this.parseNodeFactory = enableBackingStoreForParseNodeFactory(this.parseNodeFactory, this.parseNodeFactory);
      } else {
        throw new Error("parseNodeFactory is not a ParseNodeFactoryRegistry");
      }
      if (this.serializationWriterFactory instanceof SerializationWriterFactoryRegistry && this.parseNodeFactory instanceof ParseNodeFactoryRegistry) {
        this.serializationWriterFactory = enableBackingStoreForSerializationWriterFactory(this.serializationWriterFactory, this.parseNodeFactory, this.serializationWriterFactory);
      } else {
        throw new Error("serializationWriterFactory is not a SerializationWriterFactoryRegistry or parseNodeFactory is not a ParseNodeFactoryRegistry");
      }
      if (!this.serializationWriterFactory || !this.parseNodeFactory)
        throw new Error("unable to enable backing store");
      if (backingStoreFactory3) {
        this.backingStoreFactory = backingStoreFactory3;
      }
    };
    this.getRootParseNode = (response) => {
      return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getRootParseNode", async (span) => {
        try {
          const payload = await response.arrayBuffer();
          const responseContentType = this.getResponseContentType(response);
          if (!responseContentType)
            throw new Error("no response content type found for deserialization");
          return this.parseNodeFactory.getRootParseNode(responseContentType, payload);
        } finally {
          span.end();
        }
      });
    };
    this.shouldReturnUndefined = (response) => {
      return response.status === 204 || response.status === 304 || !response.body;
    };
    this.purgeResponseBody = async (response) => {
      if (!response.bodyUsed && response.body) {
        await response.arrayBuffer();
      }
    };
    this.throwIfFailedResponse = (response, errorMappings, spanForAttributes) => {
      return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("throwIfFailedResponse", async (span) => {
        var _a2, _b, _c;
        try {
          if (response.ok || response.status >= 300 && response.status < 400 && !response.headers.has(FetchRequestAdapter.locationHeaderName))
            return;
          spanForAttributes.setStatus({
            code: import_api2.SpanStatusCode.ERROR,
            message: "received_error_response"
          });
          const statusCode = response.status;
          const responseHeaders = {};
          response.headers.forEach((value, key) => {
            responseHeaders[key] = value.split(",");
          });
          const factory = errorMappings ? (_c = (_b = (_a2 = errorMappings[statusCode]) !== null && _a2 !== undefined ? _a2 : statusCode >= 400 && statusCode < 500 ? errorMappings._4XX : undefined) !== null && _b !== undefined ? _b : statusCode >= 500 && statusCode < 600 ? errorMappings._5XX : undefined) !== null && _c !== undefined ? _c : errorMappings.XXX : undefined;
          if (!factory) {
            spanForAttributes.setAttribute(FetchRequestAdapter.errorMappingFoundAttributeName, false);
            const error = new DefaultApiError("the server returned an unexpected status code and no error class is registered for this code " + statusCode);
            error.responseStatusCode = statusCode;
            error.responseHeaders = responseHeaders;
            spanForAttributes.recordException(error);
            throw error;
          }
          spanForAttributes.setAttribute(FetchRequestAdapter.errorMappingFoundAttributeName, true);
          const rootNode = await this.getRootParseNode(response);
          let deserializedError = import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getObjectValue", (deserializeSpan) => {
            try {
              return rootNode.getObjectValue(factory);
            } finally {
              deserializeSpan.end();
            }
          });
          spanForAttributes.setAttribute(FetchRequestAdapter.errorBodyFoundAttributeName, !!deserializedError);
          if (!deserializedError)
            deserializedError = new DefaultApiError("unexpected error type" + typeof deserializedError);
          const errorObject = deserializedError;
          errorObject.responseStatusCode = statusCode;
          errorObject.responseHeaders = responseHeaders;
          spanForAttributes.recordException(errorObject);
          throw errorObject;
        } finally {
          span.end();
        }
      });
    };
    this.getHttpResponseMessage = (requestInfo, spanForAttributes, claims) => {
      return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getHttpResponseMessage", async (span) => {
        try {
          if (!requestInfo) {
            throw new Error("requestInfo cannot be null");
          }
          this.setBaseUrlForRequestInformation(requestInfo);
          const additionalContext = {};
          if (claims) {
            additionalContext.claims = claims;
          }
          await this.authenticationProvider.authenticateRequest(requestInfo, additionalContext);
          const request = await this.getRequestFromRequestInformation(requestInfo, spanForAttributes);
          if (this.observabilityOptions) {
            requestInfo.addRequestOptions([this.observabilityOptions]);
          }
          let response = await this.httpClient.executeFetch(requestInfo.URL, request, requestInfo.getRequestOptions());
          response = await this.retryCAEResponseIfRequired(requestInfo, response, spanForAttributes, claims);
          if (response) {
            const responseContentLength = response.headers.get("Content-Length");
            if (responseContentLength) {
              spanForAttributes.setAttribute("http.response.body.size", parseInt(responseContentLength, 10));
            }
            const responseContentType = response.headers.get("Content-Type");
            if (responseContentType) {
              spanForAttributes.setAttribute("http.response.header.content-type", responseContentType);
            }
            spanForAttributes.setAttribute("http.response.status_code", response.status);
          }
          return response;
        } finally {
          span.end();
        }
      });
    };
    this.retryCAEResponseIfRequired = async (requestInfo, response, spanForAttributes, claims) => {
      return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("retryCAEResponseIfRequired", async (span) => {
        try {
          const responseClaims = this.getClaimsFromResponse(response, claims);
          if (responseClaims) {
            span.addEvent(FetchRequestAdapter.authenticateChallengedEventKey);
            spanForAttributes.setAttribute("http.request.resend_count", 1);
            await this.purgeResponseBody(response);
            return await this.getHttpResponseMessage(requestInfo, spanForAttributes, responseClaims);
          }
          return response;
        } finally {
          span.end();
        }
      });
    };
    this.getClaimsFromResponse = (response, claims) => {
      if (response.status === 401 && !claims) {
        const rawAuthenticateHeader = response.headers.get("WWW-Authenticate");
        if (rawAuthenticateHeader && /^Bearer /gi.test(rawAuthenticateHeader)) {
          const rawParameters = rawAuthenticateHeader.replace(/^Bearer /gi, "").split(",");
          for (const rawParameter of rawParameters) {
            const trimmedParameter = rawParameter.trim();
            if (/claims="[^"]+"/gi.test(trimmedParameter)) {
              return trimmedParameter.replace(/claims="([^"]+)"/gi, "$1");
            }
          }
        }
      }
      return;
    };
    this.setBaseUrlForRequestInformation = (requestInfo) => {
      requestInfo.pathParameters.baseurl = this.baseUrl;
    };
    this.getRequestFromRequestInformation = (requestInfo, spanForAttributes) => {
      return import_api2.trace.getTracer(this.observabilityOptions.getTracerInstrumentationName()).startActiveSpan("getRequestFromRequestInformation", async (span) => {
        var _a2, _b;
        try {
          const method = (_a2 = requestInfo.httpMethod) === null || _a2 === undefined ? undefined : _a2.toString();
          const uri = requestInfo.URL;
          spanForAttributes.setAttribute("http.request.method", method !== null && method !== undefined ? method : "");
          const uriContainsScheme = uri.includes("://");
          const schemeSplatUri = uri.split("://");
          if (uriContainsScheme) {
            spanForAttributes.setAttribute("server.address", schemeSplatUri[0]);
          }
          const uriWithoutScheme = uriContainsScheme ? schemeSplatUri[1] : uri;
          spanForAttributes.setAttribute("url.scheme", uriWithoutScheme.split("/")[0]);
          if (this.observabilityOptions.includeEUIIAttributes) {
            spanForAttributes.setAttribute("url.full", decodeURIComponent(uri));
          }
          const requestContentLength = requestInfo.headers.tryGetValue("Content-Length");
          if (requestContentLength) {
            spanForAttributes.setAttribute("http.response.body.size", parseInt(requestContentLength[0], 10));
          }
          const requestContentType = requestInfo.headers.tryGetValue("Content-Type");
          if (requestContentType) {
            spanForAttributes.setAttribute("http.request.header.content-type", requestContentType);
          }
          const headers2 = {};
          (_b = requestInfo.headers) === null || _b === undefined || _b.forEach((_, key) => {
            headers2[key.toString().toLocaleLowerCase()] = this.foldHeaderValue(requestInfo.headers.tryGetValue(key));
          });
          const request = {
            method,
            headers: headers2,
            body: requestInfo.content
          };
          return request;
        } finally {
          span.end();
        }
      });
    };
    this.foldHeaderValue = (value) => {
      if (!value || value.length < 1) {
        return "";
      } else if (value.length === 1) {
        return value[0];
      } else {
        return value.reduce((acc, val) => acc + val, ",");
      }
    };
    this.convertToNativeRequest = async (requestInfo) => {
      if (!requestInfo) {
        throw new Error("requestInfo cannot be null");
      }
      await this.authenticationProvider.authenticateRequest(requestInfo, undefined);
      return this.startTracingSpan(requestInfo, "convertToNativeRequest", async (span) => {
        const request = await this.getRequestFromRequestInformation(requestInfo, span);
        return request;
      });
    };
    if (!authenticationProvider2) {
      throw new Error("authentication provider cannot be null");
    }
    if (!parseNodeFactory2) {
      throw new Error("parse node factory cannot be null");
    }
    if (!serializationWriterFactory2) {
      throw new Error("serialization writer factory cannot be null");
    }
    if (!httpClient) {
      throw new Error("http client cannot be null");
    }
    if (!observabilityOptions) {
      throw new Error("observability options cannot be null");
    } else {
      this.observabilityOptions = new ObservabilityOptionsImpl(observabilityOptions);
    }
  }
}
FetchRequestAdapter.responseTypeAttributeKey = "com.microsoft.kiota.response.type";
FetchRequestAdapter.eventResponseHandlerInvokedKey = "com.microsoft.kiota.response_handler_invoked";
FetchRequestAdapter.errorMappingFoundAttributeName = "com.microsoft.kiota.error.mapping_found";
FetchRequestAdapter.errorBodyFoundAttributeName = "com.microsoft.kiota.error.body_found";
FetchRequestAdapter.locationHeaderName = "Location";
FetchRequestAdapter.authenticateChallengedEventKey = "com.microsoft.kiota.authenticate_challenge_received";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/authorizationHandler.js
var import_api3 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/utils/headersUtil.js
var getRequestHeader = (options, key) => {
  if (options === null || options === undefined ? undefined : options.headers) {
    return options.headers[key];
  }
  return;
};
var setRequestHeader = (options, key, value) => {
  if (options) {
    if (!options.headers) {
      options.headers = {};
    }
    options.headers[key] = value;
  }
};
var deleteRequestHeader = (options, key) => {
  if (options) {
    if (!options.headers) {
      options.headers = {};
    }
    delete options.headers[key];
  }
};
var appendRequestHeader = (options, key, value, separator = ", ") => {
  if (options) {
    if (!options.headers) {
      options.headers = {};
    }
    if (!options.headers[key]) {
      options.headers[key] = value;
    } else {
      options.headers[key] += `${separator}${value}`;
    }
  }
};

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/authorizationHandler.js
class AuthorizationHandler {
  constructor(authenticationProvider2) {
    this.authenticationProvider = authenticationProvider2;
    this.getClaimsFromResponse = (response, claims) => {
      if (response.status === 401 && !claims) {
        const rawAuthenticateHeader = response.headers.get("WWW-Authenticate");
        if (rawAuthenticateHeader && /^Bearer /gi.test(rawAuthenticateHeader)) {
          const rawParameters = rawAuthenticateHeader.replace(/^Bearer /gi, "").split(",");
          for (const rawParameter of rawParameters) {
            const trimmedParameter = rawParameter.trim();
            if (/claims="[^"]+"/gi.test(trimmedParameter)) {
              return trimmedParameter.replace(/claims="([^"]+)"/gi, "$1");
            }
          }
        }
      }
      return;
    };
    if (!authenticationProvider2) {
      throw new Error("authenticationProvider cannot be undefined");
    }
  }
  execute(url, requestInit, requestOptions) {
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api3.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("authorizationHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.authorization.enable", true);
          return this.executeInternal(url, requestInit, requestOptions, span);
        } finally {
          span.end();
        }
      });
    }
    return this.executeInternal(url, requestInit, requestOptions, undefined);
  }
  async executeInternal(url, fetchRequestInit, requestOptions, span) {
    var _a2, _b;
    if (this.authorizationIsPresent(fetchRequestInit)) {
      span === null || span === undefined || span.setAttribute("com.microsoft.kiota.handler.authorization.token_present", true);
      return await this.next.execute(url, fetchRequestInit, requestOptions);
    }
    const token = await this.authenticateRequest(url);
    setRequestHeader(fetchRequestInit, AuthorizationHandler.AUTHORIZATION_HEADER, `Bearer ${token}`);
    const response = await ((_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(url, fetchRequestInit, requestOptions));
    if (!response) {
      throw new Error("Response is undefined");
    }
    if (response.status !== 401) {
      return response;
    }
    const claims = this.getClaimsFromResponse(response);
    if (!claims) {
      return response;
    }
    span === null || span === undefined || span.addEvent("com.microsoft.kiota.handler.authorization.challenge_received");
    const claimsToken = await this.authenticateRequest(url, claims);
    setRequestHeader(fetchRequestInit, AuthorizationHandler.AUTHORIZATION_HEADER, `Bearer ${claimsToken}`);
    span === null || span === undefined || span.setAttribute("http.request.resend_count", 1);
    const retryResponse = await ((_b = this.next) === null || _b === undefined ? undefined : _b.execute(url, fetchRequestInit, requestOptions));
    if (!retryResponse) {
      throw new Error("Response is undefined");
    }
    return retryResponse;
  }
  authorizationIsPresent(request) {
    if (!request) {
      return false;
    }
    const authorizationHeader = getRequestHeader(request, AuthorizationHandler.AUTHORIZATION_HEADER);
    return authorizationHeader !== undefined && authorizationHeader !== null;
  }
  async authenticateRequest(url, claims) {
    const additionalAuthenticationContext = {};
    if (claims) {
      additionalAuthenticationContext.claims = claims;
    }
    return await this.authenticationProvider.accessTokenProvider.getAuthorizationToken(url, additionalAuthenticationContext);
  }
}
AuthorizationHandler.AUTHORIZATION_HEADER = "Authorization";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/chaosHandler.js
var import_api4 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/ChaosHandlerData.js
var methodStatusCode = {
  GET: [429, 500, 502, 503, 504],
  POST: [429, 500, 502, 503, 504, 507],
  PUT: [429, 500, 502, 503, 504, 507],
  PATCH: [429, 500, 502, 503, 504],
  DELETE: [429, 500, 502, 503, 504, 507]
};
var httpStatusCode = {
  100: "Continue",
  101: "Switching Protocols",
  102: "Processing",
  103: "Early Hints",
  200: "OK",
  201: "Created",
  202: "Accepted",
  203: "Non-Authoritative Information",
  204: "No Content",
  205: "Reset Content",
  206: "Partial Content",
  207: "Multi-Status",
  208: "Already Reported",
  226: "IM Used",
  300: "Multiple Choices",
  301: "Moved Permanently",
  302: "Found",
  303: "See Other",
  304: "Not Modified",
  305: "Use Proxy",
  307: "Temporary Redirect",
  308: "Permanent Redirect",
  400: "Bad Request",
  401: "Unauthorized",
  402: "Payment Required",
  403: "Forbidden",
  404: "Not Found",
  405: "Method Not Allowed",
  406: "Not Acceptable",
  407: "Proxy Authentication Required",
  408: "Request Timeout",
  409: "Conflict",
  410: "Gone",
  411: "Length Required",
  412: "Precondition Failed",
  413: "Payload Too Large",
  414: "URI Too Long",
  415: "Unsupported Media Type",
  416: "Range Not Satisfiable",
  417: "Expectation Failed",
  421: "Misdirected Request",
  422: "Unprocessable Entity",
  423: "Locked",
  424: "Failed Dependency",
  425: "Too Early",
  426: "Upgrade Required",
  428: "Precondition Required",
  429: "Too Many Requests",
  431: "Request Header Fields Too Large",
  451: "Unavailable For Legal Reasons",
  500: "Internal Server Error",
  501: "Not Implemented",
  502: "Bad Gateway",
  503: "Service Unavailable",
  504: "Gateway Timeout",
  505: "HTTP Version Not Supported",
  506: "Variant Also Negotiates",
  507: "Insufficient Storage",
  508: "Loop Detected",
  510: "Not Extended",
  511: "Network Authentication Required"
};

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/chaosStrategy.js
var ChaosStrategy;
(function(ChaosStrategy2) {
  ChaosStrategy2[ChaosStrategy2["MANUAL"] = 0] = "MANUAL";
  ChaosStrategy2[ChaosStrategy2["RANDOM"] = 1] = "RANDOM";
})(ChaosStrategy || (ChaosStrategy = {}));

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/chaosHandler.js
class ChaosHandler {
  constructor(options, manualMap) {
    this.options = {
      chaosStrategy: ChaosStrategy.RANDOM,
      statusMessage: "A random status message",
      chaosPercentage: 10
    };
    const chaosOptions = Object.assign(this.options, options);
    if (chaosOptions.chaosPercentage > 100 || chaosOptions.chaosPercentage < 0) {
      throw new Error("Chaos Percentage must be set to a value between 0 and 100.");
    }
    this.options = chaosOptions;
    this.manualMap = manualMap !== null && manualMap !== undefined ? manualMap : new Map;
  }
  generateRandomStatusCode(requestMethod) {
    const statusCodeArray = methodStatusCode[requestMethod];
    return statusCodeArray[Math.floor(Math.random() * statusCodeArray.length)];
  }
  getRelativeURL(chaosHandlerOptions, urlMethod) {
    const baseUrl = chaosHandlerOptions.baseUrl;
    if (baseUrl === undefined) {
      return urlMethod;
    }
    return urlMethod.replace(baseUrl, "").trim();
  }
  getStatusCode(chaosHandlerOptions, requestURL, requestMethod) {
    if (chaosHandlerOptions.chaosStrategy === ChaosStrategy.MANUAL) {
      if (chaosHandlerOptions.statusCode !== undefined) {
        return chaosHandlerOptions.statusCode;
      } else {
        const relativeURL = this.getRelativeURL(chaosHandlerOptions, requestURL);
        const definedResponses = this.manualMap.get(relativeURL);
        if (definedResponses !== undefined) {
          const mapCode = definedResponses.get(requestMethod);
          if (mapCode !== undefined) {
            return mapCode;
          }
        } else {
          this.manualMap.forEach((value, key) => {
            var _a2;
            const regexURL = new RegExp(key + "$");
            if (regexURL.test(relativeURL)) {
              const responseCode = (_a2 = this.manualMap.get(key)) === null || _a2 === undefined ? undefined : _a2.get(requestMethod);
              if (responseCode !== undefined) {
                return responseCode;
              }
            }
          });
        }
      }
    }
    return this.generateRandomStatusCode(requestMethod);
  }
  createResponseBody(chaosHandlerOptions, statusCode) {
    if (chaosHandlerOptions.responseBody) {
      return chaosHandlerOptions.responseBody;
    }
    let body;
    if (statusCode >= 400) {
      const codeMessage = httpStatusCode[statusCode];
      const errMessage = chaosHandlerOptions.statusMessage;
      body = {
        error: {
          code: codeMessage,
          message: errMessage
        }
      };
    } else {
      body = {};
    }
    return body;
  }
  createChaosResponse(url, fetchRequestInit) {
    var _a2;
    if (fetchRequestInit.method === undefined) {
      throw new Error("Request method must be defined.");
    }
    const requestMethod = fetchRequestInit.method;
    const statusCode = this.getStatusCode(this.options, url, requestMethod);
    const responseBody = this.createResponseBody(this.options, statusCode);
    const stringBody = typeof responseBody === "string" ? responseBody : JSON.stringify(responseBody);
    return {
      url,
      body: stringBody,
      status: statusCode,
      statusText: this.options.statusMessage,
      headers: (_a2 = this.options.headers) !== null && _a2 !== undefined ? _a2 : {}
    };
  }
  execute(url, requestInit, requestOptions) {
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api4.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("chaosHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.chaos.enable", true);
          return this.runChaos(url, requestInit, requestOptions);
        } finally {
          span.end();
        }
      });
    }
    return this.runChaos(url, requestInit, requestOptions);
  }
  runChaos(url, requestInit, requestOptions, span) {
    if (Math.floor(Math.random() * 100) < this.options.chaosPercentage) {
      span === null || span === undefined || span.addEvent(ChaosHandler.chaosHandlerTriggeredEventKey);
      return Promise.resolve(this.createChaosResponse(url, requestInit));
    } else {
      if (!this.next) {
        throw new Error("Please set the next middleware to continue the request");
      }
      return this.next.execute(url, requestInit, requestOptions);
    }
  }
}
ChaosHandler.chaosHandlerTriggeredEventKey = "com.microsoft.kiota.chaos_handler_triggered";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/compressionHandler.js
var import_api5 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/compressionHandlerOptions.js
var CompressionHandlerOptionsKey = "CompressionHandlerOptionsKey";

class CompressionHandlerOptions {
  constructor(config) {
    var _a2;
    this._enableCompression = (_a2 = config === null || config === undefined ? undefined : config.enableCompression) !== null && _a2 !== undefined ? _a2 : true;
  }
  getKey() {
    return CompressionHandlerOptionsKey;
  }
  get ShouldCompress() {
    return this._enableCompression;
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/compressionHandler.js
class CompressionHandler {
  constructor(handlerOptions = new CompressionHandlerOptions) {
    this.handlerOptions = handlerOptions;
    if (!handlerOptions) {
      throw new Error("handlerOptions cannot be undefined");
    }
  }
  execute(url, requestInit, requestOptions) {
    let currentOptions = this.handlerOptions;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[CompressionHandlerOptionsKey]) {
      currentOptions = requestOptions[CompressionHandlerOptionsKey];
    }
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api5.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("compressionHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.compression.enable", currentOptions.ShouldCompress);
          return this.executeInternal(currentOptions, url, requestInit, requestOptions, span);
        } finally {
          span.end();
        }
      });
    }
    return this.executeInternal(currentOptions, url, requestInit, requestOptions);
  }
  async executeInternal(options, url, requestInit, requestOptions, span) {
    var _a2, _b, _c, _d;
    if (!options.ShouldCompress || this.contentRangeBytesIsPresent(requestInit.headers) || this.contentEncodingIsPresent(requestInit.headers) || requestInit.body === null || requestInit.body === undefined) {
      return (_b = (_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(url, requestInit, requestOptions)) !== null && _b !== undefined ? _b : Promise.reject(new Error("Response is undefined"));
    }
    span === null || span === undefined || span.setAttribute("http.request.body.compressed", true);
    const unCompressedBody = requestInit.body;
    const unCompressedBodySize = this.getRequestBodySize(unCompressedBody);
    const compressedBody = await this.compressRequestBody(unCompressedBody);
    setRequestHeader(requestInit, CompressionHandler.CONTENT_ENCODING_HEADER, "gzip");
    requestInit.body = compressedBody.compressedBody;
    span === null || span === undefined || span.setAttribute("http.request.body.size", compressedBody.size);
    let response = await ((_c = this.next) === null || _c === undefined ? undefined : _c.execute(url, requestInit, requestOptions));
    if (!response) {
      throw new Error("Response is undefined");
    }
    if (response.status === 415) {
      deleteRequestHeader(requestInit, CompressionHandler.CONTENT_ENCODING_HEADER);
      requestInit.body = unCompressedBody;
      span === null || span === undefined || span.setAttribute("http.request.body.compressed", false);
      span === null || span === undefined || span.setAttribute("http.request.body.size", unCompressedBodySize);
      response = await ((_d = this.next) === null || _d === undefined ? undefined : _d.execute(url, requestInit, requestOptions));
    }
    return response !== undefined && response !== null ? Promise.resolve(response) : Promise.reject(new Error("Response is undefined"));
  }
  contentRangeBytesIsPresent(header) {
    var _a2;
    if (!header) {
      return false;
    }
    const contentRange = getRequestHeader(header, CompressionHandler.CONTENT_RANGE_HEADER);
    return (_a2 = contentRange === null || contentRange === undefined ? undefined : contentRange.toLowerCase().includes("bytes")) !== null && _a2 !== undefined ? _a2 : false;
  }
  contentEncodingIsPresent(header) {
    if (!header) {
      return false;
    }
    return getRequestHeader(header, CompressionHandler.CONTENT_ENCODING_HEADER) !== undefined;
  }
  getRequestBodySize(body) {
    if (!body) {
      return 0;
    }
    if (typeof body === "string") {
      return body.length;
    }
    if (body instanceof Blob) {
      return body.size;
    }
    if (body instanceof ArrayBuffer) {
      return body.byteLength;
    }
    if (ArrayBuffer.isView(body)) {
      return body.byteLength;
    }
    if (inNodeEnv() && Buffer.isBuffer(body)) {
      return body.byteLength;
    }
    throw new Error("Unsupported body type");
  }
  readBodyAsBytes(body) {
    if (!body) {
      return { stream: new ReadableStream, size: 0 };
    }
    const uint8ArrayToStream = (uint8Array) => {
      return new ReadableStream({
        start: (controller) => {
          controller.enqueue(uint8Array);
          controller.close();
        }
      });
    };
    if (typeof body === "string") {
      return { stream: uint8ArrayToStream(new TextEncoder().encode(body)), size: body.length };
    }
    if (body instanceof Blob) {
      return { stream: body.stream(), size: body.size };
    }
    if (body instanceof ArrayBuffer) {
      return { stream: uint8ArrayToStream(new Uint8Array(body)), size: body.byteLength };
    }
    if (ArrayBuffer.isView(body)) {
      return { stream: uint8ArrayToStream(new Uint8Array(body.buffer, body.byteOffset, body.byteLength)), size: body.byteLength };
    }
    throw new Error("Unsupported body type");
  }
  async compressRequestBody(body) {
    const compressionData = this.readBodyAsBytes(body);
    const compressedBody = await this.compressUsingCompressionStream(compressionData.stream);
    return {
      compressedBody: compressedBody.body,
      size: compressedBody.size
    };
  }
  async compressUsingCompressionStream(uint8ArrayStream) {
    const compressionStream = new CompressionStream("gzip");
    const compressedStream = uint8ArrayStream.pipeThrough(compressionStream);
    const reader = compressedStream.getReader();
    const compressedChunks = [];
    let totalLength = 0;
    let result = await reader.read();
    while (!result.done) {
      const chunk = result.value;
      compressedChunks.push(chunk);
      totalLength += chunk.length;
      result = await reader.read();
    }
    const compressedArray = new Uint8Array(totalLength);
    let offset = 0;
    for (const chunk of compressedChunks) {
      compressedArray.set(chunk, offset);
      offset += chunk.length;
    }
    return {
      body: compressedArray.buffer,
      size: compressedArray.length
    };
  }
}
CompressionHandler.CONTENT_RANGE_HEADER = "Content-Range";
CompressionHandler.CONTENT_ENCODING_HEADER = "Content-Encoding";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/headersInspectionHandler.js
var import_api6 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/headersInspectionOptions.js
var HeadersInspectionOptionsKey = "HeadersInspectionOptionsKey";

class HeadersInspectionOptions {
  getRequestHeaders() {
    return this.requestHeaders;
  }
  getResponseHeaders() {
    return this.responseHeaders;
  }
  constructor(options = {}) {
    var _a2, _b;
    this.requestHeaders = new Headers;
    this.responseHeaders = new Headers;
    this.inspectRequestHeaders = (_a2 = options.inspectRequestHeaders) !== null && _a2 !== undefined ? _a2 : false;
    this.inspectResponseHeaders = (_b = options.inspectResponseHeaders) !== null && _b !== undefined ? _b : false;
  }
  getKey() {
    return HeadersInspectionOptionsKey;
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/headersInspectionHandler.js
class HeadersInspectionHandler {
  constructor(_options = new HeadersInspectionOptions) {
    this._options = _options;
  }
  execute(url, requestInit, requestOptions) {
    let currentOptions = this._options;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[HeadersInspectionOptionsKey]) {
      currentOptions = requestOptions[HeadersInspectionOptionsKey];
    }
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api6.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("retryHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.headersInspection.enable", true);
          return this.executeInternal(url, requestInit, requestOptions, currentOptions);
        } finally {
          span.end();
        }
      });
    }
    return this.executeInternal(url, requestInit, requestOptions, currentOptions);
  }
  async executeInternal(url, requestInit, requestOptions, currentOptions) {
    if (!this.next) {
      throw new Error("next middleware is undefined.");
    }
    if (currentOptions.inspectRequestHeaders && requestInit.headers) {
      for (const [key, value] of requestInit.headers) {
        currentOptions.getRequestHeaders().add(key, value);
      }
    }
    const response = await this.next.execute(url, requestInit, requestOptions);
    if (currentOptions.inspectResponseHeaders && response.headers) {
      for (const [key, value] of response.headers.entries()) {
        currentOptions.getResponseHeaders().add(key, value);
      }
    }
    return response;
  }
}
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/parametersNameDecodingHandler.js
var import_api7 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/parametersNameDecodingOptions.js
var ParametersNameDecodingHandlerOptionsKey = "RetryHandlerOptionKey";

class ParametersNameDecodingHandlerOptions {
  getKey() {
    return ParametersNameDecodingHandlerOptionsKey;
  }
  constructor(options = {}) {
    var _a2, _b;
    this.enable = (_a2 = options.enable) !== null && _a2 !== undefined ? _a2 : true;
    this.charactersToDecode = (_b = options.charactersToDecode) !== null && _b !== undefined ? _b : [".", "-", "~", "$"];
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/parametersNameDecodingHandler.js
class ParametersNameDecodingHandler {
  constructor(options = new ParametersNameDecodingHandlerOptions) {
    this.options = options;
    if (!options) {
      throw new Error("The options parameter is required.");
    }
  }
  execute(url, requestInit, requestOptions) {
    let currentOptions = this.options;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[ParametersNameDecodingHandlerOptionsKey]) {
      currentOptions = requestOptions[ParametersNameDecodingHandlerOptionsKey];
    }
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api7.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("parametersNameDecodingHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.parameters_name_decoding.enable", currentOptions.enable);
          return this.decodeParameters(url, requestInit, currentOptions, requestOptions);
        } finally {
          span.end();
        }
      });
    }
    return this.decodeParameters(url, requestInit, currentOptions, requestOptions);
  }
  decodeParameters(url, requestInit, currentOptions, requestOptions) {
    var _a2, _b;
    let updatedUrl = url;
    if (currentOptions && currentOptions.enable && url.includes("%") && currentOptions.charactersToDecode && currentOptions.charactersToDecode.length > 0) {
      currentOptions.charactersToDecode.forEach((character) => {
        updatedUrl = updatedUrl.replace(new RegExp(`%${character.charCodeAt(0).toString(16)}`, "gi"), character);
      });
    }
    return (_b = (_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(updatedUrl, requestInit, requestOptions)) !== null && _b !== undefined ? _b : Promise.reject(new Error("The next middleware is not set."));
  }
}
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/redirectHandler.js
var import_api8 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/redirectHandlerOptions.js
var RedirectHandlerOptionKey = "RedirectHandlerOption";
var defaultScrubSensitiveHeaders = (headers2, originalUrl, newUrl) => {
  if (!headers2 || !originalUrl || !newUrl) {
    return;
  }
  try {
    const originalUri = new URL(originalUrl);
    const newUri = new URL(newUrl);
    const isDifferentHostOrScheme = originalUri.host.toLowerCase() !== newUri.host.toLowerCase() || originalUri.protocol.toLowerCase() !== newUri.protocol.toLowerCase();
    if (isDifferentHostOrScheme) {
      for (const key of Object.keys(headers2)) {
        const lower = key.toLowerCase();
        if (lower === "authorization" || lower === "cookie" || lower === "proxy-authorization") {
          delete headers2[key];
        }
      }
    }
  } catch (_a2) {
    return;
  }
};

class RedirectHandlerOptions {
  constructor(options = {}) {
    var _a2, _b, _c;
    if (options.maxRedirects && options.maxRedirects > RedirectHandlerOptions.MAX_MAX_REDIRECTS) {
      const error = new Error(`MaxRedirects should not be more than ${RedirectHandlerOptions.MAX_MAX_REDIRECTS}`);
      error.name = "MaxLimitExceeded";
      throw error;
    }
    if (options.maxRedirects !== undefined && options.maxRedirects < 0) {
      const error = new Error(`MaxRedirects should not be negative`);
      error.name = "MinExpectationNotMet";
      throw error;
    }
    this.maxRedirects = (_a2 = options.maxRedirects) !== null && _a2 !== undefined ? _a2 : RedirectHandlerOptions.DEFAULT_MAX_REDIRECTS;
    this.shouldRedirect = (_b = options.shouldRedirect) !== null && _b !== undefined ? _b : RedirectHandlerOptions.defaultShouldRetry;
    this.scrubSensitiveHeaders = (_c = options.scrubSensitiveHeaders) !== null && _c !== undefined ? _c : RedirectHandlerOptions.defaultScrubSensitiveHeaders;
  }
  getKey() {
    return RedirectHandlerOptionKey;
  }
}
RedirectHandlerOptions.DEFAULT_MAX_REDIRECTS = 5;
RedirectHandlerOptions.MAX_MAX_REDIRECTS = 20;
RedirectHandlerOptions.defaultShouldRetry = () => true;
RedirectHandlerOptions.defaultScrubSensitiveHeaders = defaultScrubSensitiveHeaders;

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/redirectHandler.js
class RedirectHandler {
  constructor(options = new RedirectHandlerOptions) {
    this.options = options;
    if (!options) {
      throw new Error("The options parameter is required.");
    }
  }
  isRedirect(response) {
    return RedirectHandler.REDIRECT_STATUS_CODES.has(response.status);
  }
  hasLocationHeader(response) {
    return response.headers.has(RedirectHandler.LOCATION_HEADER);
  }
  getLocationHeader(response) {
    return response.headers.get(RedirectHandler.LOCATION_HEADER);
  }
  isRelativeURL(url) {
    return !url.includes("://");
  }
  async executeWithRedirect(url, fetchRequestInit, redirectCount, currentOptions, requestOptions, tracerName) {
    var _a2;
    const response = await ((_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(url, fetchRequestInit, requestOptions));
    if (!response) {
      throw new Error("Response is undefined");
    }
    if (redirectCount < currentOptions.maxRedirects && this.isRedirect(response) && this.hasLocationHeader(response) && currentOptions.shouldRedirect(response)) {
      ++redirectCount;
      const redirectUrl = this.getLocationHeader(response);
      if (!redirectUrl) {
        return response;
      }
      const newUrl = this.isRelativeURL(redirectUrl) ? new URL(redirectUrl, url).toString() : redirectUrl;
      if (fetchRequestInit.headers) {
        currentOptions.scrubSensitiveHeaders(fetchRequestInit.headers, url, newUrl);
      }
      if (response.status === RedirectHandler.STATUS_CODE_SEE_OTHER) {
        fetchRequestInit.method = HttpMethod.GET;
        delete fetchRequestInit.body;
      }
      url = newUrl;
      if (tracerName) {
        return import_api8.trace.getTracer(tracerName).startActiveSpan(`redirectHandler - redirect ${redirectCount}`, (span) => {
          try {
            span.setAttribute("com.microsoft.kiota.handler.redirect.count", redirectCount);
            span.setAttribute("http.response.status_code", response.status);
            return this.executeWithRedirect(url, fetchRequestInit, redirectCount, currentOptions, requestOptions);
          } finally {
            span.end();
          }
        });
      }
      return await this.executeWithRedirect(url, fetchRequestInit, redirectCount, currentOptions, requestOptions);
    } else {
      return response;
    }
  }
  execute(url, requestInit, requestOptions) {
    const redirectCount = 0;
    let currentOptions = this.options;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[RedirectHandlerOptionKey]) {
      currentOptions = requestOptions[RedirectHandlerOptionKey];
    }
    requestInit.redirect = RedirectHandler.MANUAL_REDIRECT;
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api8.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("redirectHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.redirect.enable", true);
          return this.executeWithRedirect(url, requestInit, redirectCount, currentOptions, requestOptions, obsOptions.getTracerInstrumentationName());
        } finally {
          span.end();
        }
      });
    }
    return this.executeWithRedirect(url, requestInit, redirectCount, currentOptions, requestOptions);
  }
}
RedirectHandler.REDIRECT_STATUS_CODES = new Set([
  301,
  302,
  303,
  307,
  308
]);
RedirectHandler.STATUS_CODE_SEE_OTHER = 303;
RedirectHandler.LOCATION_HEADER = "Location";
RedirectHandler.MANUAL_REDIRECT = "manual";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/retryHandler.js
var import_api9 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/retryHandlerOptions.js
var RetryHandlerOptionKey = "RetryHandlerOptionKey";

class RetryHandlerOptions {
  constructor(options = {}) {
    var _a2, _b, _c;
    if (options.delay !== undefined && options.delay > RetryHandlerOptions.MAX_DELAY) {
      throw this.createError(`Delay should not be more than ${RetryHandlerOptions.MAX_DELAY}`, "MaxLimitExceeded");
    }
    if (options.maxRetries !== undefined && options.maxRetries > RetryHandlerOptions.MAX_MAX_RETRIES) {
      throw this.createError(`MaxRetries should not be more than ${RetryHandlerOptions.MAX_MAX_RETRIES}`, "MaxLimitExceeded");
    }
    if (options.delay !== undefined && options.delay < 0) {
      throw this.createError(`Delay should not be negative`, "MinExpectationNotMet");
    }
    if (options.maxRetries !== undefined && options.maxRetries < 0) {
      throw this.createError(`MaxRetries should not be negative`, "MinExpectationNotMet");
    }
    this.delay = Math.min((_a2 = options.delay) !== null && _a2 !== undefined ? _a2 : RetryHandlerOptions.DEFAULT_DELAY, RetryHandlerOptions.MAX_DELAY);
    this.maxRetries = Math.min((_b = options.maxRetries) !== null && _b !== undefined ? _b : RetryHandlerOptions.DEFAULT_MAX_RETRIES, RetryHandlerOptions.MAX_MAX_RETRIES);
    this.shouldRetry = (_c = options.shouldRetry) !== null && _c !== undefined ? _c : RetryHandlerOptions.defaultShouldRetry;
  }
  createError(message, name) {
    const error = new Error(message);
    error.name = name;
    return error;
  }
  getMaxDelay() {
    return RetryHandlerOptions.MAX_DELAY;
  }
  getKey() {
    return RetryHandlerOptionKey;
  }
}
RetryHandlerOptions.DEFAULT_DELAY = 3;
RetryHandlerOptions.DEFAULT_MAX_RETRIES = 3;
RetryHandlerOptions.MAX_DELAY = 180;
RetryHandlerOptions.MAX_MAX_RETRIES = 10;
RetryHandlerOptions.defaultShouldRetry = () => true;

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/retryHandler.js
class RetryHandler {
  constructor(options = new RetryHandlerOptions) {
    this.options = options;
    if (!options) {
      throw new Error("The options parameter is required.");
    }
  }
  isRetry(response) {
    return RetryHandler.RETRY_STATUS_CODES.has(response.status);
  }
  isBuffered(options) {
    var _a2;
    const method = options.method;
    const isPutPatchOrPost = method === HttpMethod.PUT || method === HttpMethod.PATCH || method === HttpMethod.POST;
    if (isPutPatchOrPost) {
      const isStream = ((_a2 = getRequestHeader(options, "content-type")) === null || _a2 === undefined ? undefined : _a2.toLowerCase()) === "application/octet-stream";
      if (isStream) {
        return false;
      }
    }
    return true;
  }
  getDelay(response, retryAttempts, delay) {
    const getRandomness = () => Number(Math.random().toFixed(3));
    const retryAfter = response.headers !== undefined ? response.headers.get(RetryHandler.RETRY_AFTER_HEADER) : null;
    let newDelay;
    if (retryAfter !== null) {
      if (Number.isNaN(Number(retryAfter))) {
        newDelay = Math.round((new Date(retryAfter).getTime() - Date.now()) / 1000);
      } else {
        newDelay = Number(retryAfter);
      }
    } else {
      newDelay = retryAttempts >= 2 ? this.getExponentialBackOffTime(retryAttempts) + delay + getRandomness() : delay + getRandomness();
    }
    return Math.min(newDelay, this.options.getMaxDelay() + getRandomness());
  }
  getExponentialBackOffTime(attempts) {
    return Math.round(1 / 2 * (2 ** attempts - 1));
  }
  async sleep(delaySeconds) {
    const delayMilliseconds = delaySeconds * 1000;
    return new Promise((resolve) => setTimeout(resolve, delayMilliseconds));
  }
  async executeWithRetry(url, fetchRequestInit, retryAttempts, currentOptions, requestOptions, tracerName) {
    var _a2;
    const response = await ((_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(url, fetchRequestInit, requestOptions));
    if (!response) {
      throw new Error("Response is undefined");
    }
    if (retryAttempts < currentOptions.maxRetries && this.isRetry(response) && this.isBuffered(fetchRequestInit) && currentOptions.shouldRetry(currentOptions.delay, retryAttempts, url, fetchRequestInit, response)) {
      ++retryAttempts;
      setRequestHeader(fetchRequestInit, RetryHandler.RETRY_ATTEMPT_HEADER, retryAttempts.toString());
      let delay = null;
      if (response) {
        delay = this.getDelay(response, retryAttempts, currentOptions.delay);
        await this.sleep(delay);
      }
      if (tracerName) {
        return await import_api9.trace.getTracer(tracerName).startActiveSpan(`retryHandler - attempt ${retryAttempts}`, (span) => {
          try {
            span.setAttribute("http.request.resend_count", retryAttempts);
            if (delay) {
              span.setAttribute("http.request.resend_delay", delay);
            }
            span.setAttribute("http.response.status_code", response.status);
            return this.executeWithRetry(url, fetchRequestInit, retryAttempts, currentOptions, requestOptions);
          } finally {
            span.end();
          }
        });
      }
      return await this.executeWithRetry(url, fetchRequestInit, retryAttempts, currentOptions, requestOptions);
    } else {
      return response;
    }
  }
  execute(url, requestInit, requestOptions) {
    const retryAttempts = 0;
    let currentOptions = this.options;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[RetryHandlerOptionKey]) {
      currentOptions = requestOptions[RetryHandlerOptionKey];
    }
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api9.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("retryHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.retry.enable", true);
          return this.executeWithRetry(url, requestInit, retryAttempts, currentOptions, requestOptions, obsOptions.getTracerInstrumentationName());
        } finally {
          span.end();
        }
      });
    }
    return this.executeWithRetry(url, requestInit, retryAttempts, currentOptions, requestOptions);
  }
}
RetryHandler.RETRY_STATUS_CODES = new Set([
  429,
  503,
  504
]);
RetryHandler.RETRY_ATTEMPT_HEADER = "Retry-Attempt";
RetryHandler.RETRY_AFTER_HEADER = "Retry-After";
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/userAgentHandler.js
var import_api10 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/version.js
var libraryVersion = "1.0.0-preview.24";

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/userAgentHandlerOptions.js
var UserAgentHandlerOptionsKey = "UserAgentHandlerOptionKey";

class UserAgentHandlerOptions {
  getKey() {
    return UserAgentHandlerOptionsKey;
  }
  constructor(options = {}) {
    var _a2, _b, _c;
    this.enable = (_a2 = options.enable) !== null && _a2 !== undefined ? _a2 : true;
    this.productName = (_b = options.productName) !== null && _b !== undefined ? _b : "kiota-typescript";
    this.productVersion = (_c = options.productVersion) !== null && _c !== undefined ? _c : libraryVersion;
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/userAgentHandler.js
var USER_AGENT_HEADER_KEY = "User-Agent";

class UserAgentHandler {
  constructor(_options = new UserAgentHandlerOptions) {
    this._options = _options;
  }
  execute(url, requestInit, requestOptions) {
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api10.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("userAgentHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.useragent.enable", true);
          return this.addValue(url, requestInit, requestOptions);
        } finally {
          span.end();
        }
      });
    } else {
      return this.addValue(url, requestInit, requestOptions);
    }
  }
  async addValue(url, requestInit, requestOptions) {
    var _a2;
    let currentOptions = this._options;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[UserAgentHandlerOptionsKey]) {
      currentOptions = requestOptions[UserAgentHandlerOptionsKey];
    }
    if (currentOptions.enable) {
      const additionalValue = `${currentOptions.productName}/${currentOptions.productVersion}`;
      const currentValue = getRequestHeader(requestInit, USER_AGENT_HEADER_KEY);
      if (!(currentValue === null || currentValue === undefined ? undefined : currentValue.includes(additionalValue))) {
        appendRequestHeader(requestInit, USER_AGENT_HEADER_KEY, additionalValue, " ");
      }
    }
    const response = await ((_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(url, requestInit, requestOptions));
    if (!response)
      throw new Error("No response returned by the next middleware");
    return response;
  }
}
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/urlReplaceHandler.js
var import_api11 = __toESM(require_src(), 1);

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/options/urlReplaceHandlerOptions.js
var UrlReplaceHandlerOptionsKey = "UrlReplaceHandlerOptionsKey";

class UrlReplaceHandlerOptions {
  constructor(config) {
    var _a2, _b;
    if (config) {
      this._urlReplacements = (_a2 = config.urlReplacements) !== null && _a2 !== undefined ? _a2 : {};
      this._enabled = (_b = config.enabled) !== null && _b !== undefined ? _b : true;
    } else {
      this._urlReplacements = {};
      this._enabled = true;
    }
  }
  getKey() {
    return UrlReplaceHandlerOptionsKey;
  }
  get enabled() {
    return this._enabled;
  }
  get urlReplacements() {
    return this._urlReplacements;
  }
}

// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/urlReplaceHandler.js
class UrlReplaceHandler {
  constructor(handlerOptions = new UrlReplaceHandlerOptions) {
    this.handlerOptions = handlerOptions;
    if (!handlerOptions) {
      throw new Error("handlerOptions cannot be undefined");
    }
  }
  execute(url, requestInit, requestOptions) {
    let currentOptions = this.handlerOptions;
    if (requestOptions === null || requestOptions === undefined ? undefined : requestOptions[UrlReplaceHandlerOptionsKey]) {
      currentOptions = requestOptions[UrlReplaceHandlerOptionsKey];
    }
    const obsOptions = getObservabilityOptionsFromRequest(requestOptions);
    if (obsOptions) {
      return import_api11.trace.getTracer(obsOptions.getTracerInstrumentationName()).startActiveSpan("urlReplaceHandler - execute", (span) => {
        try {
          span.setAttribute("com.microsoft.kiota.handler.urlReplace.enable", currentOptions.enabled);
          return this.replaceTokensInUrl(currentOptions, url, requestInit, requestOptions);
        } finally {
          span.end();
        }
      });
    }
    return this.replaceTokensInUrl(currentOptions, url, requestInit, requestOptions);
  }
  replaceTokensInUrl(options, url, requestInit, requestOptions) {
    var _a2;
    if (options.enabled) {
      Object.keys(options.urlReplacements).forEach((replacementKey) => {
        url = url.replace(replacementKey, options.urlReplacements[replacementKey]);
      });
    }
    const response = (_a2 = this.next) === null || _a2 === undefined ? undefined : _a2.execute(url, requestInit, requestOptions);
    if (!response) {
      throw new Error("Response is undefined");
    }
    return response;
  }
}
// node_modules/@microsoft/kiota-http-fetchlibrary/dist/es/src/middlewares/middlewareFactory.js
class MiddlewareFactory {
  static getDefaultMiddlewares(customFetch = (...args) => fetch(...args)) {
    return [new RetryHandler, new RedirectHandler, new ParametersNameDecodingHandler, new UserAgentHandler, new HeadersInspectionHandler, new UrlReplaceHandler, new CustomFetchHandler(customFetch)];
  }
  static getPerformanceMiddlewares(customFetch = (...args) => fetch(...args)) {
    const middlewares = MiddlewareFactory.getDefaultMiddlewares(customFetch);
    middlewares.splice(middlewares.length - 3, 0, new CompressionHandler);
    return middlewares;
  }
}
// .svelte-kit/output/server/chunks/doClient.js
var cached = null;
function loadDoSdk() {
  if (!cached)
    cached = import(["@digitalocean", "dots"].join("/"));
  return cached;
}
async function makeClient(tokenOverride) {
  const sdk = await loadDoSdk();
  const token = tokenOverride ?? getSetting("do_token") ?? "";
  const authProvider = new sdk.DigitalOceanApiKeyAuthenticationProvider(token);
  const adapter = new FetchRequestAdapter(authProvider);
  return sdk.createDigitalOceanClient(adapter);
}
function describeDoError(e) {
  const code = e?.responseStatusCode;
  if (code === 401 || code === 403)
    return {
      reason: "rejected",
      message: `DigitalOcean rejected this Personal Access Token (wrong or revoked, status ${code}). It is not a network error \u2014 do not generate a new one unless you are sure this one is dead.`
    };
  if (code === 429)
    return {
      reason: "rate",
      message: "DigitalOcean is rate-limiting this token. Wait a moment and try again."
    };
  if (typeof code === "number" && code >= 500)
    return {
      reason: "network",
      message: `DigitalOcean had a server error (status ${code}). Do not generate a new Personal Access Token \u2014 this is not your token's fault.`
    };
  return {
    reason: "network",
    message: "Could not reach DigitalOcean. Check your connection \u2014 do not generate a new Personal Access Token yet."
  };
}
async function listAllDroplets(tokenOverride) {
  return { droplets: ((await doFetch("/droplets?per_page=200", {}, tokenOverride))?.droplets ?? []).map(toPlainDropletRest) };
}
async function listDropletsByTag(tag, tokenOverride) {
  return { droplets: ((await doFetch(`/droplets?tag_name=${encodeURIComponent(tag)}&per_page=200`, {}, tokenOverride))?.droplets ?? []).map(toPlainDropletRest) };
}
function toPlainDropletRest(d) {
  return {
    id: d?.id ?? 0,
    name: d?.name ?? "",
    status: d?.status ?? "off",
    tags: d?.tags ?? [],
    networks: { v4: (d?.networks?.v4 ?? []).map((n) => ({
      ipAddress: n?.ip_address ?? "",
      type: n?.type ?? ""
    })) },
    sizeSlug: d?.size_slug ?? "",
    region: { slug: d?.region?.slug ?? "" },
    createdAt: d?.created_at ?? null
  };
}
async function createDroplet(body, tokenOverride) {
  const d = (await doFetch("/droplets", {
    method: "POST",
    body: JSON.stringify({
      name: body.name,
      region: body.region,
      size: body.size,
      image: body.image,
      ssh_keys: body.ssh_keys,
      tags: body.tags,
      user_data: body.user_data,
      monitoring: body.monitoring ?? false,
      ipv6: false,
      backups: false,
      ...body.dedicatedCpu ? { dedicated_cpu: true } : {},
      ...body.volumes?.length ? { volumes: body.volumes } : {}
    })
  }, tokenOverride))?.droplet;
  if (!d)
    throw new Error("DO returned no droplet in create response");
  return { droplet: toPlainDropletRest(d) };
}
async function deleteDroplet(id) {
  await (await makeClient()).v2.droplets.byDroplet_id(id).delete();
  return null;
}
async function getAccount(tokenOverride) {
  return (await makeClient(tokenOverride)).v2.account.get();
}
async function listSshKeys(tokenOverride) {
  return ((await (await makeClient(tokenOverride)).v2.account.keys.get())?.sshKeys ?? []).map((k) => ({
    id: k?.id ?? 0,
    name: k?.name ?? "",
    fingerprint: k?.fingerprint ?? ""
  }));
}
async function listDropletSnapshots(tokenOverride) {
  return ((await (await makeClient(tokenOverride)).v2.snapshots.get({
    resourceType: "droplet",
    perPage: 200
  }))?.snapshots ?? []).map((s) => ({
    id: String(s?.id ?? ""),
    name: s?.name ?? ""
  }));
}
async function listRegions(tokenOverride) {
  return ((await doFetch("/regions?per_page=200", {}, tokenOverride))?.regions ?? []).filter((r) => r?.available).map((r) => ({
    slug: r?.slug ?? "",
    name: r?.name ?? r?.slug ?? ""
  }));
}
async function listSizes(tokenOverride) {
  return ((await doFetch("/sizes?per_page=200", {}, tokenOverride))?.sizes ?? []).map((s) => ({
    slug: s?.slug ?? "",
    vcpus: s?.vcpus ?? 0,
    memoryGb: Math.round((s?.memory ?? 0) / 1024),
    diskGb: s?.disk ?? 0,
    hourly: s?.price_hourly ?? 0,
    monthly: s?.price_monthly ?? 0,
    regions: s?.regions ?? [],
    available: s?.available ?? false,
    description: s?.description ?? ""
  }));
}
function doFetch(path, init = {}, tokenOverride) {
  const token = tokenOverride ?? getSetting("do_token") ?? "";
  return fetch(`https://api.digitalocean.com/v2${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      ...init?.headers ?? {}
    }
  }).then(async (r) => {
    const data = await r.json().catch(() => ({}));
    if (!r.ok)
      throw Object.assign(new Error(data?.message ?? `DO request failed (${r.status})`), { responseStatusCode: r.status });
    return data;
  });
}
async function listDistributions(tokenOverride) {
  return ((await doFetch("/images?type=distribution&per_page=200", {}, tokenOverride))?.images ?? []).filter((i) => typeof i?.slug === "string" && i.slug.endsWith("-x64") && !i.slug.startsWith("gpu-")).map((i) => ({
    id: String(i?.id ?? ""),
    slug: i?.slug ?? "",
    name: i?.name ?? "",
    distribution: i?.distribution ?? ""
  }));
}
async function createSshKey(name, publicKey, tokenOverride) {
  const data = await doFetch("/account/keys", {
    method: "POST",
    body: JSON.stringify({
      name,
      public_key: publicKey
    })
  }, tokenOverride);
  return {
    id: data?.ssh_key?.id ?? 0,
    name: data?.ssh_key?.name ?? "",
    fingerprint: data?.ssh_key?.fingerprint ?? ""
  };
}
async function createDropletSnapshot(dropletId, name) {
  return { actionId: (await doFetch(`/droplets/${dropletId}/actions`, {
    method: "POST",
    body: JSON.stringify({
      type: "snapshot",
      name
    })
  }))?.action?.id ?? 0 };
}
async function listVolumesByName(name) {
  return ((await doFetch(`/volumes?name=${encodeURIComponent(name)}`))?.volumes ?? []).map((v) => ({
    id: v?.id ?? 0,
    region: v?.region?.slug ?? "",
    dropletIds: v?.droplet_ids ?? []
  }));
}
async function detachVolume(id, dropletId) {
  await doFetch(`/volumes/${id}?droplet_id=${dropletId}`, { method: "DELETE" });
  return null;
}
async function createVolume(args) {
  return { id: (await doFetch("/volumes", {
    method: "POST",
    body: JSON.stringify({
      name: args.name,
      region: args.region,
      size_gigabytes: args.sizeGb,
      filesystem_type: "ext4"
    })
  }))?.volume?.id ?? 0 };
}
async function deleteVolume(id) {
  await doFetch(`/volumes/${id}`, { method: "DELETE" });
  return null;
}
var lastSweep = 0;
async function sweepOrphanVolumes() {
  if (Date.now() - lastSweep < 300000)
    return 0;
  lastSweep = Date.now();
  let deleted = 0;
  try {
    const data = await doFetch("/volumes?per_page=200");
    for (const v of data?.volumes ?? []) {
      const match = /^sb-machine-(\d+)-data$/.exec(v?.name ?? "");
      if (!match)
        continue;
      const machineId = Number(match[1]);
      if (getMachine(machineId))
        continue;
      const attached = (v.droplet_ids ?? [])[0];
      const url = attached ? `/volumes/${v.id}?droplet_id=${attached}` : `/volumes/${v.id}`;
      try {
        await doFetch(url, { method: "DELETE" });
        deleted++;
        addEvent(machineId, "destroy", `Orphan volume ${v.name} deleted (profile gone)`);
      } catch {}
    }
  } catch {}
  return deleted;
}
function statusFromDroplet(d) {
  if (!d || d.id === 0)
    return {
      running: false,
      state: "off",
      ip: null,
      droplet_id: null
    };
  const ip = d.networks.v4.find((n) => n.type === "public")?.ipAddress ?? null;
  return {
    running: d.status === "new" || d.status === "active",
    state: d.status === "new" ? "starting" : d.status,
    ip,
    droplet_id: d.id
  };
}
function sshKeyIds() {
  return (getSetting("do_ssh_key_ids") ?? "").split(",").filter(Boolean).map(Number);
}

export { describeDoError, listAllDroplets, listDropletsByTag, createDroplet, deleteDroplet, getAccount, listSshKeys, listDropletSnapshots, listRegions, listSizes, listDistributions, createSshKey, createDropletSnapshot, listVolumesByName, detachVolume, createVolume, deleteVolume, sweepOrphanVolumes, statusFromDroplet, sshKeyIds };

//# debugId=5F082E9DB3EB387A64756E2164756E21
