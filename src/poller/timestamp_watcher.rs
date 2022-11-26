use std::path::Path;
use rusqlite::Connection;

/// Takes an approach similar to inotify, except that it uses a SQLite database to keep
/// track of changed files. SQLite lets us offload to disk, so we don't have the same
/// out-of-memory issues that inotify has.
pub struct TimestampWatcher {
    conn: Connection,
}

impl TimestampWatcher {
    pub fn open(db_path: &str) -> Self {
        let mut conn = Connection::open(db_path).unwrap();
        init_db(&mut conn);
        Self {
            conn
        }
    }

    fn get_ts(&self, path: &Path) -> Option<u64> {
        let ts = self.conn.query_row("\
            SELECT last_modified FROM timestamps WHERE path = ?1
        ", &[path.to_str()], |row| row.get(0));
        ts.ok()
    }

    fn set_all(&self, path: &Path) {
        self.conn.execute()
    }
}

fn init_db(conn: &mut Connection) {
    conn.execute("
        CREATE TABLE IF NOT EXISTS timestamps (
            path TEXT PRIMARY KEY,
            last_modified BIGINT
        )
    ", &[]).unwrap();
}
