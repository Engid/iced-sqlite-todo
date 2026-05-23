use rusqlite::{params, Connection, Result};

// ── Domain type ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

// ── Database wrapper ───────────────────────────────────────────────────

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) a database at `path`.
    /// On native this is a file path; on wasm it resolves through whatever VFS
    /// is registered as default (OPFS SAH-pool if you called `init_opfs` first,
    /// otherwise the built-in memory VFS).
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.create_tables()?;
        Ok(db)
    }

    /// Pure in-memory database — useful as a fallback when OPFS is unavailable.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.create_tables()?;
        Ok(db)
    }

    fn create_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                title     TEXT    NOT NULL,
                completed INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        Ok(())
    }

    // ── CRUD ───────────────────────────────────────────────────────────

    pub fn add(&self, title: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO todos (title) VALUES (?1)",
            params![title],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list(&self) -> Result<Vec<Todo>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, completed FROM todos ORDER BY id DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Todo {
                id: row.get(0)?,
                title: row.get(1)?,
                completed: row.get::<_, i64>(2)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn toggle(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE todos SET completed = 1 - completed WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM todos WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn count(&self) -> Result<(usize, usize)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM todos", [], |r| r.get(0))?;
        let done: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM todos WHERE completed = 1",
            [],
            |r| r.get(0),
        )?;
        Ok((done as usize, total as usize))
    }
}
