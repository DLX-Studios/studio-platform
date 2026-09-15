// @bun
import {
  listMachines,
  listSessions
} from "./index-jr5xvt5y.js";

// .svelte-kit/output/server/chunks/analytics.js
function analyticsSummary() {
  const machines = new Map(listMachines().map((m) => [m.id, m.name]));
  const completed = listSessions().filter((s) => s.stopped_at).map((s) => {
    const start = (/* @__PURE__ */ new Date(s.started_at.replace(" ", "T") + "Z")).getTime();
    const end = (/* @__PURE__ */ new Date(s.stopped_at.replace(" ", "T") + "Z")).getTime();
    const hours = Math.max(0, (end - start) / 3600000);
    const cost = hours * (s.cost_per_hour ?? 0);
    return {
      ...s,
      machine_name: machines.get(s.machine_id) ?? `machine ${s.machine_id}`,
      hours: +hours.toFixed(2),
      cost: +cost.toFixed(3)
    };
  });
  return {
    sessions: completed,
    totalHours: +completed.reduce((a, s) => a + s.hours, 0).toFixed(2),
    totalCost: +completed.reduce((a, s) => a + s.cost, 0).toFixed(3)
  };
}

export { analyticsSummary };

//# debugId=CBD98F7B1A41432764756E2164756E21
