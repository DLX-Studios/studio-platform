// @bun
import {
  __export
} from "./index-qcep1gwj.js";

// node_modules/@bradenmacdonald/s3-lite-client/errors.js
var exports_errors = {};
__export(exports_errors, {
  AccessKeyRequiredError: () => AccessKeyRequiredError,
  InvalidArgumentError: () => InvalidArgumentError,
  InvalidBucketNameError: () => InvalidBucketNameError,
  InvalidEndpointError: () => InvalidEndpointError,
  InvalidExpiryError: () => InvalidExpiryError,
  InvalidObjectNameError: () => InvalidObjectNameError,
  S3Error: () => S3Error,
  SecretKeyRequiredError: () => SecretKeyRequiredError,
  ServerError: () => ServerError,
  parseServerError: () => parseServerError
});

// node_modules/@bradenmacdonald/s3-lite-client/xml-parser.js
function childText(node, name) {
  return node.children.find((c) => c.name === name)?.content;
}
function parse(xml) {
  xml = xml.trim();
  xml = xml.replace(/<!--[\s\S]*?-->/g, "");
  match(/^<\?xml[\s\S]*?\?>\s*/);
  return tag();
  function tag() {
    const m = match(/^<([\w-:.]+)\s*/);
    if (!m)
      return;
    const node = {
      name: m[1],
      attributes: {},
      children: []
    };
    while (!(eos() || is(">") || is("?>") || is("/>"))) {
      const attr = attribute();
      if (!attr)
        return node;
      node.attributes[attr.name] = attr.value;
    }
    if (match(/^\s*\/>\s*/)) {
      return node;
    }
    match(/^\??>\s*/);
    node.content = content();
    let child;
    while (child = tag()) {
      node.children.push(child);
    }
    match(/^<\/[\w-:.]+>\s*/);
    return node;
  }
  function content() {
    const m = match(/^([^<]*)/);
    if (m)
      return entities(m[1]);
    return "";
  }
  function attribute() {
    const m = match(/^([\w:-]+)\s*=\s*("[^"]*"|'[^']*'|\w+)\s*/);
    if (!m)
      return;
    return {
      name: m[1],
      value: entities(strip(m[2]))
    };
  }
  function strip(val) {
    return val.replace(/^['"]|['"]$/g, "");
  }
  function entities(val) {
    return val.replaceAll("&lt;", "<").replaceAll("&gt;", ">").replaceAll("&amp;", "&");
  }
  function match(re) {
    const m = xml.match(re);
    if (!m)
      return;
    xml = xml.slice(m[0].length);
    return m;
  }
  function eos() {
    return xml.length === 0;
  }
  function is(prefix) {
    return xml.startsWith(prefix);
  }
}

// node_modules/@bradenmacdonald/s3-lite-client/errors.js
class S3Error extends Error {
  constructor(message, options) {
    super(message, options);
    this.name = new.target.name.endsWith("Error") ? new.target.name : "S3Error";
  }
}

class InvalidArgumentError extends S3Error {
}

class InvalidEndpointError extends S3Error {
}

class InvalidBucketNameError extends S3Error {
  bucketName;
  constructor(bucketName) {
    super(`Invalid bucket name: ${bucketName}`), this.bucketName = bucketName;
  }
}

class InvalidObjectNameError extends S3Error {
  objectName;
  constructor(objectName) {
    super(`Invalid object name: ${objectName}`), this.objectName = objectName;
  }
}

class AccessKeyRequiredError extends S3Error {
  constructor() {
    super("accessKey is required");
  }
}

class SecretKeyRequiredError extends S3Error {
  constructor() {
    super("secretKey is required");
  }
}

class InvalidExpiryError extends S3Error {
  constructor() {
    super("expirySeconds cannot be less than 1 second or more than 7 days");
  }
}

class ServerError extends S3Error {
  statusCode;
  code;
  key;
  bucketName;
  resource;
  region;
  constructor(statusCode, code, message, otherData = {}) {
    super(message), this.statusCode = statusCode, this.code = code;
    this.key = otherData.key;
    this.bucketName = otherData.bucketName;
    this.resource = otherData.resource;
    this.region = otherData.region;
  }
}
async function parseServerError(response) {
  try {
    const errorRoot = parse(await response.text());
    if (errorRoot?.name !== "Error") {
      throw new Error("Invalid root, expected <Error>");
    }
    return new ServerError(response.status, childText(errorRoot, "Code") ?? "UnknownErrorCode", childText(errorRoot, "Message") ?? "The error message could not be determined.", {
      key: childText(errorRoot, "Key"),
      bucketName: childText(errorRoot, "BucketName"),
      resource: childText(errorRoot, "Resource"),
      region: childText(errorRoot, "Region")
    });
  } catch {
    return new ServerError(response.status, "UnrecognizedError", `Error: Unexpected response code ${response.status} ${response.statusText}. Unable to parse response as XML.`);
  }
}

// node_modules/@bradenmacdonald/s3-lite-client/transform-chunk-sizes.js
class TransformChunkSizes extends TransformStream {
  outChunkSize;
  constructor(outChunkSize) {
    let buffer = new Uint8Array(0);
    let offset = 0;
    super({
      transform(chunk, controller) {
        let pos = 0;
        while (pos < chunk.length) {
          const needed = outChunkSize - offset;
          const toCopy = Math.min(needed, chunk.length - pos);
          if (buffer.length < offset + toCopy) {
            const bigger = new Uint8Array(Math.min(outChunkSize, Math.max(offset + toCopy, buffer.length * 2, 64 * 1024)));
            bigger.set(buffer.subarray(0, offset));
            buffer = bigger;
          }
          buffer.set(chunk.subarray(pos, pos + toCopy), offset);
          pos += toCopy;
          offset += toCopy;
          if (offset === outChunkSize) {
            controller.enqueue(buffer);
            buffer = new Uint8Array(0);
            offset = 0;
          }
        }
      },
      flush(controller) {
        if (offset > 0) {
          controller.enqueue(buffer.subarray(0, offset));
        }
      }
    }), this.outChunkSize = outChunkSize;
  }
}

// node_modules/@bradenmacdonald/s3-lite-client/helpers.js
var encoder = new TextEncoder;
function isValidPort(port) {
  if (typeof port !== "number" || isNaN(port)) {
    return false;
  }
  return port >= 1 && port <= 65535;
}
function isValidBucketName(bucket) {
  if (typeof bucket !== "string") {
    return false;
  }
  if (bucket.length > 255) {
    return false;
  }
  if (bucket.includes("..")) {
    return false;
  }
  return Boolean(bucket.match(/^[a-zA-Z0-9][a-zA-Z0-9.-]+[a-zA-Z0-9]$/));
}
var isValidObjectName = (n) => isValidPrefix(n) && n.length > 0;
function isValidPrefix(prefix) {
  if (typeof prefix !== "string")
    return false;
  if (prefix.length > 1024)
    return false;
  if (!prefix.isWellFormed())
    return false;
  return true;
}
function bin2hex(binary) {
  return Array.from(binary).map((b) => b.toString(16).padStart(2, "0")).join("");
}
function sanitizeETag(etag = "") {
  return etag.replace(/^(?:"|&quot;|&#34;)|(?:"|&quot;|&#34;)$/g, "");
}
function getVersionId(headers) {
  return headers.get("x-amz-version-id") ?? null;
}
function makeDateLong(date) {
  return date.toISOString().replace(/[-:]/g, "").slice(0, 15) + "Z";
}
function makeDateShort(date) {
  return makeDateLong(date).slice(0, 8);
}
function getScope(region, date) {
  return `${makeDateShort(date)}/${region}/s3/aws4_request`;
}
async function sha256digestHex(data) {
  if (!(data instanceof Uint8Array)) {
    data = encoder.encode(data);
  }
  return bin2hex(new Uint8Array(await crypto.subtle.digest("SHA-256", data)));
}

// node_modules/@bradenmacdonald/s3-lite-client/object-uploader.js
var multipartTagAlongMetadataKeys = [
  "x-amz-server-side-encryption-customer-algorithm",
  "x-amz-server-side-encryption-customer-key",
  "x-amz-server-side-encryption-customer-key-MD5"
];
var maxConcurrentParts = 4;
var maxParts = 1e4;
async function uploadSingleRequest({ client, metadata, ...requestArgs }) {
  const response = await client.makeRequest({
    method: "PUT",
    headers: new Headers(metadata),
    ...requestArgs
  });
  return {
    etag: sanitizeETag(response.headers.get("etag") ?? undefined),
    versionId: getVersionId(response.headers)
  };
}

class ObjectUploader extends WritableStream {
  getResult;
  constructor({ client, bucketName, objectName, partSize, metadata }) {
    let result;
    let nextPartNumber = 1;
    let uploadId;
    const etags = [];
    let multiUploadError;
    const partsInFlight = new Set;
    super({
      start() {},
      async write(chunk, _controller) {
        const method = "PUT";
        const partNumber = nextPartNumber++;
        try {
          if (partNumber == 1 && chunk.length < partSize) {
            result = await uploadSingleRequest({
              client,
              bucketName,
              objectName,
              metadata,
              payload: chunk
            });
            return;
          }
          if (partNumber > maxParts) {
            throw new Error(`Cannot upload more than ${maxParts} parts. If you are uploading a stream of unknown size, ` + `specify its "size" or use a larger "partSize" (currently ${partSize} bytes).`);
          }
          if (partNumber === 1) {
            uploadId = (await initiateNewMultipartUpload({
              client,
              bucketName,
              objectName,
              metadata
            })).uploadId;
          }
          const partHeaders = {
            "Content-Length": String(chunk.length)
          };
          for (const key of multipartTagAlongMetadataKeys) {
            const value = metadata[key];
            if (value) {
              partHeaders[key] = value;
            }
          }
          const partPromise = client.makeRequest({
            method,
            query: {
              partNumber: partNumber.toString(),
              uploadId
            },
            headers: new Headers(partHeaders),
            bucketName,
            objectName,
            payload: chunk
          }).then((response) => {
            etags.push({
              part: partNumber,
              etag: sanitizeETag(response.headers.get("etag") ?? undefined)
            });
          }).catch((err) => {
            if (!multiUploadError) {
              multiUploadError = err;
            }
          }).finally(() => {
            partsInFlight.delete(partPromise);
          });
          partsInFlight.add(partPromise);
          if (partsInFlight.size >= maxConcurrentParts) {
            await Promise.race(partsInFlight);
          }
        } catch (err) {
          throw err;
        }
      },
      async close() {
        if (result) {} else if (uploadId) {
          await Promise.all(partsInFlight);
          if (multiUploadError) {
            throw multiUploadError;
          }
          etags.sort((a, b) => a.part > b.part ? 1 : -1);
          result = await completeMultipartUpload({
            client,
            bucketName,
            objectName,
            uploadId,
            etags
          });
        } else {
          result = await uploadSingleRequest({
            client,
            bucketName,
            objectName,
            metadata,
            payload: new Uint8Array
          });
        }
      }
    });
    this.getResult = () => {
      if (result === undefined) {
        throw new Error("Result is not ready. await the stream first.");
      }
      return result;
    };
  }
}
async function initiateNewMultipartUpload(options) {
  const method = "POST";
  const headers = new Headers(options.metadata);
  const query = "uploads";
  const response = await options.client.makeRequest({
    method,
    bucketName: options.bucketName,
    objectName: options.objectName,
    query,
    headers,
    returnBody: true
  });
  const responseText = await response.text();
  const root = parse(responseText);
  if (root?.name !== "InitiateMultipartUploadResult") {
    throw new Error(`Unexpected response: ${responseText}`);
  }
  const uploadId = childText(root, "UploadId");
  if (!uploadId) {
    throw new Error(`Unable to get UploadId from response: ${responseText}`);
  }
  return {
    uploadId
  };
}
async function completeMultipartUpload({ client, bucketName, objectName, uploadId, etags }) {
  const payload = `
    <CompleteMultipartUpload xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
        ${etags.map((et) => `  <Part><PartNumber>${et.part}</PartNumber><ETag>${et.etag}</ETag></Part>`).join(`
`)}
    </CompleteMultipartUpload>
  `;
  const response = await client.makeRequest({
    method: "POST",
    bucketName,
    objectName,
    query: `uploadId=${encodeURIComponent(uploadId)}`,
    payload: encoder.encode(payload),
    returnBody: true
  });
  const responseText = await response.text();
  const root = parse(responseText);
  if (root?.name !== "CompleteMultipartUploadResult") {
    throw new Error(`Unexpected response: ${responseText}`);
  }
  const etagRaw = childText(root, "ETag");
  if (!etagRaw)
    throw new Error(`Unable to get ETag from response: ${responseText}`);
  const versionId = getVersionId(response.headers);
  return {
    etag: sanitizeETag(etagRaw),
    versionId
  };
}

// node_modules/@bradenmacdonald/s3-lite-client/signing.js
var signV4Algorithm = "AWS4-HMAC-SHA256";
async function signV4(request) {
  if (!request.accessKey) {
    throw new AccessKeyRequiredError;
  }
  if (!request.secretKey) {
    throw new SecretKeyRequiredError;
  }
  const sha256sum = request.headers.get("x-amz-content-sha256");
  if (sha256sum === null) {
    throw new Error("Internal S3 client error - expected x-amz-content-sha256 header, but it's missing.");
  }
  const signedHeaders = getHeadersToSign(request.headers);
  const canonicalRequest = getCanonicalRequest(request.method, request.path, request.headers, signedHeaders, sha256sum);
  const stringToSign = await getStringToSign(canonicalRequest, request.date, request.region);
  const signingKey = await getSigningKey(request.date, request.region, request.secretKey);
  const credential = getCredential(request.accessKey, request.region, request.date);
  const signature = bin2hex(await sha256hmac(signingKey, stringToSign)).toLowerCase();
  return `${signV4Algorithm} Credential=${credential}, SignedHeaders=${signedHeaders.join(";").toLowerCase()}, Signature=${signature}`;
}
async function presignV4(request) {
  if (!request.accessKey) {
    throw new AccessKeyRequiredError;
  }
  if (!request.secretKey) {
    throw new SecretKeyRequiredError;
  }
  if (request.expirySeconds < 1 || request.expirySeconds > 604800) {
    throw new InvalidExpiryError;
  }
  if (!request.headers.has("Host")) {
    throw new Error("Internal error: host header missing");
  }
  const resource = request.path.split("?")[0];
  const queryString = request.path.split("?")[1];
  const iso8601Date = makeDateLong(request.date);
  const signedHeaders = getHeadersToSign(request.headers);
  const credential = getCredential(request.accessKey, request.region, request.date);
  const hashedPayload = "UNSIGNED-PAYLOAD";
  const newQuery = new URLSearchParams(queryString);
  newQuery.set("X-Amz-Algorithm", signV4Algorithm);
  newQuery.set("X-Amz-Credential", credential);
  newQuery.set("X-Amz-Date", iso8601Date);
  newQuery.set("X-Amz-Expires", request.expirySeconds.toString());
  newQuery.set("X-Amz-SignedHeaders", signedHeaders.join(";").toLowerCase());
  if (request.sessionToken) {
    newQuery.set("X-Amz-Security-Token", request.sessionToken);
  }
  const newQueryString = newQuery.toString().replaceAll("+", "%20");
  const signingPath = resource + "?" + newQueryString;
  const encodedPath = resource.split("/").map((part) => encodeURIComponent(part)).join("/");
  const canonicalRequest = getCanonicalRequest(request.method, signingPath, request.headers, signedHeaders, hashedPayload);
  const stringToSign = await getStringToSign(canonicalRequest, request.date, request.region);
  const signingKey = await getSigningKey(request.date, request.region, request.secretKey);
  const signature = bin2hex(await sha256hmac(signingKey, stringToSign)).toLowerCase();
  const presignedUrl = `${request.protocol}//${request.headers.get("Host")}${encodedPath}?${newQueryString}&X-Amz-Signature=${signature}`;
  return presignedUrl;
}
function getHeadersToSign(headers) {
  const ignoredHeaders = [
    "authorization",
    "content-length",
    "content-type",
    "user-agent"
  ];
  return [
    ...headers.keys()
  ].filter((key) => !ignoredHeaders.includes(key));
}
function awsUriEncode(string, allowSlashes = false) {
  const encoded = encodeURIComponent(string).replace(/[!'()*]/g, (c) => "%" + c.charCodeAt(0).toString(16).toUpperCase());
  return allowSlashes ? encoded.replaceAll("%2F", "/") : encoded;
}
function getCanonicalRequest(method, path, headers, headersToSign, payloadHash) {
  const headersArray = headersToSign.map((headerKey) => {
    const val = `${headers.get(headerKey)}`.replace(/ +/g, " ");
    return `${headerKey.toLowerCase()}:${val}`;
  });
  const requestResource = path.split("?")[0];
  let requestQuery = path.split("?")[1];
  if (requestQuery) {
    requestQuery = requestQuery.split("&").map((element) => {
      const [key, val] = element.split("=", 2);
      return awsUriEncode(decodeURIComponent(key)) + "=" + awsUriEncode(decodeURIComponent(val || ""));
    }).sort().join("&");
  } else {
    requestQuery = "";
  }
  return [
    method.toUpperCase(),
    awsUriEncode(requestResource, true),
    requestQuery,
    headersArray.join(`
`) + `
`,
    headersToSign.join(";").toLowerCase(),
    payloadHash
  ].join(`
`);
}
async function getStringToSign(canonicalRequest, requestDate, region) {
  const hash = await sha256digestHex(canonicalRequest);
  const scope = getScope(region, requestDate);
  return [
    signV4Algorithm,
    makeDateLong(requestDate),
    scope,
    hash
  ].join(`
`);
}
async function getSigningKey(date, region, secretKey) {
  const dateLine = makeDateShort(date);
  const hmac1 = await sha256hmac("AWS4" + secretKey, dateLine);
  const hmac2 = await sha256hmac(hmac1, region);
  const hmac3 = await sha256hmac(hmac2, "s3");
  return await sha256hmac(hmac3, "aws4_request");
}
function getCredential(accessKey, region, requestDate) {
  return `${accessKey}/${getScope(region, requestDate)}`;
}
async function sha256hmac(secretKey, data) {
  const keyObject = await crypto.subtle.importKey("raw", secretKey instanceof Uint8Array ? secretKey : encoder.encode(secretKey), {
    name: "HMAC",
    hash: {
      name: "SHA-256"
    }
  }, false, [
    "sign"
  ]);
  const signature = await crypto.subtle.sign("HMAC", keyObject, data instanceof Uint8Array ? data : encoder.encode(data));
  return new Uint8Array(signature);
}
async function presignPostV4(request) {
  if (!request.accessKey) {
    throw new AccessKeyRequiredError;
  }
  if (!request.secretKey) {
    throw new SecretKeyRequiredError;
  }
  if (request.expirySeconds < 1 || request.expirySeconds > 604800) {
    throw new InvalidExpiryError;
  }
  const expiration = new Date(request.date);
  expiration.setSeconds(expiration.getSeconds() + request.expirySeconds);
  const credential = getCredential(request.accessKey, request.region, request.date);
  const iso8601Date = makeDateLong(request.date);
  const fields = {
    key: request.objectKey,
    "X-Amz-Algorithm": signV4Algorithm,
    "X-Amz-Credential": credential,
    "X-Amz-Date": iso8601Date
  };
  for (const [name, value] of Object.entries(request.fields ?? {})) {
    if (name in fields) {
      throw new InvalidArgumentError(`The "${name}" field cannot be passed in fields`);
    }
    fields[name] = value;
  }
  const conditions = [
    {
      bucket: request.bucket
    },
    ...Object.entries(fields).map(([name, value]) => ({
      [name]: value
    })),
    ...request.conditions ?? []
  ];
  const policy = {
    expiration: expiration.toISOString(),
    conditions
  };
  const policyBytes = encoder.encode(JSON.stringify(policy));
  const base64Policy = btoa(String.fromCharCode(...policyBytes));
  const signingKey = await getSigningKey(request.date, request.region, request.secretKey);
  fields["policy"] = base64Policy;
  fields["X-Amz-Signature"] = bin2hex(await sha256hmac(signingKey, base64Policy)).toLowerCase();
  const url = `${request.protocol}//${request.host}/${request.bucket}`;
  return {
    url,
    fields
  };
}

// node_modules/@bradenmacdonald/s3-lite-client/client.js
var metadataKeys = [
  "Content-Type",
  "Cache-Control",
  "Content-Disposition",
  "Content-Encoding",
  "Content-Language",
  "Expires",
  "x-amz-checksum-sha256",
  "x-amz-grant-full-control",
  "x-amz-grant-read",
  "x-amz-grant-read-acp",
  "x-amz-grant-write-acp",
  "x-amz-server-side-encryption",
  "x-amz-storage-class",
  "x-amz-website-redirect-location",
  "x-amz-server-side-encryption-customer-algorithm",
  "x-amz-server-side-encryption-customer-key",
  "x-amz-server-side-encryption-customer-key-MD5",
  "x-amz-server-side-encryption-aws-kms-key-id",
  "x-amz-server-side-encryption-context",
  "x-amz-server-side-encryption-bucket-key-enabled",
  "x-amz-request-payer",
  "x-amz-tagging",
  "x-amz-object-lock-mode",
  "x-amz-object-lock-retain-until-date",
  "x-amz-object-lock-legal-hold",
  "x-amz-expected-bucket-owner"
];
var minimumPartSize = 5 * 1024 * 1024;
var maximumPartSize = 5 * 1024 * 1024 * 1024;
var maxObjectSize = 5 * 1024 * 1024 * 1024 * 1024;
var defaultPartSize = 64 * 1024 * 1024;

class Client {
  host;
  port;
  protocol;
  accessKey;
  #secretKey;
  sessionToken;
  defaultBucket;
  region;
  pathStyle;
  pathPrefix;
  constructor({ endPoint, useSSL, port, pathPrefix, ...params }) {
    if (/^https?:\/\//i.test(endPoint)) {
      if (useSSL !== undefined || port !== undefined || pathPrefix !== undefined) {
        throw new InvalidArgumentError(`useSSL/port/pathPrefix cannot be specified if endPoint is a URL.`);
      }
      try {
        const url = new URL(endPoint);
        endPoint = url.hostname;
        useSSL = url.protocol === "https:";
        port = url.port ? parseInt(url.port, 10) : useSSL ? 443 : 80;
        if (url.pathname && url.pathname !== "/") {
          pathPrefix = url.pathname.endsWith("/") ? url.pathname.slice(0, -1) : url.pathname;
        }
      } catch {
        throw new InvalidEndpointError(`Invalid endPoint URL: ${endPoint}`);
      }
    }
    if (typeof endPoint !== "string" || endPoint.length === 0) {
      throw new InvalidEndpointError(`Invalid endPoint: ${endPoint}`);
    }
    if (useSSL === undefined) {
      useSSL = true;
    }
    if (port !== undefined && !isValidPort(port)) {
      throw new InvalidArgumentError(`Invalid port: ${port}`);
    }
    if (params.accessKey && !params.secretKey) {
      throw new InvalidArgumentError(`If specifying access key, secret key must also be provided.`);
    }
    if (params.accessKey && params.accessKey.startsWith("ASIA") && !params.sessionToken) {
      throw new InvalidArgumentError(`If specifying temporary access key, session token must also be provided.`);
    }
    const defaultPort = useSSL ? 443 : 80;
    this.port = port ?? defaultPort;
    this.host = endPoint.toLowerCase() + (this.port !== defaultPort ? `:${this.port}` : "");
    this.protocol = useSSL ? "https:" : "http:";
    this.accessKey = params.accessKey;
    this.#secretKey = params.secretKey ?? "";
    this.sessionToken = params.sessionToken;
    this.pathStyle = params.pathStyle ?? true;
    this.pathPrefix = pathPrefix ?? "";
    this.defaultBucket = params.bucket;
    this.region = params.region;
    if (this.pathPrefix) {
      if (!this.pathStyle) {
        throw new InvalidArgumentError(`pathPrefix is incompatible with pathStyle=false`);
      }
      if (!this.pathPrefix.startsWith("/")) {
        throw new InvalidArgumentError(`pathPrefix should start with /`);
      }
      if (this.pathPrefix.endsWith("/")) {
        throw new InvalidArgumentError(`pathPrefix should not end with /`);
      }
    }
  }
  getBucketName(options) {
    const bucketName = options?.bucketName ?? this.defaultBucket;
    if (bucketName === undefined || !isValidBucketName(bucketName)) {
      throw new InvalidBucketNameError(bucketName ?? "");
    }
    return bucketName;
  }
  checkNames(objectName, options) {
    const bucketName = this.getBucketName(options);
    if (!isValidObjectName(objectName)) {
      throw new InvalidObjectNameError(objectName);
    }
    return bucketName;
  }
  buildRequestOptions(options) {
    const bucketName = this.getBucketName(options);
    const host = this.pathStyle ? this.host : `${bucketName}.${this.host}`;
    const headers = options.headers ?? new Headers;
    headers.set("host", host);
    const queryAsString = typeof options.query === "object" ? new URLSearchParams(options.query).toString().replaceAll("+", "%20") : options.query;
    const basePath = this.pathStyle ? `${this.pathPrefix}/${bucketName}/${options.objectName}` : `/${options.objectName}`;
    const querySuffix = queryAsString ? `?${queryAsString}` : "";
    const path = basePath + querySuffix;
    const encodedPath = basePath.split("/").map((part) => encodeURIComponent(part)).join("/") + querySuffix;
    return {
      headers,
      host,
      path,
      encodedPath
    };
  }
  async makeRequest({ method, payload, ...options }) {
    const date = new Date;
    const { headers, host, path, encodedPath } = this.buildRequestOptions(options);
    const statusCode = options.statusCode ?? 200;
    if (method === "POST" || method === "PUT" || method === "DELETE") {
      if (payload === undefined) {
        payload = new Uint8Array;
      } else if (typeof payload === "string") {
        payload = encoder.encode(payload);
      }
      headers.set("Content-Length", String(payload.length));
    } else if (payload) {
      throw new Error(`Unexpected payload on ${method} request.`);
    }
    const sha256sum = await sha256digestHex(payload ?? new Uint8Array);
    headers.set("x-amz-date", makeDateLong(date));
    headers.set("x-amz-content-sha256", sha256sum);
    if (this.accessKey) {
      if (this.sessionToken) {
        headers.set("x-amz-security-token", this.sessionToken);
      }
      headers.set("authorization", await signV4({
        headers,
        method,
        path,
        accessKey: this.accessKey,
        secretKey: this.#secretKey,
        region: this.region,
        date
      }));
    }
    const fullUrl = `${this.protocol}//${host}${encodedPath}`;
    const response = await fetch(fullUrl, {
      method,
      headers,
      body: payload
    });
    if (response.status !== statusCode) {
      if (response.status >= 400) {
        const error = await parseServerError(response);
        throw error;
      } else if (response.status === 301) {
        throw new ServerError(response.status, "UnexpectedRedirect", `The server unexpectedly returned a redirect response. With AWS S3, this usually means you need to use a ` + `region-specific endpoint like "s3.us-west-2.amazonaws.com" instead of "s3.amazonaws.com"`);
      }
      throw new ServerError(response.status, "UnexpectedStatusCode", `Unexpected response code from the server (expected ${statusCode}, got ${response.status} ${response.statusText}).`);
    }
    if (!options.returnBody) {
      await response.body?.cancel();
    }
    return response;
  }
  async deleteObject(objectName, options = {}) {
    const bucketName = this.checkNames(objectName, options);
    const query = options.versionId ? {
      versionId: options.versionId
    } : {};
    const headers = new Headers;
    if (options.governanceBypass) {
      headers.set("X-Amz-Bypass-Governance-Retention", "true");
    }
    await this.makeRequest({
      method: "DELETE",
      bucketName,
      objectName,
      headers,
      query,
      statusCode: 204
    });
  }
  async exists(objectName, options) {
    try {
      await this.statObject(objectName, options);
      return true;
    } catch (err) {
      if (err instanceof ServerError && err.statusCode === 404) {
        return false;
      }
      throw err;
    }
  }
  getObject(objectName, options) {
    return this.getPartialObject(objectName, {
      ...options,
      offset: 0,
      length: 0
    });
  }
  async getPartialObject(objectName, { offset, length, ...options }) {
    const bucketName = this.checkNames(objectName, options);
    const headers = new Headers(Object.entries(options.metadata ?? {}));
    let statusCode = 200;
    if (offset || length) {
      headers.set("Range", `bytes=${offset || 0}-${length ? (offset || 0) + length - 1 : ""}`);
      statusCode = 206;
    }
    const query = {
      ...options.responseParams,
      ...options.versionId ? {
        versionId: options.versionId
      } : {}
    };
    return await this.makeRequest({
      method: "GET",
      bucketName,
      objectName,
      headers,
      query,
      statusCode,
      returnBody: true
    });
  }
  async getPresignedUrl(method, objectName, options = {}) {
    if (!this.accessKey) {
      throw new AccessKeyRequiredError;
    }
    const bucketName = this.checkNames(objectName, options);
    const { headers, path } = this.buildRequestOptions({
      objectName,
      bucketName,
      query: options.parameters,
      headers: new Headers(options.extraHeaders)
    });
    const requestDate = options.requestDate ?? new Date;
    const expirySeconds = options.expirySeconds ?? 24 * 60 * 60 * 7;
    return await presignV4({
      protocol: this.protocol,
      headers,
      method,
      path,
      accessKey: this.accessKey,
      secretKey: this.#secretKey,
      sessionToken: this.sessionToken,
      region: this.region,
      date: requestDate,
      expirySeconds
    });
  }
  presignedGetObject(objectName, options = {}) {
    const { versionId, responseParams, ...otherOptions } = options;
    const parameters = {
      ...responseParams,
      ...versionId ? {
        versionId
      } : {}
    };
    return this.getPresignedUrl("GET", objectName, {
      parameters,
      ...otherOptions
    });
  }
  async* listObjects(options = {}) {
    for await (const result of this.listObjectsGrouped({
      ...options,
      delimiter: ""
    })) {
      if (result.type === "Object") {
        yield result;
      } else {
        throw new Error(`Unexpected result from listObjectsGrouped(): ${result}`);
      }
    }
  }
  async* listObjectsGrouped(options) {
    const bucketName = this.getBucketName(options);
    let continuationToken = options.continuationToken ?? "";
    const pageSize = options.pageSize ?? 1000;
    if (pageSize < 1 || pageSize > 1000) {
      throw new InvalidArgumentError("pageSize must be between 1 and 1,000.");
    }
    let resultCount = 0;
    while (true) {
      const maxKeys = options.maxResults ? Math.min(pageSize, options.maxResults - resultCount) : pageSize;
      if (maxKeys === 0) {
        return;
      }
      const pageResponse = await this.makeRequest({
        method: "GET",
        bucketName,
        objectName: "",
        query: {
          "list-type": "2",
          prefix: options.prefix ?? "",
          delimiter: options.delimiter ?? "",
          "max-keys": String(maxKeys),
          ...continuationToken ? {
            "continuation-token": continuationToken
          } : {}
        },
        returnBody: true
      });
      const responseText = await pageResponse.text();
      const root = parse(responseText);
      if (root?.name !== "ListBucketResult") {
        throw new Error(`Unexpected response: ${responseText}`);
      }
      const prefixElements = root.children.filter((c) => c.name === "CommonPrefixes").flatMap((c) => c.children);
      const toYield = [];
      for (const prefixElement of prefixElements) {
        toYield.push({
          type: "CommonPrefix",
          prefix: prefixElement.content ?? ""
        });
        resultCount++;
      }
      for (const objectElement of root.children.filter((c) => c.name === "Contents")) {
        toYield.push({
          type: "Object",
          key: childText(objectElement, "Key") ?? "",
          etag: sanitizeETag(childText(objectElement, "ETag") ?? ""),
          size: parseInt(childText(objectElement, "Size") ?? "", 10),
          lastModified: new Date(childText(objectElement, "LastModified") ?? "invalid")
        });
        resultCount++;
      }
      toYield.sort((a, b) => {
        const aStr = a.type === "Object" ? a.key : a.prefix;
        const bStr = b.type === "Object" ? b.key : b.prefix;
        return aStr > bStr ? 1 : aStr < bStr ? -1 : 0;
      });
      for (const entry of toYield) {
        yield entry;
      }
      const isTruncated = childText(root, "IsTruncated") === "true";
      if (isTruncated) {
        const nextContinuationToken = childText(root, "NextContinuationToken");
        if (!nextContinuationToken) {
          throw new Error("Unexpectedly missing continuation token, but server said there are more results.");
        }
        continuationToken = nextContinuationToken;
        if (options.maxResults && resultCount >= options.maxResults) {
          return continuationToken;
        }
      } else {
        return;
      }
    }
  }
  async putObject(objectName, streamOrData, options) {
    const bucketName = this.checkNames(objectName, options);
    let size;
    let bytes;
    if (!(streamOrData instanceof ReadableStream)) {
      bytes = typeof streamOrData === "string" ? encoder.encode(streamOrData) : streamOrData;
      if (!(bytes instanceof Uint8Array))
        throw new InvalidArgumentError(`Invalid stream/data type provided.`);
      size = bytes.byteLength;
    }
    if (options?.size !== undefined) {
      if (size !== undefined && options?.size !== size) {
        throw new InvalidArgumentError(`size was specified (${options.size}) but doesn't match auto-detected size (${size}).`);
      }
      if (typeof options.size !== "number" || options.size < 0 || isNaN(options.size)) {
        throw new InvalidArgumentError(`invalid size specified: ${options.size}`);
      } else {
        size = options.size;
      }
    }
    const partSize = options?.partSize ?? this.calculatePartSize(size);
    if (partSize < minimumPartSize) {
      throw new InvalidArgumentError(`Part size should be greater than 5MB`);
    } else if (partSize > maximumPartSize) {
      throw new InvalidArgumentError(`Part size should be less than 5GB`);
    }
    const metadata = options?.metadata ?? {};
    if (bytes !== undefined && bytes.byteLength < partSize) {
      return uploadSingleRequest({
        client: this,
        bucketName,
        objectName,
        metadata,
        payload: bytes
      });
    }
    const stream = bytes === undefined ? streamOrData : new Blob([
      bytes
    ]).stream();
    const chunker = new TransformChunkSizes(partSize);
    const uploader = new ObjectUploader({
      client: this,
      bucketName,
      objectName,
      partSize,
      metadata
    });
    await stream.pipeThrough(chunker).pipeTo(uploader);
    return uploader.getResult();
  }
  calculatePartSize(size) {
    if (size === undefined) {
      return defaultPartSize;
    }
    if (size > maxObjectSize) {
      throw new TypeError(`size should not be more than ${maxObjectSize}`);
    }
    let partSize = defaultPartSize;
    while (true) {
      if (partSize * 1e4 > size) {
        return partSize;
      }
      partSize += 16 * 1024 * 1024;
    }
  }
  async statObject(objectName, options) {
    const bucketName = this.checkNames(objectName, options);
    const query = {};
    if (options?.versionId) {
      query.versionId = options.versionId;
    }
    const response = await this.makeRequest({
      method: "HEAD",
      bucketName,
      objectName,
      query,
      headers: new Headers(options?.headers)
    });
    const metadata = {};
    for (const header of metadataKeys) {
      if (response.headers.has(header)) {
        metadata[header] = response.headers.get(header);
      }
    }
    response.headers.forEach((_value, key) => {
      if (key.startsWith("x-amz-meta-")) {
        metadata[key] = response.headers.get(key);
      }
    });
    return {
      type: "Object",
      key: objectName,
      size: parseInt(response.headers.get("content-length") ?? "", 10),
      metadata,
      lastModified: new Date(response.headers.get("Last-Modified") ?? "error: missing last modified"),
      versionId: response.headers.get("x-amz-version-id") || null,
      etag: sanitizeETag(response.headers.get("ETag") ?? "")
    };
  }
  async copyObject(source, objectName, options) {
    const bucketName = this.checkNames(objectName, options);
    const sourceBucketName = source.sourceBucketName ?? bucketName;
    let xAmzCopySource = `${sourceBucketName}/${source.sourceKey.split("/").map((part) => encodeURIComponent(part)).join("/")}`;
    if (source.sourceVersionId)
      xAmzCopySource += `?versionId=${source.sourceVersionId}`;
    const headers = new Headers(options?.metadata);
    if (options?.metadata !== undefined) {
      headers.set("x-amz-metadata-directive", "REPLACE");
    }
    headers.set("x-amz-copy-source", xAmzCopySource);
    const response = await this.makeRequest({
      method: "PUT",
      bucketName,
      objectName,
      headers,
      returnBody: true
    });
    const responseText = await response.text();
    const root = parse(responseText);
    if (root?.name !== "CopyObjectResult") {
      throw new Error(`Unexpected response: ${responseText}`);
    }
    const etagString = childText(root, "ETag") ?? "";
    const lastModifiedString = childText(root, "LastModified");
    if (lastModifiedString === undefined) {
      throw new Error("Unable to find <LastModified>...</LastModified> from the server.");
    }
    return {
      copySourceVersionId: response.headers.get("x-amz-copy-source-version-id") || null,
      etag: sanitizeETag(etagString),
      lastModified: new Date(lastModifiedString),
      versionId: response.headers.get("x-amz-version-id") || null
    };
  }
  async bucketExists(bucketName) {
    try {
      const objects = this.listObjects({
        bucketName
      });
      await objects.next();
      return true;
    } catch (err) {
      if (err instanceof ServerError && err.statusCode === 404) {
        return false;
      }
      throw err;
    }
  }
  async makeBucket(bucketName) {
    await this.makeRequest({
      method: "PUT",
      bucketName: this.getBucketName({
        bucketName
      }),
      objectName: "",
      statusCode: 200
    });
  }
  async removeBucket(bucketName) {
    await this.makeRequest({
      method: "DELETE",
      bucketName: this.getBucketName({
        bucketName
      }),
      objectName: "",
      statusCode: 204
    });
  }
  async presignedPostObject(objectName, options = {}) {
    const bucketName = this.checkNames(objectName, options);
    const requestDate = options.requestDate || new Date;
    const expirySeconds = options.expirySeconds ?? 3600;
    return await presignPostV4({
      protocol: this.protocol,
      host: this.host,
      bucket: bucketName,
      objectKey: objectName,
      accessKey: this.accessKey || "",
      secretKey: this.#secretKey || "",
      region: this.region,
      date: requestDate,
      expirySeconds,
      conditions: options.conditions,
      fields: options.fields
    });
  }
}
export { exports_errors, Client };

//# debugId=7C34146CC313054864756E2164756E21
