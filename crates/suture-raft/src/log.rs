use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub index: u64,
    pub term: u64,
    pub command: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub data: Vec<u8>,
    pub last_included_index: u64,
    pub last_included_term: u64,
    pub created_at: u64,
}

#[derive(Clone, Debug)]
pub struct RaftLog {
    entries: Vec<LogEntry>,
    snapshot_index: u64,
    snapshot_term: u64,
}

impl RaftLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            snapshot_index: 0,
            snapshot_term: 0,
        }
    }

    pub fn append(&mut self, term: u64, command: Vec<u8>) -> u64 {
        let index = self.snapshot_index + self.entries.len() as u64 + 1;
        self.entries.push(LogEntry {
            index,
            term,
            command,
        });
        index
    }

    pub fn append_entry(&mut self, entry: LogEntry) {
        self.entries.push(entry);
    }

    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        if index == 0 || index <= self.snapshot_index {
            return None;
        }
        let local = (index - self.snapshot_index - 1) as usize;
        self.entries.get(local)
    }

    pub fn last_index(&self) -> u64 {
        self.snapshot_index + self.entries.len() as u64
    }

    pub fn last_term(&self) -> u64 {
        if self.entries.is_empty() {
            return self.snapshot_term;
        }
        self.entries.last().map(|e| e.term).unwrap_or(0)
    }

    pub fn term_for(&self, index: u64) -> Option<u64> {
        if index == self.snapshot_index && self.snapshot_index > 0 {
            return Some(self.snapshot_term);
        }
        self.get(index).map(|e| e.term)
    }

    pub fn entries_from(&self, index: u64) -> &[LogEntry] {
        if index == 0 || self.entries.is_empty() {
            return &[];
        }
        let local = (index - self.snapshot_index - 1) as usize;
        if local >= self.entries.len() {
            return &[];
        }
        &self.entries[local..]
    }

    pub fn truncate_from(&mut self, index: u64) {
        if index == 0 || index <= self.snapshot_index {
            return;
        }
        let local = (index - self.snapshot_index - 1) as usize;
        if local >= self.entries.len() {
            return;
        }
        self.entries.truncate(local);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.snapshot_index == 0
    }

    pub fn as_slice(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn compact(&mut self, last_index: u64) {
        if last_index <= self.snapshot_index || self.entries.is_empty() {
            return;
        }
        if last_index >= self.last_index() {
            let last_entry = self.entries.last().unwrap();
            self.snapshot_index = last_entry.index;
            self.snapshot_term = last_entry.term;
            self.entries.clear();
            return;
        }
        let keep_local = (last_index - self.snapshot_index) as usize;
        if keep_local >= self.entries.len() {
            return;
        }
        self.snapshot_index = last_index;
        self.snapshot_term = self.entries[keep_local - 1].term;
        self.entries = self.entries.split_off(keep_local);
    }

    pub fn set_snapshot(&mut self, index: u64, term: u64) {
        if index > self.snapshot_index {
            self.snapshot_index = index;
            self.snapshot_term = term;
            self.entries.clear();
        }
    }

    pub fn snapshot_index(&self) -> u64 {
        self.snapshot_index
    }

    pub fn snapshot_term(&self) -> u64 {
        self.snapshot_term
    }
}

impl Default for RaftLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "persist")]
pub struct SqliteRaftLog {
    conn: rusqlite::Connection,
}

#[cfg(feature = "persist")]
impl SqliteRaftLog {
    pub fn new(path: &std::path::Path) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS raft_log (
                \"index\" INTEGER PRIMARY KEY,
                term INTEGER NOT NULL,
                command BLOB NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn append(&mut self, term: u64, command: Vec<u8>) -> u64 {
        let index = self.last_index() + 1;
        if let Err(e) = self.conn.execute(
            "INSERT INTO raft_log (\"index\", term, command) VALUES (?1, ?2, ?3)",
            rusqlite::params![index as i64, term as i64, command],
        ) {
            eprintln!("raft: failed to append log entry: {e}");
        }
        index
    }

    pub fn append_entry(&mut self, entry: LogEntry) -> Result<(), rusqlite::Error> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO raft_log (\"index\", term, command) VALUES (?1, ?2, ?3)",
                rusqlite::params![entry.index as i64, entry.term as i64, entry.command],
            )?;
        Ok(())
    }

    pub fn get(&self, index: u64) -> Option<LogEntry> {
        if index == 0 {
            return None;
        }
        let result = self.conn.query_row(
            "SELECT \"index\", term, command FROM raft_log WHERE \"index\" = ?1",
            rusqlite::params![index as i64],
            |row| {
                Ok(LogEntry {
                    index: row.get::<_, i64>(0)? as u64,
                    term: row.get::<_, i64>(1)? as u64,
                    command: row.get(2)?,
                })
            },
        );
        result.ok()
    }

