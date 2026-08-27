import {
  createPrivateKey,
  createPublicKey,
  sign as ed25519Sign,
} from "node:crypto";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const supportedExamples = new Set(["starter", "pos-desktop", "github-viewer"]);
const encoder = new TextEncoder();
const EXAMPLE_SEED = Buffer.alloc(32, 7);
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const examplePrivateKey = createPrivateKey({
  key: Buffer.concat([PKCS8_PREFIX, EXAMPLE_SEED]),
  format: "der",
  type: "pkcs8",
});

export const EXAMPLE_PUBLIC_KEY_DER = createPublicKey(examplePrivateKey).export({
  format: "der",
  type: "spki",
});

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

export interface BundleBuild {
  bytes: Uint8Array;
  outputPath: string;
  signedDocument: Uint8Array;
}

interface ZipEntry {
  path: string;
  bytes: Uint8Array;
}

function lexicalCompare(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function canonicalize(value: Json): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalize).join(",")}]`;
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalize(value[key]!)}`)
    .join(",")}}`;
}

function sha256(bytes: Uint8Array): string {
  return new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffff_ffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb8_8320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffff_ffff) >>> 0;
}

function u16(value: number): Buffer {
  const output = Buffer.allocUnsafe(2);
  output.writeUInt16LE(value);
  return output;
}

function u32(value: number): Buffer {
  const output = Buffer.allocUnsafe(4);
  output.writeUInt32LE(value >>> 0);
  return output;
}

function deterministicZip(entries: ZipEntry[]): Uint8Array {
  const ordered = [...entries].sort((left, right) => lexicalCompare(left.path, right.path));
  const localParts: Buffer[] = [];
  const centralParts: Buffer[] = [];
  let offset = 0;
  for (const entry of ordered) {
    const name = Buffer.from(entry.path, "utf8");
    const contents = Buffer.from(entry.bytes);
    const checksum = crc32(contents);
    const local = Buffer.concat([
      u32(0x0403_4b50),
      u16(20),
      u16(0),
      u16(0),
      u16(0),
      u16(0x21),
      u32(checksum),
      u32(contents.length),
      u32(contents.length),
      u16(name.length),
      u16(0),
      name,
      contents,
    ]);
    localParts.push(local);
    centralParts.push(
      Buffer.concat([
        u32(0x0201_4b50),
        u16(0x0314),
        u16(20),
        u16(0),
        u16(0),
        u16(0),
        u16(0x21),
        u32(checksum),
        u32(contents.length),
        u32(contents.length),
        u16(name.length),
        u16(0),
        u16(0),
        u16(0),
        u16(0),
        u32(0x81a4_0000),
        u32(offset),
        name,
      ]),
    );
    offset += local.length;
  }
  const central = Buffer.concat(centralParts);
  return Buffer.concat([
    ...localParts,
    central,
    u32(0x0605_4b50),
    u16(0),
    u16(0),
    u16(ordered.length),
    u16(ordered.length),
    u32(central.length),
    u32(offset),
    u16(0),
  ]);
}

export function canonicalBundleDocument(
  manifest: Json,
  module: Uint8Array,
  assets: Map<string, Uint8Array>,
): Uint8Array {
  const document: Json = {
    domain: "studio.bundle.signature.v1",
    manifest,
    module: {
      path: "module.wasm",
      length: module.length,
      sha256: sha256(module),
    },
    assets: [...assets.entries()]
      .sort(([left], [right]) => lexicalCompare(left, right))
      .map(([path, bytes]) => ({
        path,
        length: bytes.length,
        sha256: sha256(bytes),
      })),
  };
  return encoder.encode(canonicalize(document));
}

export async function buildExample(example = "pos-desktop"): Promise<BundleBuild> {
  if (!supportedExamples.has(example)) throw new Error(`unsupported example: ${example}`);
  // If wasm already exists, skip rebuild (allows manual bun-asc builds in sandboxed env)
  const prebuilt = Bun.file(join(root, "examples", example, "build", `${example}.wasm`));
  if (!(await prebuilt.exists())) {
    const packageName = `@studio/example-${example}`;
    const child = Bun.spawn(["bun", "run", "--filter", packageName, "build"], {
      cwd: root,
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });
    if ((await child.exited) !== 0) throw new Error(`AssemblyScript build failed: ${example}`);
  }

  const directory = join(root, "examples", example);
  const manifest = (await Bun.file(join(directory, "manifest.json")).json()) as Json;
  const manifestBytes = encoder.encode(canonicalize(manifest));
  const module = new Uint8Array(
    await Bun.file(join(directory, "build", `${example}.wasm`)).arrayBuffer(),
  );
  const declaredAssets = (manifest as { assets?: string[] }).assets ?? [];
  const assets = new Map<string, Uint8Array>();
  for (const assetPath of declaredAssets) {
    assets.set(assetPath, new Uint8Array(await Bun.file(join(directory, assetPath)).arrayBuffer()));
  }
  const signedDocument = canonicalBundleDocument(manifest, module, assets);
  const signature = ed25519Sign(null, signedDocument, examplePrivateKey);
  const bytes = deterministicZip([
    ...Array.from(assets.entries(), ([path, bytes]) => ({ path, bytes })),
    { path: "manifest.json", bytes: manifestBytes },
    { path: "module.wasm", bytes: module },
    { path: "signature.ed25519", bytes: signature },
  ]);
  const outputPath = join(directory, "build", `${example}.studio`);
  await Bun.write(outputPath, bytes);
  return { bytes, outputPath, signedDocument };
}

if (import.meta.main) {
  const example = process.argv[2];
  if (!example || !supportedExamples.has(example)) {
    console.error("usage: bun run ./scripts/build-example.ts <pos-desktop|starter|github-viewer>");
    process.exit(2);
  }
  const result = await buildExample(example);
  console.log(`built ${result.outputPath} (${result.bytes.length} bytes)`);
}
