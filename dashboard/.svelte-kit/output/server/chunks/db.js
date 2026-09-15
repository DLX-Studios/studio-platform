import { Database } from "bun:sqlite";
import path from "node:path";
import { mkdirSync } from "node:fs";
//#region src/lib/server/db.ts
var DATA_DIR = path.resolve(process.cwd(), "data");
mkdirSync(DATA_DIR, { recursive: true });
var db = new Database(path.join(DATA_DIR, "studio.db"));
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
var machineCols = db.query("PRAGMA table_info(machines)").all();
var hasCol = (c) => machineCols.some((x) => x.name === c);
if (!hasCol("dedicated")) db.exec("ALTER TABLE machines ADD COLUMN dedicated INTEGER NOT NULL DEFAULT 0");
if (!hasCol("volume_gb")) db.exec("ALTER TABLE machines ADD COLUMN volume_gb INTEGER NOT NULL DEFAULT 0");
if (!hasCol("image")) db.exec("ALTER TABLE machines ADD COLUMN image TEXT NOT NULL DEFAULT ''");
if (!hasCol("ssh_key_ids")) db.exec("ALTER TABLE machines ADD COLUMN ssh_key_ids TEXT NOT NULL DEFAULT ''");
if (!hasCol("daemon_ready")) db.exec("ALTER TABLE machines ADD COLUMN daemon_ready INTEGER NOT NULL DEFAULT 0");
if (!hasCol("daemon_token")) db.exec("ALTER TABLE machines ADD COLUMN daemon_token TEXT NOT NULL DEFAULT ''");
function getSetting(key) {
	return db.query("SELECT value FROM settings WHERE key = ?").get(key)?.value ?? null;
}
function setSetting(key, value) {
	db.query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(key, value);
}
function isConfigured() {
	return getSetting("configured") === "1";
}
function listMachines() {
	return db.query("SELECT * FROM machines ORDER BY name").all();
}
function getMachine(id) {
	return db.query("SELECT * FROM machines WHERE id = ?").get(id);
}
function createMachine(m) {
	const info = db.query(`INSERT INTO machines (name, size, region, snapshot_id, image, repo, color, idle_timeout_min, dedicated, volume_gb, ssh_key_ids)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`).run(m.name, m.size, m.region, m.snapshot_id, m.image ?? "", m.repo ?? null, m.color ?? null, m.idle_timeout_min ?? 0, m.dedicated ? 1 : 0, m.volume_gb ?? 0, m.ssh_key_ids ?? "");
	return getMachine(Number(info.lastInsertRowid));
}
function updateMachine(id, fields) {
	const sets = [
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
		"daemon_token"
	].filter((k) => fields[k] !== void 0);
	if (sets.length) {
		const sql = `UPDATE machines SET ${sets.map((k) => `${k} = ?`).join(", ")} WHERE id = ?`;
		db.query(sql).run(...sets.map((k) => fields[k]), id);
	}
	return getMachine(id);
}
function deleteMachine(id) {
	db.query("DELETE FROM machines WHERE id = ?").run(id);
}
function setDaemonReady(machineId, ready) {
	db.query("UPDATE machines SET daemon_ready = ? WHERE id = ?").run(ready ? 1 : 0, machineId);
}
function getMachineByDaemonToken(token) {
	if (!token) return null;
	return db.query("SELECT * FROM machines WHERE daemon_token = ?").get(token) ?? null;
}
function machineTag(id) {
	return `sb-machine-${id}`;
}
function startSession(s) {
	const info = db.query(`INSERT INTO sessions (machine_id, droplet_id, size, region, started_at, cost_per_hour)
     VALUES (?, ?, ?, ?, datetime('now'), ?)`).run(s.machine_id, s.droplet_id, s.size, s.region, s.cost_per_hour);
	return Number(info.lastInsertRowid);
}
function getOpenSession(machineId) {
	return db.query("SELECT * FROM sessions WHERE machine_id = ? AND stopped_at IS NULL ORDER BY id DESC LIMIT 1").get(machineId) ?? null;
}
function endSession(dropletId, reason) {
	db.query(`UPDATE sessions
     SET stopped_at = datetime('now'), end_reason = ?
     WHERE droplet_id = ? AND stopped_at IS NULL`).run(reason, dropletId);
}
function listSessions() {
	return db.query("SELECT * FROM sessions ORDER BY started_at DESC").all();
}
function addEvent(machineId, kind, detail) {
	db.query("INSERT INTO events (machine_id, kind, detail) VALUES (?, ?, ?)").run(machineId, kind, detail);
}
//#endregion
export { getMachine as a, getSetting as c, listSessions as d, machineTag as f, updateMachine as g, startSession as h, endSession as i, isConfigured as l, setSetting as m, createMachine as n, getMachineByDaemonToken as o, setDaemonReady as p, deleteMachine as r, getOpenSession as s, addEvent as t, listMachines as u };

//# sourceMappingURL=db.js.map