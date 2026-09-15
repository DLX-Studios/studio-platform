import { Database } from "bun:sqlite";
import path from "node:path";
import { mkdirSync } from "node:fs";

const DATA_DIR = path.resolve(process.cwd(), "data");
mkdirSync(DATA_DIR, { recursive: true });

export const db = new Database(path.join(DATA_DIR, "studio.db"));
db.exec("PRAGMA journal_mode = WAL;");
db.exec("PRAGMA foreign_keys = ON;");

db.exec(`
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS machines (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  name             TEXT UNIQUE NOT NULL,
  size             TEXT NOT NULL,
  region           TEXT NOT NULL,
  snapshot_id      TEXT NOT NULL,
  repo             TEXT,
  color            TEXT,
  idle_timeout_min INTEGER NOT NULL DEFAULT 0,
  created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sessions (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  machine_id    INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
  droplet_id    INTEGER,
  size          TEXT,
  region        TEXT,
  started_at    TEXT NOT NULL,
  stopped_at    TEXT,
  end_reason    TEXT,
  cost_per_hour REAL
);

CREATE TABLE IF NOT EXISTS events (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  machine_id INTEGER REFERENCES machines(id) ON DELETE CASCADE,
  ts         TEXT NOT NULL DEFAULT (datetime('now')),
  kind       TEXT NOT NULL,
  detail     TEXT
);
`);

/* ---------- migrations ---------- */
const machineCols = db.query("PRAGMA table_info(machines)").all() as { name: string }[];
const hasCol = (c: string) => machineCols.some((x) => x.name === c);
if (!hasCol("dedicated")) db.exec("ALTER TABLE machines ADD COLUMN dedicated INTEGER NOT NULL DEFAULT 0");
if (!hasCol("volume_gb")) db.exec("ALTER TABLE machines ADD COLUMN volume_gb INTEGER NOT NULL DEFAULT 0");
if (!hasCol("image")) db.exec("ALTER TABLE machines ADD COLUMN image TEXT NOT NULL DEFAULT ''");
if (!hasCol("ssh_key_ids")) db.exec("ALTER TABLE machines ADD COLUMN ssh_key_ids TEXT NOT NULL DEFAULT ''");
if (!hasCol("daemon_ready")) db.exec("ALTER TABLE machines ADD COLUMN daemon_ready INTEGER NOT NULL DEFAULT 0");
if (!hasCol("daemon_token")) db.exec("ALTER TABLE machines ADD COLUMN daemon_token TEXT NOT NULL DEFAULT ''");

/* ---------- settings ---------- */

export function getSetting(key: string): string | null {
  const row = db
    .query<{ value: string }, [string]>("SELECT value FROM settings WHERE key = ?")
    .get(key);
  return row?.value ?? null;
}

export function setSetting(key: string, value: string): void {
  db.query(
    "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
  ).run(key, value);
}

export function isConfigured(): boolean {
  return getSetting("configured") === "1";
}

/* ---------- machines ---------- */

export interface Machine {
  id: number;
  name: string;
  size: string;
  region: string;
  snapshot_id: string;
  image: string;
  repo: string | null;
  color: string | null;
  idle_timeout_min: number;
  dedicated: number;
  volume_gb: number;
  ssh_key_ids: string;
  daemon_ready: number;
  daemon_token: string;
  created_at: string;
}

export function listMachines(): Machine[] {
  return db.query<Machine, []>("SELECT * FROM machines ORDER BY name").all();
}

export function getMachine(id: number): Machine | null {
  return db.query<Machine, [number]>("SELECT * FROM machines WHERE id = ?").get(id);
}

