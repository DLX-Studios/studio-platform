// @bun
import {
  describeDoError,
  getAccount,
  listDropletSnapshots,
  listSshKeys
} from "./index-mcwdb471.js";
import"./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/setup/probe/_server.ts.js
var POST = async ({ request }) => {
  const { token } = await request.json();
  if (!token)
    return json({
      error: "Token required",
      reason: "rejected"
    }, { status: 400 });
  try {
    await getAccount(token);
  } catch (e) {
    const d = describeDoError(e);
    return json({
      error: d.message,
      reason: d.reason
    }, { status: 400 });
  }
  const keys = await listSshKeys(token).catch(() => []);
  const snapshots = await listDropletSnapshots(token).catch(() => []);
  return json({
    ok: true,
    keys,
    snapshots
  });
};
export {
  POST
};

//# debugId=E759FB0D7788224A64756E2164756E21
