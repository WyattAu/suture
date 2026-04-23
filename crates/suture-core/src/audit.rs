//! Tamper-evident audit log using hash chaining.
//!
//! Each audit entry contains a hash of the previous entry, creating a chain.
//! Any modification to a historical entry invalidates all subsequent hashes.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub sequence: u64,
    pub prev_hash: [u8; 32],
    pub content_hash: [u8; 32],
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub details: String,
    pub signature: Option<Vec<u8>>,
}

impl AuditEntry {
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

    pub fn verify_integrity(&self) -> bool {
        self.compute_content_hash() == self.content_hash
    }

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

pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn open(path: &Path) -> Result<Self, std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { path: path.to_path_buf() })
    }

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
        let entry = AuditEntry { content_hash, ..entry };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(&entry).unwrap())?;

        Ok(entry)
    }

    pub fn last_entry(&self) -> Result<Option<AuditEntry>, std::io::Error> {
        if !self.path.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let last_line = reader.lines().filter_map(|l| l.ok()).last();
        match last_line {
            Some(line) => {
                let entry: AuditEntry = serde_json::from_str(&line)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    pub fn entries(&self) -> Result<Vec<AuditEntry>, std::io::Error> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }

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

    pub fn size_bytes(&self) -> Result<u64, std::io::Error> {
        if !self.path.exists() {
            return Ok(0);
        }
        Ok(std::fs::metadata(&self.path)?.len())
    }
}
