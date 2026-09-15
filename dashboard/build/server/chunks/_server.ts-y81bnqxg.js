// @bun
import {
  describeDoError,
  listDistributions,
  listDropletSnapshots,
  listRegions,
  listSizes,
  listSshKeys
} from "./index-mcwdb471.js";
import {
  getSetting
} from "./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/do/options/_server.ts.js
var GET = async () => {
  try {
    const [regions, sizes, snapshots, sshKeys, distributions] = await Promise.all([
      listRegions(),
      listSizes(),
      listDropletSnapshots(),
      listSshKeys(),
      listDistributions()
    ]);
    return json({
      regions,
      sizes,
      snapshots,
      sshKeys,
      distributions,
      openrouter_configured: !!getSetting("openrouter_key")
    });
  } catch (e) {
    const d = describeDoError(e);
    return json({ error: d.message }, { status: 502 });
  }
};
export {
  GET
};

//# debugId=FA91E17FC4FE4BEC64756E2164756E21
