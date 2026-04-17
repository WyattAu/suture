use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub index: u64,
    pub term: u64,
    pub command: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct RaftLog {
    entries: Vec<LogEntry>,
}

impl RaftLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn append(&mut self, term: u64, command: Vec<u8>) -> u64 {
        let index = self.entries.len() as u64 + 1;
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
        if index == 0 || index as usize > self.entries.len() {
            return None;
        }
        Some(&self.entries[(index - 1) as usize])
    }

    pub fn last_index(&self) -> u64 {
        self.entries.len() as u64
    }

    pub fn last_term(&self) -> u64 {
        self.entries.last().map(|e| e.term).unwrap_or(0)
    }

    pub fn term_for(&self, index: u64) -> Option<u64> {
        self.get(index).map(|e| e.term)
    }

    pub fn entries_from(&self, index: u64) -> &[LogEntry] {
        if index == 0 || self.entries.is_empty() {
            return &[];
        }
        let start = (index - 1) as usize;
        if start >= self.entries.len() {
            return &[];
        }
        &self.entries[start..]
    }

    pub fn truncate_from(&mut self, index: u64) {
        if index == 0 || index as usize > self.entries.len() {
            return;
        }
        self.entries.truncate((index - 1) as usize);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn as_slice(&self) -> &[LogEntry] {
        &self.entries
    }
}

impl Default for RaftLog {
    fn default() -> Self {
        Self::new()
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
}
