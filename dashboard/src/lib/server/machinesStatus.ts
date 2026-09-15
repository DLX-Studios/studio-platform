/**
 * Shared machine-status aggregation — used by GET /api/machines, GET
 * /api/machines/[id] and the page server loads so all return identical,
 * SSR-friendly data.
 */
import { listAllDroplets, listDropletsByTag, statusFromDroplet, sweepOrphanVolumes } from '#lib/doClient.js';
import { getMachine, listMachines, machineTag } from './db.js';
import { probeUntilReady } from './daemonProbe.js';

export async function machinesWithStatus() {
  // Safety net for leaked volumes (throttled internally to once per 5 min).
  void sweepOrphanVolumes().catch(() => {});

  const machines = listMachines();
  let droplets: Awaited<ReturnType<typeof listAllDroplets>>['droplets'] = [];
  try {
    droplets = (await listAllDroplets()).droplets;
  } catch {
    // DO unreachable — cards still render, status unknown
  }

  return Promise.all(
    machines.map(async (m) => {
      const d = droplets.find((droplet) => droplet.tags?.includes(machineTag(m.id)));
      const status = statusFromDroplet(d);
      if (status.state === 'active') {
        status.state = await probeUntilReady(m, 'active', status.ip);
      }
      return { ...m, status };
    }),
  );
}

export async function machineWithStatus(id: number) {
  void sweepOrphanVolumes().catch(() => {});

  const machine = getMachine(id);
  if (!machine) {
    return { machine: null, status: { running: false, state: 'off', ip: null, droplet_id: null } };
  }

  let status = {
    running: false,
    state: 'off',
    ip: null as string | null,
    droplet_id: null as number | null,
  };
  try {
    const data = await listDropletsByTag(machineTag(machine.id));
    status = statusFromDroplet(data.droplets[0]);
  } catch {
    // offline DO — machine row still returned
  }
  if (status.state === 'active') {
    status.state = await probeUntilReady(machine, 'active', status.ip);
  }
  return { machine, status };
}
