//! Tamper-evident audit log using hash chaining.
//!
//! Each audit entry contains a hash of the previous entry, creating a chain.
//! Any modification to a historical entry invalidates all subsequent hashes.
//!
//! # Example
//!
//! ```no_run
//! use suture_core::audit::AuditLog;
//! use std::path::Path;
//!
//! let log = AuditLog::open(Path::new(".suture/audit.jsonl"))?;
//! let entry = log.append("alice", "commit", "created patch abc123")?;
//! assert!(entry.verify_integrity());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// A single tamper-evident audit log entry.
///
/// Each entry stores a content hash and a reference to the previous entry's hash,
/// forming an append-only hash chain. Verifying the chain ensures no historical
/// entries have been tampered with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// BLAKE3 hash of the previous entry's content hash (all zeros for the first entry).
    pub prev_hash: [u8; 32],
    /// BLAKE3 hash of this entry's fields (sequence, prev_hash, timestamp, actor, action, details).
    pub content_hash: [u8; 32],
    /// ISO 8601 timestamp of when this entry was created.
    pub timestamp: String,
    /// The identity that performed the action.
    pub actor: String,
    /// The action type (e.g., "commit", "merge", "config").
    pub action: String,
    /// Human-readable details about the action.
    pub details: String,
    /// Optional cryptographic signature for non-repudiation.
    pub signature: Option<Vec<u8>>,
}

impl AuditEntry {
    /// Compute the BLAKE3 content hash for this entry over its fields.
    pub fn compute_content_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(&self.prev_hash);
        hasher.update(self.timestamp.as_bytes());
        hasher.update(self.actor.as_bytes());
        hasher.update(self.action.as_bytes());
        hasher.update(self.details.as_bytes());
        hasher.finalize().into()
    }

    /// Verify this entry's content hash matches its stored hash.
    pub fn verify_integrity(&self) -> bool {
        self.compute_content_hash() == self.content_hash
    }

    /// Verify both this entry's integrity and its chain link to the previous entry.
    pub fn verify_chain(&self, prev: Option<&AuditEntry>) -> bool {
        if let Some(prev) = prev {
            if self.prev_hash != prev.content_hash {
                return false;
            }
        } else if self.prev_hash != [0u8; 32] {
            return false;
        }
        self.verify_integrity()
    }
}

/// An append-only audit log backed by a JSONL file.
///
/// Each line in the file is a JSON-serialized [`AuditEntry`]. New entries are
/// always appended to maintain the hash chain integrity.
pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    /// Open (or create) an audit log at the given file path.
    ///
    /// Parent directories are created automatically if they don't exist.
    pub fn open(path: &Path) -> Result<Self, std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Append a new entry to the audit log, computing hashes automatically.
    pub fn append(
        &self,
        actor: &str,
        action: &str,
        details: &str,
    ) -> Result<AuditEntry, std::io::Error> {
        let prev = self.last_entry()?;
        let prev_hash = prev.as_ref().map(|e| e.content_hash).unwrap_or([0u8; 32]);
        let sequence = prev.as_ref().map(|e| e.sequence + 1).unwrap_or(0);
        let timestamp = chrono::Utc::now().to_rfc3339();

        let entry = AuditEntry {
            sequence,
            prev_hash,
            content_hash: [0u8; 32],
            timestamp,
            actor: actor.to_string(),
            action: action.to_string(),
            details: details.to_string(),
            signature: None,
        };

        let content_hash = entry.compute_content_hash();
        let entry = AuditEntry {
            content_hash,
            ..entry
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let json = serde_json::to_string(&entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(file, "{}", json)?;

        Ok(entry)
    }

    /// Read the last (most recent) entry from the log, if any.
    pub fn last_entry(&self) -> Result<Option<AuditEntry>, std::io::Error> {
        if !self.path.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let last_line = reader.lines().map_while(Result::ok).last();
        match last_line {
            Some(line) => {
                let entry: AuditEntry = serde_json::from_str(&line)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Read all entries from the log.
    pub fn entries(&self) -> Result<Vec<AuditEntry>, std::io::Error> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            if let Ok(line) = line
                && let Ok(entry) = serde_json::from_str::<AuditEntry>(&line)
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Verify the entire chain integrity.
    ///
    /// Returns `(total_entries, first_invalid_index)`. If `first_invalid_index`
    /// is `None`, the entire chain is valid.
    pub fn verify_chain(&self) -> Result<(usize, Option<usize>), std::io::Error> {
        let entries = self.entries()?;
        let mut first_invalid = None;

        for (i, entry) in entries.iter().enumerate() {
            let prev = if i > 0 { Some(&entries[i - 1]) } else { None };
            if !entry.verify_chain(prev) && first_invalid.is_none() {
                first_invalid = Some(i);
            }
        }

        Ok((entries.len(), first_invalid))
    }

    /// Return the file size of the audit log in bytes.
    pub fn size_bytes(&self) -> Result<u64, std::io::Error> {
        if !self.path.exists() {
            return Ok(0);
        }
        Ok(std::fs::metadata(&self.path)?.len())
    }
}
