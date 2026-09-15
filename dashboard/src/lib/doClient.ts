import { FetchRequestAdapter } from "@microsoft/kiota-http-fetchlibrary";
import { getSetting, getMachine, addEvent, machineTag } from "./server/db.js";
import { loadDoSdk } from "./sdkLoader.js";

export { machineTag };

/* --------------------------------------------------------------------------
   Client factory — token comes from SQLite (or an override for setup probe).
   The SDK is loaded dynamically (see sdkLoader.ts) so its untypecheckable
   .ts sources never enter our typecheck program.
   -------------------------------------------------------------------------- */
async function makeClient(tokenOverride?: string) {
  const sdk = await loadDoSdk();
  const token = tokenOverride ?? getSetting("do_token") ?? "";
  const authProvider = new sdk.DigitalOceanApiKeyAuthenticationProvider(token);
  const adapter = new FetchRequestAdapter(authProvider);
  return sdk.createDigitalOceanClient(adapter);
}

/* --------------------------------------------------------------------------
   Plain shapes the rest of the app consumes (never leak SDK types)
   -------------------------------------------------------------------------- */
export interface Droplet {
  id: number;
  name: string;
  status: string;
  tags: string[];
  networks: { v4: { ipAddress: string; type: string }[] };
  sizeSlug: string;
  region: { slug: string };
  createdAt: string | null;
}

export interface CreateDropletOptions {
  name: string;
  region: string;
  size: string;
  image: string;
  ssh_keys: number[];
  tags: string[];
  user_data?: string;
  dedicatedCpu?: boolean;
  volumes?: number[];
  monitoring?: boolean;
}

function toPlainDroplet(d: Record<string, any>): Droplet {
  return {
    id: d?.id ?? 0,
    name: d?.name ?? "",
    status: d?.status ?? "off",
    tags: d?.tags ?? [],
    networks: {
      v4: (d?.networks?.v4 ?? []).map((n: Record<string, any>) => ({
        ipAddress: n?.ipAddress ?? "",
        type: n?.type ?? "",
      })),
    },
    sizeSlug: d?.sizeSlug ?? "",
    region: { slug: d?.region?.slug ?? "" },
    createdAt: d?.createdAt instanceof Date ? d.createdAt.toISOString() : (d?.createdAt ?? null),
  };
}

/* --------------------------------------------------------------------------
   Error classification (design: reject vs network, never blame the token twice)
   -------------------------------------------------------------------------- */
export function describeDoError(e: unknown): {
  reason: "rejected" | "network" | "rate";
  message: string;
} {
  const err = e as { responseStatusCode?: number; message?: string };
  const code = err?.responseStatusCode;

  if (code === 401 || code === 403) {
    return {
      reason: "rejected",
      message: `DigitalOcean rejected this Personal Access Token (wrong or revoked, status ${code}). It is not a network error — do not generate a new one unless you are sure this one is dead.`,
    };
  }
  if (code === 429) {
    return {
      reason: "rate",
      message: "DigitalOcean is rate-limiting this token. Wait a moment and try again.",
    };
  }
  if (typeof code === "number" && code >= 500) {
    return {
      reason: "network",
      message: `DigitalOcean had a server error (status ${code}). Do not generate a new Personal Access Token — this is not your token's fault.`,
    };
  }
  return {
    reason: "network",
    message:
      "Could not reach DigitalOcean. Check your connection — do not generate a new Personal Access Token yet.",
  };
}

/* --------------------------------------------------------------------------
   Operations
   -------------------------------------------------------------------------- */
export async function listAllDroplets(tokenOverride?: string): Promise<{ droplets: Droplet[] }> {
  const data = await doFetch("/droplets?per_page=200", {}, tokenOverride);
  return { droplets: (data?.droplets ?? []).map(toPlainDropletRest) };
}

export async function listDropletsByTag(
  tag: string,
  tokenOverride?: string,
): Promise<{ droplets: Droplet[] }> {
  // Plain REST: droplet payloads include volume_ids once a volume is attached,
  // which the SDK's deserializer cannot parse.
  const data = await doFetch(
    `/droplets?tag_name=${encodeURIComponent(tag)}&per_page=200`,
    {},
    tokenOverride,
  );
  return { droplets: (data?.droplets ?? []).map(toPlainDropletRest) };
}

function toPlainDropletRest(d: Record<string, any>): Droplet {
  // Plain-REST responses use snake_case (unlike the SDK's camelCase mapping).
  return {
    id: d?.id ?? 0,
    name: d?.name ?? "",
    status: d?.status ?? "off",
    tags: d?.tags ?? [],
    networks: {
      v4: (d?.networks?.v4 ?? []).map((n: Record<string, any>) => ({
        ipAddress: n?.ip_address ?? "",
        type: n?.type ?? "",
      })),
    },
    sizeSlug: d?.size_slug ?? "",
    region: { slug: d?.region?.slug ?? "" },
    createdAt: d?.created_at ?? null,
  };
}

