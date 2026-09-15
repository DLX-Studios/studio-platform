# Studio Builder

One-click DigitalOcean build machine for Studio Canvas. Start a powerful Rust builder from a snapshot, work, then nuke it. All compilation state persists in Spaces via `sccache`.

## Directory Layout

```
studio-builder/
├── .env                 # Secrets (not committed)
├── scripts/
│   ├── setup-builder.sh # Run once inside a DO droplet to create your golden snapshot
│   └── dev-server       # Optional CLI (uses doctl)
└── dashboard/           # SvelteKit controller UI
    └── ...
```

## Step 1: Create Your Golden Snapshot

1. Create a vanilla Ubuntu 22.04 droplet on DigitalOcean (smallest size is fine for this).
2. SSH in and copy `scripts/setup-builder.sh`:

   ```bash
   scp scripts/setup-builder.sh root@<ip>:/root/
   ssh root@<ip>
   chmod +x setup-builder.sh && ./setup-builder.sh
   ```

The setup script installs the Rust toolchain, sccache, Vulkan/lib stuff — plus **Bun** (no Node/npm anywhere) and the **Vite+ (`vp`) unified toolchain** for running the SvelteKit dashboard. It then configures cargo's sparse registry + rustc-wrapper through sccache.
4. **Power off** the droplet: `poweroff`
5. In the DigitalOcean dashboard, **Take a Snapshot** of the powered-off droplet.
6. Note the **Snapshot ID** (visible in Images > Snapshots, or via `doctl compute image list --public false`).

## Step 2: Create a Spaces Bucket for sccache

1. In DO, create a **Spaces** bucket (e.g. `studio-cache`).
2. Create Spaces access keys.
3. The droplet will export these as `AWS_ACCESS_KEY_ID` / etc. so `sccache` treats Spaces like S3.

## Step 3: Add Your SSH Key to DO

1. Add your laptop's SSH key in DO account settings.
2. Note the **Key ID** number (visible in the URL or via `doctl compute ssh-key list`).

## Step 4: Configure Environment

```bash
cp .env.example .env
# Edit .env with your tokens, snapshot ID, key IDs, Spaces creds, GitHub PAT
```

**GitHub Token:** Use a fine-grained PAT with **Contents (read)** on your private repo.

**Cost per hour:** Adjust `COST_PER_HOUR` to match your default DO size pricing (used only for dashboard analytics).

## Step 5: Run the Dashboard (Bun + Vite+)

Install Bun once (it also powers the **SvelteKit** server, via the `adapter-bun` adapter):

```bash
curl -fsSL https://bun.sh/install | bash
curl -fsSL https://viteplus.dev/install.sh | bash
cd dashboard
bun install
bun dev   # runs `vp dev`
```

Open http://localhost:3000

### Build and run permanently (single-file Bun server)

```bash
cd dashboard
bun install
bun run node_modules/.bin/svelte-kit sync   # warm SvelteKit's $app/tsconfig once
bun --bun run build                         # adapter-bun output
bun run start                               # adapter-bun runtime
```

The `adapter-bun` runtime serves your client assets/prerendered output through Bun's native route/File system, with automatic ETag-based 304s and pre-compressed (`.br`/`.gz`) files — **production-hardened**, no nginx.

## Usage

### From the Svelte Dashboard

1. Click **Start Builder** → DO creates droplet from snapshot, cloud-init clones repo & starts sccache.
2. Wait ~60s for provisioning.
3. Copy the **SSH command** shown on the dashboard.
4. Work on your project. Because sccache writes to Spaces, you can destroy the machine without losing build artifacts.
5. Click **Stop & Destroy** when done. Analytics are saved locally.

### From CLI (optional)

If you also install `doctl` locally:

```bash
export DO_SNAPSHOT_ID=12345678
export DO_SSH_KEY_IDS=12345678
./scripts/dev-server up
./scripts/dev-server ssh
./scripts/dev-server down
```

## How It Works

| Component | Purpose |
|-----------|---------|
| **Snapshot** | Fast boots with all toolchains pre-installed |
| **cloud-init** | Clones/pulls repo + configures sccache env on every boot |
| **Spaces + sccache** | Persistent distributed cache; survive droplet destruction |
| **Dashboard** | Local SvelteKit app proxies DO API, tracks cost/session history, and streams agent chat |
| **sb-daemon** | Authenticated probe and persistent Pi RPC/SSE bridge on each builder |

The machine workspace includes a persistent, streamed Pi chat. The daemon
keeps one RPC-mode Pi process alive, resumes its saved session after restart,
and relays structured message/tool events through the authenticated dashboard.
See [the agent chat architecture](docs/Plan-02-pi-agent-chat.md).

## Security Notes

- The GitHub token is passed via cloud-init `user_data`. It is visible inside the droplet at `/var/lib/cloud/instance/user-data.txt`. This is acceptable because the droplet is **ephemeral** and the token should be a restricted PAT.
- Your DO token stays on your local machine; the dashboard server-side routes keep it secret from the browser.
- The dashboard stores a local `data/ledger.jsonl` file for analytics.
