// @bun
import {
  Client
} from "./index-z8d1ece1.js";
import {
  machineWithStatus
} from "./index-nnagv1cz.js";
import {
  createDroplet,
  createDropletSnapshot,
  createVolume,
  deleteDroplet,
  deleteVolume,
  detachVolume,
  listDropletsByTag,
  listVolumesByName,
  sshKeyIds
} from "./index-mcwdb471.js";
import {
  addEvent,
  deleteMachine,
  endSession,
  getMachine,
  getOpenSession,
  getSetting,
  machineTag,
  setDaemonReady,
  startSession,
  updateMachine
} from "./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/machines/_id_/_server.ts.js
var cloud_init_template_default = `#cloud-config
package_update: true
packages:
  - git
  - curl
  - ca-certificates
  - fish
  - build-essential
  - pkg-config
  - libssl-dev
  - mesa-vulkan-drivers
  - vulkan-tools
write_files:
  - path: /opt/studio-bootstrap.sh
    permissions: "0755"
    content: |
      #!/bin/bash
      set -e

      LOG="/var/log/studio-bootstrap.log"
      exec > >(tee -a "$LOG") 2>&1

      echo "[studio] Bootstrapping at $(date)"

      # --- fastfetch (not in Ubuntu 22.04 repos; GitHub .deb fallback) ---
      if ! command -v fastfetch >/dev/null 2>&1; then
        echo "[studio] Installing fastfetch from GitHub release..."
        curl -fsSL -o /tmp/fastfetch.deb \\
          https://github.com/fastfetch-cli/fastfetch/releases/latest/download/fastfetch-linux-amd64.deb \\
          || echo "[studio] fastfetch download failed (non-fatal)"
        dpkg -i /tmp/fastfetch.deb >/dev/null 2>&1 \\
          || apt-get install -f -y >/dev/null 2>&1 \\
          || echo "[studio] fastfetch install failed (non-fatal)"
        rm -f /tmp/fastfetch.deb
      fi

      # --- cage (Wayland kiosk compositor for the app-stream viewer) ---
      # Requires 24.04+; skipped gracefully elsewhere (viewer is Phase 3).
      if ! command -v cage >/dev/null 2>&1; then
        apt-get install -y cage >/dev/null 2>&1 || echo "[studio] cage unavailable on this distro (non-fatal)"
      fi

      # --- rustup (stable, minimal profile) ---
      if ! command -v rustc >/dev/null 2>&1; then
        echo "[studio] Installing rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \\
          | sh -s -- -y --default-toolchain stable --profile minimal
      fi
      # Expose cargo/rustc system-wide (symlink rustup proxies)
      ln -sf /root/.cargo/bin/* /usr/local/bin/
      rustc --version || echo "[studio] rustc missing (non-fatal)"

      # --- sccache (pinned release binary -- much faster than cargo install) ---
      SCCACHE_VERSION=v0.9.1
      if ! command -v sccache >/dev/null 2>&1; then
        echo "[studio] Installing sccache \${SCCACHE_VERSION}..."
        curl -fsSL -o /tmp/sccache.tar.gz \\
          "https://github.com/mozilla/sccache/releases/download/\${SCCACHE_VERSION}/sccache-\${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz" \\
          || echo "[studio] sccache download failed (non-fatal)"
        tar -xzf /tmp/sccache.tar.gz -C /tmp \\
          && install -m 0755 "/tmp/sccache-\${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache" /usr/local/bin/sccache \\
          || echo "[studio] sccache install failed (non-fatal)"
        rm -rf /tmp/sccache.tar.gz /tmp/sccache-\${SCCACHE_VERSION}-x86_64-unknown-linux-musl
      fi

      # --- Pi coding agent (the daemon keeps it alive in RPC mode) ---
      if ! command -v node >/dev/null 2>&1; then
        echo "[studio] Installing Node.js for Pi..."
        curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
        apt-get install -y nodejs
      fi
      if ! command -v pi >/dev/null 2>&1 && [ ! -x /root/.local/bin/pi ]; then
        echo "[studio] Installing Pi coding agent..."
        curl -fsSL https://pi.dev/install.sh | bash \\
          || echo "[studio] Pi install failed (agent chat will be unavailable)"
      fi
      export PATH="/root/.local/bin:/root/.cargo/bin:$PATH"
      pi --version || /root/.local/bin/pi --version || true

      # --- studio daemon (binary from the assets bucket, presigned URL) ---
      if [ -n "__DAEMON_BINARY_URL__" ]; then
        echo "[studio] Installing sb-daemon..."
        curl -fsSL -o /usr/local/bin/sb-daemon "__DAEMON_BINARY_URL__" \\
          && chmod 0755 /usr/local/bin/sb-daemon \\
          && systemctl daemon-reload \\
          && systemctl enable --now sb-daemon \\
          || echo "[studio] daemon install failed (non-fatal)"
        /usr/local/bin/sb-daemon --version 2>/dev/null || true
      else
        echo "[studio] No daemon binary URL -- skipping daemon install"
      fi

      # --- fish as login shell for root ---
      chsh -s "$(command -v fish)" root || echo "[studio] chsh failed (non-fatal)"

      # --- workspace + repo ---
      mkdir -p /root/workspace
      cd /root/workspace
      if [ -n "__REPO_URL__" ]; then
        if [ ! -d "studio" ]; then
          echo "[studio] Cloning repo..."
          git clone "__REPO_URL__" studio
        else
          echo "[studio] Pulling latest changes..."
          cd studio && git pull
        fi
      else
        echo "[studio] No repo configured -- skipping clone"
      fi

      # --- sccache environment ---
      echo "[studio] Configuring sccache environment..."
      {
        echo "SCCACHE_BUCKET=__SCCACHE_BUCKET__"
        echo "SCCACHE_REGION=__SCCACHE_REGION__"
        echo "SCCACHE_ENDPOINT=__SCCACHE_ENDPOINT__"
        echo "AWS_ACCESS_KEY_ID=__AWS_ACCESS_KEY_ID__"
        echo "AWS_SECRET_ACCESS_KEY=__AWS_SECRET_ACCESS_KEY__"
        echo "OPENROUTER_API_KEY=__OPENROUTER_API_KEY__"
        echo "SB_DAEMON_URL=__DAEMON_URL__"
        echo "SB_DAEMON_TOKEN=__DAEMON_TOKEN__"
        echo "RUSTC_WRAPPER=sccache"
        echo "SCCACHE_S3_USE_SSL=true"
        echo "CARGO_INCREMENTAL=1"
      } >> /etc/environment

      export SCCACHE_BUCKET=__SCCACHE_BUCKET__
      export SCCACHE_REGION=__SCCACHE_REGION__
      export SCCACHE_ENDPOINT=__SCCACHE_ENDPOINT__
      export AWS_ACCESS_KEY_ID=__AWS_ACCESS_KEY_ID__
      export AWS_SECRET_ACCESS_KEY=__AWS_SECRET_ACCESS_KEY__
      export OPENROUTER_API_KEY=__OPENROUTER_API_KEY__
      export SB_DAEMON_URL=__DAEMON_URL__
      export SB_DAEMON_TOKEN=__DAEMON_TOKEN__
      export RUSTC_WRAPPER=sccache
      export SCCACHE_S3_USE_SSL=true

      echo "[studio] Starting sccache server..."
      sccache --start-server || true
      sccache --show-stats || true

      echo "[studio] Bootstrap complete at $(date)"

  - path: /etc/systemd/system/sb-daemon.service
    permissions: "0644"
    content: |
      [Unit]
      Description=Studio Builder daemon
      After=network-online.target
      Wants=network-online.target

      [Service]
      EnvironmentFile=/etc/sb-daemon.env
      ExecStart=/usr/local/bin/sb-daemon
      Restart=on-failure
      RestartSec=5

      [Install]
      WantedBy=multi-user.target
  - path: /etc/sb-daemon.env
    permissions: "0600"
    content: |
      SB_DAEMON_URL=__DAEMON_URL__
      SB_DAEMON_TOKEN=__DAEMON_TOKEN__
      SB_WORKSPACE=/root/workspace/studio
      SB_PI_SESSION_DIR=/root/.studio-builder/pi-sessions
      HOME=/root
      PATH=/root/.local/bin:/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
      OPENROUTER_API_KEY=__OPENROUTER_API_KEY__
  - path: /root/.config/fish/config.fish
    permissions: "0644"
    content: |
      # Cargo (rustup) on PATH
      fish_add_path -U $HOME/.cargo/bin
      # Greet with system info
      fastfetch
runcmd:
  - /opt/studio-bootstrap.sh
`;
var DAEMON_BINARY_KEY = "daemon/sb-daemon";
var DAEMON_URL_EXPIRY_SECONDS = 86400;
function assetsConfig() {
  return {
    bucket: getSetting("assets_bucket") ?? "studio-assets",
    region: getSetting("assets_region") ?? "atl1",
    endpoint: getSetting("assets_endpoint") ?? "atl1.digitaloceanspaces.com"
  };
}
function client() {
  const { bucket, region, endpoint } = assetsConfig();
  return new Client({
    endPoint: endpoint,
    region,
    bucket,
    accessKey: getSetting("spaces_key_id") ?? "",
    secretKey: getSetting("spaces_secret") ?? "",
    pathStyle: false
  });
}
async function presignDaemonBinaryUrl() {
  return client().getPresignedUrl("GET", DAEMON_BINARY_KEY, { expirySeconds: DAEMON_URL_EXPIRY_SECONDS });
}
async function ensureVolume(machine) {
  const name = `${machineTag(machine.id)}-data`;
  const match = (await listVolumesByName(name)).find((v) => v.region === machine.region);
  if (match) {
    for (const dropletId of match.dropletIds)
      await detachVolume(String(match.id), dropletId).catch(() => {});
    return match.id;
  }
  return (await createVolume({
    name,
    region: machine.region,
    sizeGb: machine.volume_gb
  })).id;
}
async function deleteVolumeByName(tag) {
  const name = `${tag}-data`;
  const vols = await listVolumesByName(name);
  for (const v of vols)
    try {
      await deleteVolume(String(v.id));
      addEvent(null, "destroy", `Volume ${name} deleted`);
    } catch {}
}
function buildUserData(machine, daemonUrl, daemonToken, daemonBinaryUrl) {
  let repoUrl = machine?.repo || getSetting("repo_url") || "";
  const gitToken = getSetting("git_token") ?? "";
  if (gitToken && repoUrl.startsWith("https://") && !repoUrl.includes("@"))
    repoUrl = repoUrl.replace(/^https:\/\//, "https://oauth2:" + gitToken + "@");
  return cloud_init_template_default.replace(/__REPO_URL__/g, repoUrl).replace(/__DAEMON_URL__/g, daemonUrl).replace(/__DAEMON_TOKEN__/g, daemonToken).replace(/__DAEMON_BINARY_URL__/g, daemonBinaryUrl).replace(/__SCCACHE_BUCKET__/g, getSetting("sccache_bucket") ?? "").replace(/__SCCACHE_REGION__/g, getSetting("sccache_region") ?? "nyc3").replace(/__SCCACHE_ENDPOINT__/g, getSetting("sccache_endpoint") ?? "nyc3.digitaloceanspaces.com").replace(/__AWS_ACCESS_KEY_ID__/g, getSetting("spaces_key_id") ?? "").replace(/__AWS_SECRET_ACCESS_KEY__/g, getSetting("spaces_secret") ?? "").replace(/__OPENROUTER_API_KEY__/g, getSetting("openrouter_key") ?? "");
}
var GET = async ({ params }) => {
  return json(await machineWithStatus(Number(params.id)));
};
var POST = async ({ params, request }) => {
  const id = Number(params.id);
  const machine = getMachine(id);
  if (!machine)
    return json({ error: "Not found" }, { status: 404 });
  const { action } = await request.json();
  const tag = machineTag(id);
  const daemonUrl = new URL(request.url).origin;
  if (action === "start")
    try {
      const orphan = (await listDropletsByTag(tag)).droplets[0];
      if (orphan && (orphan.status === "new" || orphan.status === "active")) {
        setDaemonReady(id, false);
        if (!getOpenSession(id)) {
          startSession({
            machine_id: id,
            droplet_id: orphan.id,
            size: machine.size,
            region: machine.region,
            cost_per_hour: parseFloat(getSetting("cost_per_hour") ?? "0.143")
          });
          addEvent(id, "create", `Adopted orphan droplet ${orphan.id} (recovered session)`);
        }
        return json({
          ok: true,
          droplet_id: orphan.id,
          adopted: true
        });
      }
      const image = machine.snapshot_id || machine.image || getSetting("snapshot_id") || "";
      if (!image)
        return json({ error: "This machine has no image. Pick a distribution or golden snapshot in the machine settings (gear icon) \u2014 or build one: start the machine from a plain distribution, set it up, then take a snapshot." }, { status: 400 });
      const sshKeys = machine.ssh_key_ids ? machine.ssh_key_ids.split(",").filter(Boolean).map(Number) : sshKeyIds();
      if (sshKeys.length === 0)
        return json({ error: "No SSH keys selected for this machine \u2014 add or pick one in the machine settings." }, { status: 400 });
      const daemonToken = machine.daemon_token || crypto.randomUUID();
      updateMachine(id, { daemon_token: daemonToken });
      const daemonBinaryUrl = await presignDaemonBinaryUrl().catch((e) => {
        addEvent(id, "daemon", `Presign failed: ${e.message}`);
        return "";
      });
      const result = await createDroplet({
        name: machine.name,
        region: machine.region,
        size: machine.size,
        image,
        ssh_keys: sshKeys,
        tags: [tag],
        user_data: buildUserData(machine, daemonUrl, daemonToken, daemonBinaryUrl),
        dedicatedCpu: machine.dedicated === 1,
        volumes: machine.volume_gb > 0 ? [await ensureVolume(machine)] : []
      });
      setDaemonReady(id, false);
      startSession({
        machine_id: id,
        droplet_id: result.droplet.id,
        size: machine.size,
        region: machine.region,
        cost_per_hour: parseFloat(getSetting("cost_per_hour") ?? "0.143")
      });
      addEvent(id, "create", `Droplet ${result.droplet.id} launched`);
      return json({
        ok: true,
        droplet_id: result.droplet.id
      });
    } catch (e) {
      return json({ error: e.message }, { status: 502 });
    }
  if (action === "snapshot")
    try {
      const droplet = (await listDropletsByTag(tag)).droplets[0];
      if (!droplet)
        return json({ error: "No running droplet to snapshot \u2014 start the machine first." }, { status: 404 });
      const name = `sb-golden-${machine.name}-${(/* @__PURE__ */ new Date()).toISOString().slice(0, 10)}`;
      const r = await createDropletSnapshot(droplet.id, name);
      addEvent(id, "snapshot", `Snapshot "${name}" queued (action ${r.actionId})`);
      return json({
        ok: true,
        name,
        action_id: r.actionId
      });
    } catch (e) {
      return json({ error: e.message }, { status: 502 });
    }
  if (action === "stop")
    try {
      const droplet = (await listDropletsByTag(tag)).droplets[0];
      if (!droplet)
        return json({ error: "No running droplet" }, { status: 404 });
      await deleteDroplet(droplet.id);
      endSession(droplet.id, "manual");
      setDaemonReady(id, false);
      addEvent(id, "destroy", `Droplet ${droplet.id} destroyed`);
      return json({ ok: true });
    } catch (e) {
      return json({ error: e.message }, { status: 502 });
    }
  if (action === "update") {
    const body = await request.json().catch(() => ({}));
    const updated = updateMachine(id, body);
    return json({ machine: updated });
  }
  if (action === "delete") {
    const data = await listDropletsByTag(tag);
    if (data.droplets[0])
      await deleteDroplet(data.droplets[0].id).catch(() => {});
    try {
      await deleteVolumeByName(tag);
    } catch (e) {
      addEvent(id, "destroy", `Volume cleanup failed: ${e.message}`);
    }
    deleteMachine(id);
    addEvent(null, "machine", `Profile "${machine.name}" deleted`);
    return json({ ok: true });
  }
  return json({ error: "Unknown action" }, { status: 400 });
};
export {
  GET,
  POST
};

//# debugId=67643BA21F5E305064756E2164756E21