    pub fn last_index(&self) -> u64 {
        let result = self
            .conn
            .query_row("SELECT MAX(\"index\") FROM raft_log", [], |row| {
                row.get::<_, Option<i64>>(0)
            });
        result.unwrap_or(None).unwrap_or(0) as u64
    }

    pub fn last_term(&self) -> u64 {
        let idx = self.last_index();
        if idx == 0 {
            return 0;
        }
        self.get(idx).map(|e| e.term).unwrap_or(0)
    }

    pub fn term_for(&self, index: u64) -> Option<u64> {
        self.get(index).map(|e| e.term)
    }

    pub fn entries_from(&self, index: u64) -> Result<Vec<LogEntry>, rusqlite::Error> {
        if index == 0 {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT \"index\", term, command FROM raft_log WHERE \"index\" >= ?1 ORDER BY \"index\"")?;
        let rows = stmt.query_map(rusqlite::params![index as i64], |row| {
            Ok(LogEntry {
                index: row.get::<_, i64>(0)? as u64,
                term: row.get::<_, i64>(1)? as u64,
                command: row.get(2)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn truncate_from(&mut self, index: u64) {
        if index == 0 {
            return;
        }
        let _ = self.conn.execute(
            "DELETE FROM raft_log WHERE \"index\" >= ?1",
            rusqlite::params![index as i64],
        );
    }

    pub fn is_empty(&self) -> bool {
        self.last_index() == 0
    }

    pub fn as_slice(&self) -> Result<Vec<LogEntry>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT \"index\", term, command FROM raft_log ORDER BY \"index\"")?;
        let rows = stmt.query_map([], |row| {
            Ok(LogEntry {
                index: row.get::<_, i64>(0)? as u64,
                term: row.get::<_, i64>(1)? as u64,
                command: row.get(2)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_append_and_get() {
        let mut log = RaftLog::new();
        let idx = log.append(1, vec![1, 2, 3]);
        assert_eq!(idx, 1);

        let entry = log.get(1).unwrap();
        assert_eq!(entry.term, 1);
        assert_eq!(entry.command, vec![1, 2, 3]);
        assert_eq!(entry.index, 1);

        let idx2 = log.append(2, vec![4, 5]);
        assert_eq!(idx2, 2);
        assert_eq!(log.get(2).unwrap().term, 2);
    }

    #[test]
    fn test_log_empty() {
        let log = RaftLog::new();
        assert!(log.is_empty());
        assert_eq!(log.last_index(), 0);
        assert_eq!(log.last_term(), 0);
        assert!(log.get(0).is_none());
        assert!(log.get(1).is_none());
        assert!(log.entries_from(1).is_empty());
        assert!(log.term_for(1).is_none());
    }

    #[test]
    fn test_log_last_entry() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(3, vec![3]);

        assert_eq!(log.last_index(), 3);
        assert_eq!(log.last_term(), 3);
    }

    #[test]
    fn test_log_entries_from() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(2, vec![3]);
        log.append(2, vec![4]);

        let entries = log.entries_from(3);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 3);
        assert_eq!(entries[1].index, 4);

        let entries = log.entries_from(5);
        assert!(entries.is_empty());

        let entries = log.entries_from(1);
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn test_log_term_for_index() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(3, vec![3]);

        assert_eq!(log.term_for(1), Some(1));
        assert_eq!(log.term_for(2), Some(1));
        assert_eq!(log.term_for(3), Some(3));
        assert_eq!(log.term_for(4), None);
        assert_eq!(log.term_for(0), None);
    }

    #[test]
    fn test_log_compact_partial() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(2, vec![3]);
        log.append(2, vec![4]);
        log.append(3, vec![5]);

        log.compact(2);
        assert_eq!(log.snapshot_index(), 2);
        assert_eq!(log.snapshot_term(), 1);
        assert_eq!(log.last_index(), 5);
        assert!(log.get(1).is_none());
        assert!(log.get(2).is_none());
        assert_eq!(log.get(3).unwrap().command, vec![3]);
        assert_eq!(log.last_term(), 3);
        assert_eq!(log.term_for(2), Some(1));
    }

    #[test]
    fn test_log_compact_full() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(2, vec![2]);

        log.compact(2);
        assert_eq!(log.snapshot_index(), 2);
        assert_eq!(log.snapshot_term(), 2);
        assert!(log.as_slice().is_empty());
        assert_eq!(log.last_index(), 2);
        assert_eq!(log.last_term(), 2);
        assert!(log.get(2).is_none());
    }

    #[test]
    fn test_log_append_after_compact() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(2, vec![3]);

        log.compact(2);
        let idx = log.append(3, vec![4]);
        assert_eq!(idx, 4);
        assert_eq!(log.last_index(), 4);
        assert_eq!(log.get(4).unwrap().command, vec![4]);
    }

    #[test]
    fn test_log_compact_no_op() {
        let mut log = RaftLog::new();
        log.append(1, vec![1]);
        log.append(2, vec![2]);

        log.compact(0);
        assert_eq!(log.snapshot_index(), 0);

        log.compact(5);
        assert_eq!(log.snapshot_index(), 2);
        assert_eq!(log.snapshot_term(), 2);
    }
}

#[cfg(feature = "persist")]
#[cfg(test)]
mod persist_tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(format!("test_{name}_{}.db", std::process::id()))
    }

