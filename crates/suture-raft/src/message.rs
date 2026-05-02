use serde::{Deserialize, Serialize};

use crate::log::LogEntry;
use crate::log::Snapshot;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftMessage {
    AppendEntriesRequest {
        term: u64,
        leader_id: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    AppendEntriesResponse {
        term: u64,
        success: bool,
        match_index: u64,
    },
    RequestVoteRequest {
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
        is_pre_vote: bool,
    },
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
        is_pre_vote: bool,
    },
    InstallSnapshotRequest {
        term: u64,
        leader_id: u64,
        snapshot: Snapshot,
    },
    InstallSnapshotResponse {
        term: u64,
    },
    ReadIndexRequest {
        term: u64,
        index: u64,
    },
    ReadIndexResponse {
        term: u64,
    },
    TimeoutNow {
        term: u64,
    },
    ConfigChangeRequest {
        term: u64,
        leader_id: u64,
        new_nodes: Vec<u64>,
    },
}
