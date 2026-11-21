use std::sync::{Mutex, MutexGuard};
use rusqlite::{Connection, Result};

/// Shared database connection guarded by a mutex so Tauri commands can be
/// called from multiple threads safely.
pub struct Db(pub Mutex<Connection>);

impl Db {
    /// Open (or create) the database at `path` and ensure all tables exist.
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    /// Lock the connection for a read/write operation.
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().expect("database is poisoned")
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS books (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                author TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL CHECK (category IN ('EGW Library','Devotionals','Biographies','Bibles')),
                file_path TEXT NOT NULL,
                cover_path TEXT,
                added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS reading_progress (
                book_id INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
                chapter_index INTEGER NOT NULL DEFAULT 0,
                scroll_offset REAL NOT NULL DEFAULT 0.0,
                percent REAL NOT NULL DEFAULT 0.0,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                chapter_index INTEGER NOT NULL,
                scroll_offset REAL NOT NULL DEFAULT 0.0,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS book_chapters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                chapter_index INTEGER NOT NULL,
                text TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_book_chapters_book ON book_chapters(book_id);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }
}
