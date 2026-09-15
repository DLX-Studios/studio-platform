import { a as getMachine, c as getSetting, f as machineTag, g as updateMachine, h as startSession, i as endSession, p as setDaemonReady, r as deleteMachine, s as getOpenSession, t as addEvent } from "../../../../../chunks/db.js";
import { _ as listVolumesByName, a as deleteDroplet, c as detachVolume, i as createVolume, n as createDropletSnapshot, o as deleteVolume, p as listDropletsByTag, t as createDroplet, v as sshKeyIds } from "../../../../../chunks/doClient.js";
import { t as machineWithStatus } from "../../../../../chunks/machinesStatus.js";
import { json } from "@sveltejs/kit";
import { S3Client } from "@bradenmacdonald/s3-lite-client";
//#region src/lib/cloud-init-template.yml?raw
var cloud_init_template_default = "#cloud-config\npackage_update: true\npackages:\n  - git\n  - curl\n  - ca-certificates\n  - fish\n  - build-essential\n  - pkg-config\n  - libssl-dev\n  - mesa-vulkan-drivers\n  - vulkan-tools\nwrite_files:\n  - path: /opt/studio-bootstrap.sh\n    permissions: \"0755\"\n    content: |\n      #!/bin/bash\n      set -e\n\n      LOG=\"/var/log/studio-bootstrap.log\"\n      exec > >(tee -a \"$LOG\") 2>&1\n\n      echo \"[studio] Bootstrapping at $(date)\"\n\n      # --- fastfetch (not in Ubuntu 22.04 repos; GitHub .deb fallback) ---\n      if ! command -v fastfetch >/dev/null 2>&1; then\n        echo \"[studio] Installing fastfetch from GitHub release...\"\n        curl -fsSL -o /tmp/fastfetch.deb \\\n          https://github.com/fastfetch-cli/fastfetch/releases/latest/download/fastfetch-linux-amd64.deb \\\n          || echo \"[studio] fastfetch download failed (non-fatal)\"\n        dpkg -i /tmp/fastfetch.deb >/dev/null 2>&1 \\\n          || apt-get install -f -y >/dev/null 2>&1 \\\n          || echo \"[studio] fastfetch install failed (non-fatal)\"\n        rm -f /tmp/fastfetch.deb\n      fi\n\n      # --- cage (Wayland kiosk compositor for the app-stream viewer) ---\n      # Requires 24.04+; skipped gracefully elsewhere (viewer is Phase 3).\n      if ! command -v cage >/dev/null 2>&1; then\n        apt-get install -y cage >/dev/null 2>&1 || echo \"[studio] cage unavailable on this distro (non-fatal)\"\n      fi\n\n      # --- rustup (stable, minimal profile) ---\n      if ! command -v rustc >/dev/null 2>&1; then\n        echo \"[studio] Installing rustup...\"\n        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \\\n          | sh -s -- -y --default-toolchain stable --profile minimal\n      fi\n      # Expose cargo/rustc system-wide (symlink rustup proxies)\n      ln -sf /root/.cargo/bin/* /usr/local/bin/\n      rustc --version || echo \"[studio] rustc missing (non-fatal)\"\n\n      # --- sccache (pinned release binary -- much faster than cargo install) ---\n      SCCACHE_VERSION=v0.9.1\n      if ! command -v sccache >/dev/null 2>&1; then\n        echo \"[studio] Installing sccache ${SCCACHE_VERSION}...\"\n        curl -fsSL -o /tmp/sccache.tar.gz \\\n          \"https://github.com/mozilla/sccache/releases/download/${SCCACHE_VERSION}/sccache-${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz\" \\\n          || echo \"[studio] sccache download failed (non-fatal)\"\n        tar -xzf /tmp/sccache.tar.gz -C /tmp \\\n          && install -m 0755 \"/tmp/sccache-${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache\" /usr/local/bin/sccache \\\n          || echo \"[studio] sccache install failed (non-fatal)\"\n        rm -rf /tmp/sccache.tar.gz /tmp/sccache-${SCCACHE_VERSION}-x86_64-unknown-linux-musl\n      fi\n\n      # --- Pi coding agent (the daemon keeps it alive in RPC mode) ---\n      if ! command -v node >/dev/null 2>&1; then\n        echo \"[studio] Installing Node.js for Pi...\"\n        curl -fsSL https://deb.nodesource.com/setup_22.x | bash -\n        apt-get install -y nodejs\n      fi\n      if ! command -v pi >/dev/null 2>&1 && [ ! -x /root/.local/bin/pi ]; then\n        echo \"[studio] Installing Pi coding agent...\"\n        curl -fsSL https://pi.dev/install.sh | bash \\\n          || echo \"[studio] Pi install failed (agent chat will be unavailable)\"\n      fi\n      export PATH=\"/root/.local/bin:/root/.cargo/bin:$PATH\"\n      pi --version || /root/.local/bin/pi --version || true\n\n      # --- studio daemon (binary from the assets bucket, presigned URL) ---\n      if [ -n \"__DAEMON_BINARY_URL__\" ]; then\n        echo \"[studio] Installing sb-daemon...\"\n        curl -fsSL -o /usr/local/bin/sb-daemon \"__DAEMON_BINARY_URL__\" \\\n          && chmod 0755 /usr/local/bin/sb-daemon \\\n          && systemctl daemon-reload \\\n          && systemctl enable --now sb-daemon \\\n          || echo \"[studio] daemon install failed (non-fatal)\"\n        /usr/local/bin/sb-daemon --version 2>/dev/null || true\n      else\n        echo \"[studio] No daemon binary URL -- skipping daemon install\"\n      fi\n\n      # --- fish as login shell for root ---\n      chsh -s \"$(command -v fish)\" root || echo \"[studio] chsh failed (non-fatal)\"\n\n      # --- workspace + repo ---\n      mkdir -p /root/workspace\n      cd /root/workspace\n      if [ -n \"__REPO_URL__\" ]; then\n        if [ ! -d \"studio\" ]; then\n          echo \"[studio] Cloning repo...\"\n          git clone \"__REPO_URL__\" studio\n        else\n          echo \"[studio] Pulling latest changes...\"\n          cd studio && git pull\n        fi\n      else\n        echo \"[studio] No repo configured -- skipping clone\"\n      fi\n\n      # --- sccache environment ---\n      echo \"[studio] Configuring sccache environment...\"\n      {\n        echo \"SCCACHE_BUCKET=__SCCACHE_BUCKET__\"\n        echo \"SCCACHE_REGION=__SCCACHE_REGION__\"\n        echo \"SCCACHE_ENDPOINT=__SCCACHE_ENDPOINT__\"\n        echo \"AWS_ACCESS_KEY_ID=__AWS_ACCESS_KEY_ID__\"\n        echo \"AWS_SECRET_ACCESS_KEY=__AWS_SECRET_ACCESS_KEY__\"\n        echo \"OPENROUTER_API_KEY=__OPENROUTER_API_KEY__\"\n        echo \"SB_DAEMON_URL=__DAEMON_URL__\"\n        echo \"SB_DAEMON_TOKEN=__DAEMON_TOKEN__\"\n        echo \"RUSTC_WRAPPER=sccache\"\n        echo \"SCCACHE_S3_USE_SSL=true\"\n        echo \"CARGO_INCREMENTAL=1\"\n      } >> /etc/environment\n\n      export SCCACHE_BUCKET=__SCCACHE_BUCKET__\n      export SCCACHE_REGION=__SCCACHE_REGION__\n      export SCCACHE_ENDPOINT=__SCCACHE_ENDPOINT__\n      export AWS_ACCESS_KEY_ID=__AWS_ACCESS_KEY_ID__\n      export AWS_SECRET_ACCESS_KEY=__AWS_SECRET_ACCESS_KEY__\n      export OPENROUTER_API_KEY=__OPENROUTER_API_KEY__\n      export SB_DAEMON_URL=__DAEMON_URL__\n      export SB_DAEMON_TOKEN=__DAEMON_TOKEN__\n      export RUSTC_WRAPPER=sccache\n      export SCCACHE_S3_USE_SSL=true\n\n      echo \"[studio] Starting sccache server...\"\n      sccache --start-server || true\n      sccache --show-stats || true\n\n      echo \"[studio] Bootstrap complete at $(date)\"\n\n  - path: /etc/systemd/system/sb-daemon.service\n    permissions: \"0644\"\n    content: |\n      [Unit]\n      Description=Studio Builder daemon\n      After=network-online.target\n      Wants=network-online.target\n\n      [Service]\n      EnvironmentFile=/etc/sb-daemon.env\n      ExecStart=/usr/local/bin/sb-daemon\n      Restart=on-failure\n      RestartSec=5\n\n      [Install]\n      WantedBy=multi-user.target\n  - path: /etc/sb-daemon.env\n    permissions: \"0600\"\n    content: |\n      SB_DAEMON_URL=__DAEMON_URL__\n      SB_DAEMON_TOKEN=__DAEMON_TOKEN__\n      SB_WORKSPACE=/root/workspace/studio\n      SB_PI_SESSION_DIR=/root/.studio-builder/pi-sessions\n      HOME=/root\n      PATH=/root/.local/bin:/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\n      OPENROUTER_API_KEY=__OPENROUTER_API_KEY__\n  - path: /root/.config/fish/config.fish\n    permissions: \"0644\"\n    content: |\n      # Cargo (rustup) on PATH\n      fish_add_path -U $HOME/.cargo/bin\n      # Greet with system info\n      fastfetch\nruncmd:\n  - /opt/studio-bootstrap.sh\n";
//#endregion
//#region src/lib/server/assets.ts
/**
* Assets bucket access — hosts the daemon binary (and future release
* artifacts). Separate from the sccache bucket: one bucket for build
* machines, one for distribution.
*
* Defaults live here; settings (assets_bucket / assets_region /
* assets_endpoint) can override. Uses the same account-wide Spaces keys
* as sccache.
*/
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
	return new S3Client({
		endPoint: endpoint,
		region,
		bucket,
		accessKey: getSetting("spaces_key_id") ?? "",
		secretKey: getSetting("spaces_secret") ?? "",
		pathStyle: false
	});
}
/** Short-lived signed GET URL for the daemon binary, safe to embed in user_data. */
async function presignDaemonBinaryUrl() {
	return client().getPresignedUrl("GET", DAEMON_BINARY_KEY, { expirySeconds: DAEMON_URL_EXPIRY_SECONDS });
}
//#endregion
//#region src/routes/api/machines/[id]/+server.ts
/**
* Additional storage: reuse the profile's persistent volume if it already
* exists (it survives destroy cycles), otherwise create it. The volume is
* attached at droplet creation so the machine always boots with it.
*/
async function ensureVolume(machine) {
	const name = `${machineTag(machine.id)}-data`;
	const match = (await listVolumesByName(name)).find((v) => v.region === machine.region);
	if (match) {
		for (const dropletId of match.dropletIds) await detachVolume(String(match.id), dropletId).catch(() => {});
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
	for (const v of vols) try {
		await deleteVolume(String(v.id));
		addEvent(null, "destroy", `Volume ${name} deleted`);
	} catch {}
}
function buildUserData(machine, daemonUrl, daemonToken, daemonBinaryUrl) {
	let repoUrl = machine?.repo || getSetting("repo_url") || "";
	const gitToken = getSetting("git_token") ?? "";
	if (gitToken && repoUrl.startsWith("https://") && !repoUrl.includes("@")) repoUrl = repoUrl.replace(/^https:\/\//, "https://oauth2:" + gitToken + "@");
	return cloud_init_template_default.replace(/__REPO_URL__/g, repoUrl).replace(/__DAEMON_URL__/g, daemonUrl).replace(/__DAEMON_TOKEN__/g, daemonToken).replace(/__DAEMON_BINARY_URL__/g, daemonBinaryUrl).replace(/__SCCACHE_BUCKET__/g, getSetting("sccache_bucket") ?? "").replace(/__SCCACHE_REGION__/g, getSetting("sccache_region") ?? "nyc3").replace(/__SCCACHE_ENDPOINT__/g, getSetting("sccache_endpoint") ?? "nyc3.digitaloceanspaces.com").replace(/__AWS_ACCESS_KEY_ID__/g, getSetting("spaces_key_id") ?? "").replace(/__AWS_SECRET_ACCESS_KEY__/g, getSetting("spaces_secret") ?? "").replace(/__OPENROUTER_API_KEY__/g, getSetting("openrouter_key") ?? "");
}
var GET = async ({ params }) => {
	return json(await machineWithStatus(Number(params.id)));
};
var POST = async ({ params, request }) => {
	const id = Number(params.id);
	const machine = getMachine(id);
	if (!machine) return json({ error: "Not found" }, { status: 404 });
	const { action } = await request.json();
	const tag = machineTag(id);
	const daemonUrl = new URL(request.url).origin;
	if (action === "start") try {
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
		if (!image) return json({ error: "This machine has no image. Pick a distribution or golden snapshot in the machine settings (gear icon) — or build one: start the machine from a plain distribution, set it up, then take a snapshot." }, { status: 400 });
		const sshKeys = machine.ssh_key_ids ? machine.ssh_key_ids.split(",").filter(Boolean).map(Number) : sshKeyIds();
		if (sshKeys.length === 0) return json({ error: "No SSH keys selected for this machine — add or pick one in the machine settings." }, { status: 400 });
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
	if (action === "snapshot") try {
		const droplet = (await listDropletsByTag(tag)).droplets[0];
		if (!droplet) return json({ error: "No running droplet to snapshot — start the machine first." }, { status: 404 });
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
	if (action === "stop") try {
		const droplet = (await listDropletsByTag(tag)).droplets[0];
		if (!droplet) return json({ error: "No running droplet" }, { status: 404 });
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
		if (data.droplets[0]) await deleteDroplet(data.droplets[0].id).catch(() => {});
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
//#endregion
export { GET, POST };

//# sourceMappingURL=_server.ts.js.map