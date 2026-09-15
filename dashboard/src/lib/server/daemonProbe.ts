/**
 * Pull-mode daemon probe: the dashboard (possibly behind NAT) polls the
 * machine's public IP; the daemon serves a token-authenticated /probe
 * endpoint. A 200 with ok:true flips the machine to "ready".
 *
 * Dial-home remains supported for when the dashboard is publicly reachable.
 */
import { getMachineByDaemonToken, setDaemonReady } from '#lib/server/db.js';

const PROBE_PORT = 4545;
const PROBE_TIMEOUT_MS = 2500;

export interface DaemonProbeResult {
  ok: boolean;
  data?: {
    hostname?: string;
    daemon_version?: string;
    rustc_version?: string;
    cargo_version?: string;
    sccache_version?: string;
    os?: string;
    disk_root_free_gb?: number;
    disk_workspace_free_gb?: number;
  };
}

export async function probeDaemon(ip: string | null, token: string): Promise<DaemonProbeResult> {
  if (!ip || !token) return { ok: false };
  try {
    const r = await fetch(`http://${ip}:${PROBE_PORT}/probe`, {
      headers: { Authorization: `Bearer ${token}` },
      signal: AbortSignal.timeout(PROBE_TIMEOUT_MS),
    });
    if (!r.ok) return { ok: false };
    const data = await r.json();
    return { ok: !!data, data };
  } catch {
    return { ok: false };
  }
}

/**
 * If the machine is booting/provisioning with a known IP, probe it and mark
 * it ready when the daemon answers. Returns the (possibly updated) state.
 */
export async function probeUntilReady(
  machine: { id: number; daemon_token: string; daemon_ready: number },
  state: string,
  ip: string | null,
): Promise<string> {
  if (state !== 'active' && state !== 'provisioning') return state;
  if (machine.daemon_ready || !machine.daemon_token) return machine.daemon_ready ? 'ready' : 'provisioning';

  const result = await probeDaemon(ip, machine.daemon_token);
  if (result.ok) {
    setDaemonReady(machine.id, true);
    return 'ready';
  }
  return 'provisioning';
}