    #[test]
    fn test_sqlite_log_append_and_get() {
        let path = temp_path("append_get");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        let idx = log.append(1, vec![1, 2, 3]);
        assert_eq!(idx, 1);

        let entry = log.get(1).unwrap();
        assert_eq!(entry.term, 1);
        assert_eq!(entry.command, vec![1, 2, 3]);
        assert_eq!(entry.index, 1);

        let idx2 = log.append(2, vec![4, 5]);
        assert_eq!(idx2, 2);
        assert_eq!(log.get(2).unwrap().term, 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_empty() {
        let path = temp_path("empty");
        let _ = std::fs::remove_file(&path);
        let log = SqliteRaftLog::new(&path).expect("open");
        assert!(log.is_empty());
        assert_eq!(log.last_index(), 0);
        assert_eq!(log.last_term(), 0);
        assert!(log.get(0).is_none());
        assert!(log.get(1).is_none());
        assert!(log.entries_from(1).expect("entries_from").is_empty());
        assert!(log.term_for(1).is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_last_entry() {
        let path = temp_path("last_entry");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(3, vec![3]);

        assert_eq!(log.last_index(), 3);
        assert_eq!(log.last_term(), 3);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_entries_from() {
        let path = temp_path("entries_from");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(2, vec![3]);
        log.append(2, vec![4]);

        let entries = log.entries_from(3).expect("entries_from");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 3);
        assert_eq!(entries[1].index, 4);

        let entries = log.entries_from(5).expect("entries_from");
        assert!(entries.is_empty());

        let entries = log.entries_from(1).expect("entries_from");
        assert_eq!(entries.len(), 4);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_term_for_index() {
        let path = temp_path("term_for");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(3, vec![3]);

        assert_eq!(log.term_for(1), Some(1));
        assert_eq!(log.term_for(2), Some(1));
        assert_eq!(log.term_for(3), Some(3));
        assert_eq!(log.term_for(4), None);
        assert_eq!(log.term_for(0), None);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_truncate_from() {
        let path = temp_path("truncate");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        log.append(1, vec![1]);
        log.append(1, vec![2]);
        log.append(2, vec![3]);
        log.append(2, vec![4]);

        log.truncate_from(3);
        assert_eq!(log.last_index(), 2);
        assert!(log.get(3).is_none());

        let entries = log.entries_from(1).expect("entries_from");
        assert_eq!(entries.len(), 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_persistence() {
        let path = temp_path("persistence");
        let _ = std::fs::remove_file(&path);
        {
            let mut log = SqliteRaftLog::new(&path).expect("open");
            log.append(1, vec![10, 20]);
            log.append(2, vec![30, 40]);
        }
        {
            let log = SqliteRaftLog::new(&path).expect("reopen");
            assert_eq!(log.last_index(), 2);
            let entry = log.get(1).unwrap();
            assert_eq!(entry.term, 1);
            assert_eq!(entry.command, vec![10, 20]);
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_append_entry() {
        let path = temp_path("append_entry");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        let entry = LogEntry {
            index: 1,
            term: 5,
            command: vec![99],
        };
        log.append_entry(entry).expect("append_entry");
        assert_eq!(log.last_index(), 1);
        assert_eq!(log.last_term(), 5);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sqlite_log_as_slice() {
        let path = temp_path("as_slice");
        let _ = std::fs::remove_file(&path);
        let mut log = SqliteRaftLog::new(&path).expect("open");
        log.append(1, vec![1]);
        log.append(2, vec![2]);

        let slice = log.as_slice().expect("as_slice");
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0].index, 1);
        assert_eq!(slice[1].index, 2);

        let _ = std::fs::remove_file(&path);
    }
}