export async function createDroplet(
  body: CreateDropletOptions,
  tokenOverride?: string,
): Promise<{ droplet: Droplet }> {
  // Plain REST: the SDK's deserializer chokes on the droplet-create response
  // ("unsupported type during deserialization"), same class of bug as regions/sizes.
  const data = await doFetch(
    "/droplets",
    {
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
        ...(body.dedicatedCpu ? { dedicated_cpu: true } : {}),
        ...(body.volumes?.length ? { volumes: body.volumes } : {}),
      }),
    },
    tokenOverride,
  );
  const d = data?.droplet;
  if (!d) throw new Error("DO returned no droplet in create response");
  return { droplet: toPlainDropletRest(d) };
}

export async function deleteDroplet(id: number): Promise<null> {
  const client = await makeClient();
  await client.v2.droplets.byDroplet_id(id).delete();
  return null;
}

/** Validate a token — throws with responseStatusCode on rejection. */
export async function getAccount(tokenOverride?: string) {
  const client = await makeClient(tokenOverride);
  return client.v2.account.get();
}

export async function listSshKeys(
  tokenOverride?: string,
): Promise<{ id: number; name: string; fingerprint: string }[]> {
  const client = await makeClient(tokenOverride);
  const res = await client.v2.account.keys.get();
  return (res?.sshKeys ?? []).map((k: Record<string, any>) => ({
    id: k?.id ?? 0,
    name: k?.name ?? "",
    fingerprint: k?.fingerprint ?? "",
  }));
}

export async function listDropletSnapshots(
  tokenOverride?: string,
): Promise<{ id: string; name: string }[]> {
  const client = await makeClient(tokenOverride);
  const res = await client.v2.snapshots.get({ resourceType: "droplet", perPage: 200 });
  return (res?.snapshots ?? []).map((s: Record<string, any>) => ({
    id: String(s?.id ?? ""),
    name: s?.name ?? "",
  }));
}

/* --------------------------------------------------------------------------
   Regions & sizes (for the DO-style machine creation flow)
   -------------------------------------------------------------------------- */
export interface DoRegion {
  slug: string;
  name: string;
}

/**
 * Regions & sizes go through plain REST: the dynamically-loaded SDK fails
 * to deserialize these two responses ("unsupported type during
 * deserialization"), while plain fetch + our own mapping works fine.
 */
export async function listRegions(tokenOverride?: string): Promise<DoRegion[]> {
  const data = await doFetch("/regions?per_page=200", {}, tokenOverride);
  return (data?.regions ?? [])
    .filter((r: Record<string, any>) => r?.available)
    .map((r: Record<string, any>) => ({ slug: r?.slug ?? "", name: r?.name ?? r?.slug ?? "" }));
}

export interface DoSize {
  slug: string;
  vcpus: number;
  memoryGb: number;
  diskGb: number;
  hourly: number;
  monthly: number;
  regions: string[];
  available: boolean;
  description: string;
}

export async function listSizes(tokenOverride?: string): Promise<DoSize[]> {
  const data = await doFetch("/sizes?per_page=200", {}, tokenOverride);
  return (data?.sizes ?? []).map((s: Record<string, any>) => ({
    slug: s?.slug ?? "",
    vcpus: s?.vcpus ?? 0,
    memoryGb: Math.round((s?.memory ?? 0) / 1024), // API reports MB
    diskGb: s?.disk ?? 0,
    hourly: s?.price_hourly ?? 0,
    monthly: s?.price_monthly ?? 0,
    regions: s?.regions ?? [],
    available: s?.available ?? false,
    description: s?.description ?? "",
  }));
}

/* --------------------------------------------------------------------------
   Volumes (additional storage). Plain REST — the SDK's volume request
   builders are not part of the typed subset we rely on elsewhere.
   -------------------------------------------------------------------------- */
