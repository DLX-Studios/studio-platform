/** Shared analytics summary — used by GET /api/analytics and the page load. */
import { listSessions, listMachines, type Session } from '#lib/server/db.js';

export interface CompletedSession extends Session {
  machine_name: string;
  hours: number;
  cost: number;
}

export function analyticsSummary() {
  const machines = new Map(listMachines().map((m) => [m.id, m.name]));
  const sessions = listSessions().filter((s) => s.stopped_at);

  const completed = sessions.map((s: Session) => {
    const start = new Date(s.started_at.replace(' ', 'T') + 'Z').getTime();
    const end = new Date(s.stopped_at!.replace(' ', 'T') + 'Z').getTime();
    const hours = Math.max(0, (end - start) / 36e5);
    const cost = hours * (s.cost_per_hour ?? 0);
    return {
      ...s,
      machine_name: machines.get(s.machine_id) ?? `machine ${s.machine_id}`,
      hours: +hours.toFixed(2),
      cost: +cost.toFixed(3),
    };
  });

  return {
    sessions: completed,
    totalHours: +completed.reduce((a, s) => a + s.hours, 0).toFixed(2),
    totalCost: +completed.reduce((a, s) => a + s.cost, 0).toFixed(3),
  };
}
