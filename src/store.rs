use crate::model::{Plan, Run, Snapshot};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

pub struct Store {
    conn: Connection,
    _lock: File,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let dir = root.join(".concord");
        if dir.exists() && fs::symlink_metadata(&dir)?.file_type().is_symlink() {
            bail!(".concord must not be a symlink")
        }
        fs::create_dir_all(&dir)?;
        for name in ["lock", "state.sqlite", "operations", "runs"] {
            let path = dir.join(name);
            if let Ok(metadata) = fs::symlink_metadata(&path)
                && metadata.file_type().is_symlink()
            {
                bail!("Concord state path must not be a symlink: {name}");
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("lock"))?;
        lock.try_lock()
            .context("another Concord operation owns this checkout")?;
        let mut conn = Connection::open(dir.join("state.sqlite"))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            bail!("unsupported SQLite schema version {version}")
        }
        let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            bail!("SQLite integrity check failed")
        }
        let tx = conn.transaction()?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS records(kind TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(kind,id)); PRAGMA user_version=1;")?;
        tx.commit()?;
        Ok(Self { conn, _lock: lock })
    }
    pub fn snapshot(&self) -> Result<Option<Snapshot>> {
        self.get("snapshot:v3", "current")
    }
    pub fn latest_plan(&self) -> Result<Option<Plan>> {
        self.get("plan:v3", "latest")
    }
    fn get<T: serde::de::DeserializeOwned>(&self, kind: &str, id: &str) -> Result<Option<T>> {
        self.conn
            .query_row(
                "SELECT payload FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| serde_json::from_str(&s).map_err(Into::into))
            .transpose()
    }
    fn save<T: Serialize>(&mut self, kind: &str, id: &str, value: &T) -> Result<()> {
        let data = serde_json::to_string(value)?;
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO records(kind,id,payload) VALUES(?1,?2,?3) ON CONFLICT(kind,id) DO UPDATE SET payload=excluded.payload",params![kind,id,data])?;
        tx.commit()?;
        Ok(())
    }
    pub fn save_snapshot(&mut self, s: &Snapshot) -> Result<()> {
        self.save("snapshot:v3", "current", s)
    }
    pub fn save_plan(&mut self, p: &Plan) -> Result<()> {
        self.save("plan:v3", &p.plan_id, p)?;
        self.save("plan:v3", "latest", p)
    }
    pub fn save_run(&mut self, r: &Run) -> Result<()> {
        self.save("run:v3", &r.run_id, r)
    }
}