function doFetch(path: string, init: RequestInit = {}, tokenOverride?: string): Promise<any> {
  const token = tokenOverride ?? getSetting("do_token") ?? "";
  return fetch(`https://api.digitalocean.com/v2${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  }).then(async (r) => {
    const data = await r.json().catch(() => ({}));
    if (!r.ok) {
      throw Object.assign(new Error(data?.message ?? `DO request failed (${r.status})`), {
        responseStatusCode: r.status,
      });
    }
    return data;
  });
}

/* --------------------------------------------------------------------------
   Distributions (boot from a plain OS instead of a golden snapshot)
   -------------------------------------------------------------------------- */
export interface DoImage {
  id: string;
  slug: string;
  name: string;
  distribution: string;
}

export async function listDistributions(tokenOverride?: string): Promise<DoImage[]> {
  const data = await doFetch("/images?type=distribution&per_page=200", {}, tokenOverride);
  // Note: distribution images don't carry an `available` flag (that's a sizes
  // concept) — filtering on it returned an empty list. Keep x64 images only
  // (our size catalog is x64) and skip the GPU base images that DO tags as
  // Ubuntu distributions.
  return (data?.images ?? [])
    .filter((i: Record<string, any>) => typeof i?.slug === "string" && i.slug.endsWith("-x64") && !i.slug.startsWith("gpu-"))
    .map((i: Record<string, any>) => ({
      id: String(i?.id ?? ""),
      slug: i?.slug ?? "",
      name: i?.name ?? "",
      distribution: i?.distribution ?? "",
    }));
}

/** Create an SSH key on the DO account (wizard: "paste a public key"). */
export async function createSshKey(
  name: string,
  publicKey: string,
  tokenOverride?: string,
): Promise<{ id: number; name: string; fingerprint: string }> {
  const data = await doFetch(
    "/account/keys",
    { method: "POST", body: JSON.stringify({ name, public_key: publicKey }) },
    tokenOverride,
  );
  return {
    id: data?.ssh_key?.id ?? 0,
    name: data?.ssh_key?.name ?? "",
    fingerprint: data?.ssh_key?.fingerprint ?? "",
  };
}

/** Queue a snapshot of a (running) droplet. DO processes it asynchronously. */
export async function createDropletSnapshot(
  dropletId: number,
  name: string,
): Promise<{ actionId: number }> {
  const data = await doFetch(`/droplets/${dropletId}/actions`, {
    method: "POST",
    body: JSON.stringify({ type: "snapshot", name }),
  });
  return { actionId: data?.action?.id ?? 0 };
}

export async function listVolumesByName(
  name: string,
): Promise<{ id: number; region: string; dropletIds: number[] }[]> {
  const data = await doFetch(`/volumes?name=${encodeURIComponent(name)}`);
  return (data?.volumes ?? []).map((v: Record<string, any>) => ({
    id: v?.id ?? 0,
    region: v?.region?.slug ?? "",
    dropletIds: v?.droplet_ids ?? [],
  }));
}

/** Detach a volume from a droplet (no-op if already detached). */
export async function detachVolume(id: string, dropletId: number): Promise<null> {
  await doFetch(`/volumes/${id}?droplet_id=${dropletId}`, { method: "DELETE" });
  return null;
}

export async function createVolume(args: {
  name: string;
  region: string;
  sizeGb: number;
}): Promise<{ id: number }> {
  const data = await doFetch("/volumes", {
    method: "POST",
    body: JSON.stringify({
      name: args.name,
      region: args.region,
      size_gigabytes: args.sizeGb,
      filesystem_type: "ext4",
    }),
  });
  return { id: data?.volume?.id ?? 0 };
}

export async function deleteVolume(id: string): Promise<null> {
  await doFetch(`/volumes/${id}`, { method: "DELETE" });
  return null;
}

let lastSweep = 0;

/**
 * Safety net: delete volumes named sb-machine-<id>-data whose machine
 * profile no longer exists (e.g. leaked when a delete's DO cleanup failed).
 * Throttled to once per 5 minutes; safe to call on every status poll.
 */
export async function sweepOrphanVolumes(): Promise<number> {
  if (Date.now() - lastSweep < 5 * 60_000) return 0;
  lastSweep = Date.now();
  let deleted = 0;
  try {
    const data = await doFetch("/volumes?per_page=200");
    for (const v of data?.volumes ?? []) {
      const match = /^sb-machine-(\d+)-data$/.exec(v?.name ?? "");
      if (!match) continue;
      const machineId = Number(match[1]);
      if (getMachine(machineId)) continue; // profile still exists — keep it
      const attached = (v.droplet_ids ?? [])[0];
      const url = attached ? `/volumes/${v.id}?droplet_id=${attached}` : `/volumes/${v.id}`;
      try {
        await doFetch(url, { method: "DELETE" });
        deleted++;
        addEvent(machineId, "destroy", `Orphan volume ${v.name} deleted (profile gone)`);
      } catch {
        // detach race or DO hiccup — the next sweep retries
      }
    }
  } catch {
    // DO unreachable — retried on a later cycle
  }
  return deleted;
}

/* --------------------------------------------------------------------------
   Status mapping
   -------------------------------------------------------------------------- */
export function statusFromDroplet(d: Droplet | undefined): {
  running: boolean;
  state: string;
  ip: string | null;
  droplet_id: number | null;
} {
  if (!d || d.id === 0) return { running: false, state: "off", ip: null, droplet_id: null };
  const ip = d.networks.v4.find((n) => n.type === "public")?.ipAddress ?? null;
  return {
    running: d.status === "new" || d.status === "active",
    state: d.status === "new" ? "starting" : d.status,
    ip,
    droplet_id: d.id,
  };
}

export function sshKeyIds(): number[] {
  return (getSetting("do_ssh_key_ids") ?? "").split(",").filter(Boolean).map(Number);
}
