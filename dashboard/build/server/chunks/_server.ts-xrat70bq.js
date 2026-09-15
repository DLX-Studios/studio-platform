// @bun
import {
  machinesWithStatus
} from "./index-nnagv1cz.js";
import"./index-mcwdb471.js";
import {
  addEvent,
  createMachine,
  getSetting
} from "./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/machines/_server.ts.js
var GET = async () => {
  return json({ machines: await machinesWithStatus() });
};
var POST = async ({ request }) => {
  const body = await request.json();
  if (!body.name?.trim())
    return json({ error: "Name required" }, { status: 400 });
  const machine = createMachine({
    name: body.name.trim(),
    size: body.size || getSetting("size") || "c-8",
    region: body.region || getSetting("region") || "nyc3",
    snapshot_id: body.snapshotId || "",
    image: body.imageSlug || "",
    repo: body.repo || null,
    idle_timeout_min: body.idleTimeoutMin ?? 0,
    dedicated: body.dedicated ?? false,
    volume_gb: body.volumeGb ?? 0,
    ssh_key_ids: body.sshKeyIds ?? ""
  });
  addEvent(machine.id, "machine", `Profile "${machine.name}" created`);
  return json({ machine }, { status: 201 });
};
export {
  GET,
  POST
};

//# debugId=1F22B01EC29568CD64756E2164756E21