export function createMachine(m: {
  name: string;
  size: string;
  region: string;
  snapshot_id: string;
  image?: string;
  repo?: string | null;
  color?: string | null;
  idle_timeout_min?: number;
  dedicated?: boolean;
  volume_gb?: number;
  ssh_key_ids?: string;
}): Machine {
  const info = db
    .query(
      `INSERT INTO machines (name, size, region, snapshot_id, image, repo, color, idle_timeout_min, dedicated, volume_gb, ssh_key_ids)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      m.name,
      m.size,
      m.region,
      m.snapshot_id,
      m.image ?? "",
      m.repo ?? null,
      m.color ?? null,
      m.idle_timeout_min ?? 0,
      m.dedicated ? 1 : 0,
      m.volume_gb ?? 0,
      m.ssh_key_ids ?? "",
    );
  return getMachine(Number(info.lastInsertRowid))!;
}

export function updateMachine(
  id: number,
  fields: Partial<
    Pick<
      Machine,
      "name" | "size" | "region" | "snapshot_id" | "image" | "repo" | "color" | "idle_timeout_min" | "dedicated" | "volume_gb" | "ssh_key_ids" | "daemon_token"
    >
  >,
): Machine | null {
  const allowed = [
    "name",
    "size",
    "region",
    "snapshot_id",
    "image",
    "repo",
    "color",
    "idle_timeout_min",
    "dedicated",
    "volume_gb",
    "ssh_key_ids",
    "daemon_token",
  ] as const;
  const sets = allowed.filter((k) => fields[k] !== undefined);
  if (sets.length) {
    const sql = `UPDATE machines SET ${sets.map((k) => `${k} = ?`).join(", ")} WHERE id = ?`;
    db.query(sql).run(...sets.map((k) => fields[k]!), id);
  }
  return getMachine(id);
}

export function deleteMachine(id: number): void {
  db.query("DELETE FROM machines WHERE id = ?").run(id);
}

export function setDaemonReady(machineId: number, ready: boolean): void {
  db.query("UPDATE machines SET daemon_ready = ? WHERE id = ?").run(ready ? 1 : 0, machineId);
}

export function getMachineByDaemonToken(token: string): Machine | null {
  if (!token) return null;
  return db.query<Machine, [string]>("SELECT * FROM machines WHERE daemon_token = ?").get(token) ?? null;
}

export function machineTag(id: number): string {
  return `sb-machine-${id}`;
}

/* ---------- sessions ---------- */

export interface Session {
  id: number;
  machine_id: number;
  droplet_id: number | null;
  size: string | null;
  region: string | null;
  started_at: string;
  stopped_at: string | null;
  end_reason: string | null;
  cost_per_hour: number | null;
}

export function startSession(s: {
  machine_id: number;
  droplet_id: number;
  size: string;
  region: string;
  cost_per_hour: number;
}): number {
  const info = db
    .query(
      `INSERT INTO sessions (machine_id, droplet_id, size, region, started_at, cost_per_hour)
     VALUES (?, ?, ?, ?, datetime('now'), ?)`,
    )
    .run(s.machine_id, s.droplet_id, s.size, s.region, s.cost_per_hour);
  return Number(info.lastInsertRowid);
}

export function getOpenSession(machineId: number): Session | null {
  return (
    db
      .query<Session, [number]>(
        "SELECT * FROM sessions WHERE machine_id = ? AND stopped_at IS NULL ORDER BY id DESC LIMIT 1",
      )
      .get(machineId) ?? null
  );
}

export function endSession(dropletId: number, reason: string): void {
  db.query(
    `UPDATE sessions
     SET stopped_at = datetime('now'), end_reason = ?
     WHERE droplet_id = ? AND stopped_at IS NULL`,
  ).run(reason, dropletId);
}

export function listSessions(): Session[] {
  return db.query<Session, []>("SELECT * FROM sessions ORDER BY started_at DESC").all();
}

/* ---------- events ---------- */

export function addEvent(machineId: number | null, kind: string, detail: string): void {
  db.query("INSERT INTO events (machine_id, kind, detail) VALUES (?, ?, ?)").run(
    machineId,
    kind,
    detail,
  );
}
