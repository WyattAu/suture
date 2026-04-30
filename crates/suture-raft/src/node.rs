use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::RaftError;
use crate::log::{LogEntry, RaftLog, Snapshot};
use crate::message::RaftMessage;
use rand::Rng;

pub type NodeId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    Follower,
    PreCandidate,
    Candidate,
    Leader,
}

pub struct PreVote {
    pub term: u64,
    pub last_log_term: u64,
    pub last_log_index: u64,
    pub votes_received: HashSet<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub nodes: Vec<NodeId>,
    pub transition: Option<Vec<NodeId>>,
}

pub struct ReadIndex {
    pub index: u64,
    pub term: u64,
    pub quorum_acked: HashSet<NodeId>,
}

pub struct RaftNode {
    id: NodeId,
    state: NodeState,
    current_term: u64,
    voted_for: Option<NodeId>,
    log: RaftLog,
    commit_index: u64,
    last_applied: u64,
    next_index: HashMap<NodeId, u64>,
    match_index: HashMap<NodeId, u64>,
    leader_id: Option<NodeId>,
    peers: Vec<NodeId>,
    election_timeout: Duration,
    heartbeat_interval: Duration,
    ticks_since_reset: u64,
    votes_received: HashSet<NodeId>,
    pre_vote: Option<PreVote>,
    snapshot: Option<Snapshot>,
    config: ClusterConfig,
    pending_transfer: Option<NodeId>,
}

impl RaftNode {
    pub fn new(id: NodeId, peers: Vec<NodeId>) -> Self {
        let election_timeout = Duration::from_millis(10);
        let max_offset = election_timeout.as_millis() as u64;
        let initial_offset = if max_offset > 0 {
            rand::thread_rng().gen_range(0..max_offset)
        } else {
            0
        };
        let all_nodes: Vec<NodeId> = std::iter::once(id).chain(peers.iter().copied()).collect();
        Self {
            id,
            state: NodeState::Follower,
            current_term: 0,
            voted_for: None,
            log: RaftLog::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            leader_id: None,
            peers,
            election_timeout,
            heartbeat_interval: Duration::from_millis(3),
            ticks_since_reset: initial_offset,
            votes_received: HashSet::new(),
            pre_vote: None,
            snapshot: None,
            config: ClusterConfig {
                nodes: all_nodes,
                transition: None,
            },
            pending_transfer: None,
        }
    }

    pub fn with_timeouts(
        id: NodeId,
        peers: Vec<NodeId>,
        election_timeout: Duration,
        heartbeat_interval: Duration,
    ) -> Self {
        Self {
            election_timeout,
            heartbeat_interval,
            ..Self::new(id, peers)
        }
    }

    pub fn tick(&mut self) -> Vec<(NodeId, RaftMessage)> {
        self.ticks_since_reset += 1;

        if self.state == NodeState::PreCandidate {
            let timeout = self.election_timeout.as_millis() as u64;
            if self.ticks_since_reset >= timeout {
                self.pre_vote = None;
                self.state = NodeState::Follower;
                return Vec::new();
            }
            return Vec::new();
        }

        match self.state {
            NodeState::Follower | NodeState::Candidate => {
                let timeout = self.election_timeout.as_millis() as u64;
                if self.ticks_since_reset >= timeout {
                    return self.start_pre_vote();
                }
                Vec::new()
            }
            NodeState::Leader => {
                let interval = self.heartbeat_interval.as_millis() as u64;
                if self.ticks_since_reset >= interval {
                    self.ticks_since_reset = 0;
                    let mut messages = self.replicate_log();
                    if let Some(target) = self.pending_transfer {
                        if self.match_index.get(&target).copied().unwrap_or(0)
                            >= self.log.last_index()
                        {
                            messages.push((
                                target,
                                RaftMessage::TimeoutNow {
                                    term: self.current_term,
                                },
                            ));
                            self.pending_transfer = None;
                        }
                    }
                    return messages;
                }
                Vec::new()
            }
            NodeState::PreCandidate => Vec::new(),
        }
    }

    pub fn handle_message(&mut self, from: NodeId, msg: RaftMessage) -> Vec<(NodeId, RaftMessage)> {
        match msg {
            RaftMessage::AppendEntriesRequest {
                term,
                leader_id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                let resp = self.handle_append_entries(
                    from,
                    term,
                    leader_id,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit,
                );
                resp.map(|r| vec![(from, r)]).unwrap_or_default()
            }
            RaftMessage::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => {
                self.handle_append_entries_response(from, term, success, match_index);
                Vec::new()
            }
            RaftMessage::RequestVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
                is_pre_vote,
            } => {
                let resp = self.handle_request_vote(
                    term,
                    candidate_id,
                    last_log_index,
                    last_log_term,
                    is_pre_vote,
                );
                resp.map(|r| vec![(from, r)]).unwrap_or_default()
            }
            RaftMessage::RequestVoteResponse {
                term,
                vote_granted,
                is_pre_vote,
            } => {
                if is_pre_vote {
                    self.handle_pre_vote_response(from, term, vote_granted)
                } else {
                    self.handle_request_vote_response(from, term, vote_granted);
                    Vec::new()
                }
            }
            RaftMessage::InstallSnapshotRequest {
                term,
                leader_id,
                snapshot,
            } => {
                let resp = self.handle_install_snapshot(from, term, leader_id, snapshot);
                resp.map(|r| vec![(from, r)]).unwrap_or_default()
            }
            RaftMessage::InstallSnapshotResponse { term } => {
                self.handle_install_snapshot_response(from, term);
                Vec::new()
            }
            RaftMessage::ReadIndexRequest { term: _term, index: _index } => {
                vec![(
                    from,
                    RaftMessage::ReadIndexResponse { term: self.current_term },
                )]
            }
            RaftMessage::ReadIndexResponse { term } => {
                if term > self.current_term {
                    self.step_down(term);
                }
                Vec::new()
            }
            RaftMessage::TimeoutNow { term } => {
                if term >= self.current_term {
                    self.start_election()
                } else {
                    Vec::new()
                }
            }
            RaftMessage::ConfigChangeRequest {
                term,
                leader_id,
                new_nodes: _new_nodes,
            } => {
                if term >= self.current_term {
                    self.current_term = term;
                    self.voted_for = None;
                    self.state = NodeState::Follower;
                    self.leader_id = Some(leader_id);
                    self.ticks_since_reset = 0;
                }
                Vec::new()
            }
        }
    }

    pub fn state(&self) -> &NodeState {
        &self.state
    }

    pub fn term(&self) -> u64 {
        self.current_term
    }

    pub fn leader(&self) -> Option<NodeId> {
        self.leader_id
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn propose(&mut self, command: Vec<u8>) -> Result<(), RaftError> {
        if self.state != NodeState::Leader {
            return Err(RaftError::NotLeader);
        }
        let index = self.log.append(self.current_term, command);
        self.next_index.insert(self.id, index + 1);
        self.match_index.insert(self.id, index);
        self.try_commit();
        Ok(())
    }

    pub fn committed_entries(&self) -> &[LogEntry] {
        if self.last_applied >= self.commit_index {
            return &[];
        }
        let start = self.last_applied as usize;
        let end = self.commit_index as usize;
        &self.log.as_slice()[start..end]
    }

    pub fn advance_applied(&mut self, count: usize) {
        self.last_applied += count as u64;
    }

    pub fn start_pre_vote(&mut self) -> Vec<(NodeId, RaftMessage)> {
        let last_index = self.log.last_index();
        let last_term = self.log.last_term();
        self.state = NodeState::PreCandidate;
        self.pre_vote = Some(PreVote {
            term: self.current_term,
            last_log_term: last_term,
            last_log_index: last_index,
            votes_received: HashSet::from_iter([self.id]),
        });
        self.ticks_since_reset = 0;

        tracing::info!(
            node = self.id,
            term = self.current_term,
            "starting pre-vote"
        );

        if self.pre_vote.as_ref().unwrap().votes_received.len() >= self.majority() {
            self.pre_vote = None;
            return self.start_election();
        }

        self.peers
            .iter()
            .map(|&peer| {
                (
                    peer,
                    RaftMessage::RequestVoteRequest {
                        term: self.current_term,
                        candidate_id: self.id,
                        last_log_index: last_index,
                        last_log_term: last_term,
                        is_pre_vote: true,
                    },
                )
            })
            .collect()
    }

    pub fn handle_pre_vote_response(&mut self, from: NodeId, term: u64, granted: bool) -> Vec<(NodeId, RaftMessage)> {
        if let Some(ref mut pre_vote) = self.pre_vote {
            if term > self.current_term {
                self.pre_vote = None;
                self.state = NodeState::Follower;
                self.step_down(term);
                return Vec::new();
            }
            if self.state != NodeState::PreCandidate {
                self.pre_vote = None;
                return Vec::new();
            }
            if granted {
                pre_vote.votes_received.insert(from);
                if pre_vote.votes_received.len() >= self.majority() {
                    self.pre_vote = None;
                    return self.start_election();
                }
            }
        }
        Vec::new()
    }

    fn start_election(&mut self) -> Vec<(NodeId, RaftMessage)> {
        self.current_term += 1;
        self.state = NodeState::Candidate;
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);
        self.ticks_since_reset = 0;
        self.leader_id = None;
        self.pre_vote = None;

        tracing::info!(
            node = self.id,
            term = self.current_term,
            "starting election"
        );

        if self.votes_received.len() >= self.majority() {
            self.become_leader();
            return self.replicate_log();
        }

        let last_index = self.log.last_index();
        let last_term = self.log.last_term();

        self.peers
            .iter()
            .map(|&peer| {
                (
                    peer,
                    RaftMessage::RequestVoteRequest {
                        term: self.current_term,
                        candidate_id: self.id,
                        last_log_index: last_index,
                        last_log_term: last_term,
                        is_pre_vote: false,
                    },
                )
            })
            .collect()
    }

    fn become_leader(&mut self) {
        self.state = NodeState::Leader;
        self.leader_id = Some(self.id);
        let last_index = self.log.last_index();

        for &peer in &self.peers {
            self.next_index.insert(peer, last_index + 1);
            self.match_index.insert(peer, 0);
        }

        tracing::info!(node = self.id, term = self.current_term, "became leader");

        let interval = self.heartbeat_interval.as_millis() as u64;
        self.ticks_since_reset = interval.saturating_sub(1);
    }

    fn replicate_log(&mut self) -> Vec<(NodeId, RaftMessage)> {
        let mut messages = Vec::new();

        for &peer in &self.peers {
            let next_idx = self.next_index.get(&peer).copied().unwrap_or(1);
            let prev_log_index = next_idx.saturating_sub(1);

            if prev_log_index > 0 && prev_log_index <= self.log.snapshot_index() {
                messages.push((
                    peer,
                    RaftMessage::InstallSnapshotRequest {
                        term: self.current_term,
                        leader_id: self.id,
                        snapshot: self.snapshot.clone().unwrap_or(Snapshot {
                            data: vec![],
                            last_included_index: self.log.snapshot_index(),
                            last_included_term: self.log.snapshot_term(),
                            created_at: 0,
                        }),
                    },
                ));
                continue;
            }

            let prev_log_term = self.log.term_for(prev_log_index).unwrap_or(0);
            let entries = self.log.entries_from(next_idx).to_vec();

            messages.push((
                peer,
                RaftMessage::AppendEntriesRequest {
                    term: self.current_term,
                    leader_id: self.id,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit: self.commit_index,
                },
            ));
        }

        messages
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_append_entries(
        &mut self,
        _from: NodeId,
        term: u64,
        leader_id: NodeId,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    ) -> Option<RaftMessage> {
        if term < self.current_term {
            return Some(RaftMessage::AppendEntriesResponse {
                term: self.current_term,
                success: false,
                match_index: 0,
            });
        }

        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
        }
        self.state = NodeState::Follower;
        self.leader_id = Some(leader_id);
        self.ticks_since_reset = 0;
        self.pre_vote = None;

        if prev_log_index > 0 {
            match self.log.term_for(prev_log_index) {
                None => {
                    return Some(RaftMessage::AppendEntriesResponse {
                        term: self.current_term,
                        success: false,
                        match_index: self.log.last_index(),
                    });
                }
                Some(t) if t != prev_log_term => {
                    return Some(RaftMessage::AppendEntriesResponse {
                        term: self.current_term,
                        success: false,
                        match_index: self.log.last_index(),
                    });
                }
                _ => {}
            }
        }

        for entry in &entries {
            match self.log.term_for(entry.index) {
                Some(t) if t != entry.term => {
                    self.log.truncate_from(entry.index);
                    self.log.append_entry(entry.clone());
                }
                None => {
                    self.log.append_entry(entry.clone());
                }
                Some(_) => {}
            }
        }

        if leader_commit > self.commit_index {
            self.commit_index = std::cmp::min(leader_commit, self.log.last_index());
        }

        Some(RaftMessage::AppendEntriesResponse {
            term: self.current_term,
            success: true,
            match_index: self.log.last_index(),
        })
    }

    fn handle_append_entries_response(
        &mut self,
        from: NodeId,
        term: u64,
        success: bool,
        match_index: u64,
    ) {
        if term > self.current_term {
            self.step_down(term);
            return;
        }

        if self.state != NodeState::Leader {
            return;
        }

        if success {
            self.next_index.insert(from, match_index + 1);
            self.match_index.insert(from, match_index);
            self.try_commit();
        } else {
            let current = self.next_index.get(&from).copied().unwrap_or(1);
            let min_next = (self.log.snapshot_index() + 1).max(1);
            if current > min_next {
                self.next_index.insert(from, current - 1);
            }
        }
    }

    fn handle_request_vote(
        &mut self,
        term: u64,
        candidate_id: NodeId,
        last_log_index: u64,
        last_log_term: u64,
        is_pre_vote: bool,
    ) -> Option<RaftMessage> {
        if term < self.current_term {
            return Some(RaftMessage::RequestVoteResponse {
                term: self.current_term,
                vote_granted: false,
                is_pre_vote,
            });
        }

        if !is_pre_vote {
            if term > self.current_term {
                self.current_term = term;
                self.voted_for = None;
                self.state = NodeState::Follower;
                self.leader_id = None;
                self.pre_vote = None;
            }
        }

        let can_vote = if is_pre_vote {
            let my_last_term = self.log.last_term();
            let my_last_index = self.log.last_index();
            last_log_term > my_last_term
                || (last_log_term == my_last_term && last_log_index >= my_last_index)
        } else {
            match self.voted_for {
                Some(v) if v != candidate_id => false,
                _ => {
                    let my_last_term = self.log.last_term();
                    let my_last_index = self.log.last_index();

                    last_log_term > my_last_term
                        || (last_log_term == my_last_term && last_log_index >= my_last_index)
                }
            }
        };

        if can_vote && !is_pre_vote {
            self.voted_for = Some(candidate_id);
            self.ticks_since_reset = 0;
            tracing::debug!(
                node = self.id,
                term = self.current_term,
                candidate = candidate_id,
                "granted vote"
            );
        }

        Some(RaftMessage::RequestVoteResponse {
            term: self.current_term,
            vote_granted: can_vote,
            is_pre_vote,
        })
    }

    fn handle_request_vote_response(&mut self, from: NodeId, term: u64, vote_granted: bool) {
        if term > self.current_term {
            self.step_down(term);
            return;
        }

        if self.state != NodeState::Candidate {
            return;
        }

        if vote_granted {
            self.votes_received.insert(from);
            tracing::debug!(
                node = self.id,
                term = self.current_term,
                votes = self.votes_received.len(),
                "received vote"
            );
            if self.votes_received.len() >= self.majority() {
                self.become_leader();
            }
        }
    }

    fn handle_install_snapshot(
        &mut self,
        _from: NodeId,
        term: u64,
        leader_id: NodeId,
        snapshot: Snapshot,
    ) -> Option<RaftMessage> {
        if term < self.current_term {
            return Some(RaftMessage::InstallSnapshotResponse {
                term: self.current_term,
            });
        }

        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
        }
        self.state = NodeState::Follower;
        self.leader_id = Some(leader_id);
        self.ticks_since_reset = 0;
        self.pre_vote = None;

        if snapshot.last_included_index > self.log.last_index() {
            self.log.compact(snapshot.last_included_index);
            if snapshot.last_included_index > self.log.snapshot_index() {
                self.log.compact(snapshot.last_included_index);
            }
        }

        self.snapshot = Some(snapshot);

        Some(RaftMessage::InstallSnapshotResponse {
            term: self.current_term,
        })
    }

    fn handle_install_snapshot_response(&mut self, _from: NodeId, term: u64) {
        if term > self.current_term {
            self.step_down(term);
        }
    }

    pub fn create_snapshot(
        &mut self,
        last_index: u64,
        state_machine_data: Vec<u8>,
    ) -> Result<(), RaftError> {
        let last_term = self
            .log
            .term_for(last_index)
            .ok_or(RaftError::InvalidIndex(last_index))?;

        if last_index > self.commit_index {
            return Err(RaftError::InvalidIndex(last_index));
        }

        let snapshot = Snapshot {
            data: state_machine_data,
            last_included_index: last_index,
            last_included_term: last_term,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        self.log.compact(last_index);
        self.snapshot = Some(snapshot);

        Ok(())
    }

    pub fn install_snapshot(&mut self, snapshot: Snapshot) -> Result<(), RaftError> {
        if snapshot.last_included_index <= self.log.last_index() {
            return Ok(());
        }

        self.log.compact(snapshot.last_included_index);
        if snapshot.last_included_index > self.log.snapshot_index() {
            let log = &mut self.log;
            log.set_snapshot(snapshot.last_included_index, snapshot.last_included_term);
        }
        self.snapshot = Some(snapshot);

        Ok(())
    }

    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    pub fn transfer_leadership(&mut self, target: NodeId) -> Result<(), RaftError> {
        if self.state != NodeState::Leader {
            return Err(RaftError::NotLeader);
        }

        if !self.config.nodes.contains(&target) {
            return Err(RaftError::NodeNotFound(target));
        }

        let target_match = self.match_index.get(&target).copied().unwrap_or(0);
        let last_log = self.log.last_index();

        if target_match < last_log {
            self.pending_transfer = Some(target);
            self.replicate_to(target);
        } else {
            self.step_down(self.current_term);
        }

        Ok(())
    }

    fn replicate_to(&mut self, target: NodeId) {
        let next_idx = self.next_index.get(&target).copied().unwrap_or(1);
        self.next_index.insert(target, next_idx.min(self.log.last_index() + 1));
    }

    pub fn read_index(&self) -> Result<ReadIndex, RaftError> {
        if self.state != NodeState::Leader {
            return Err(RaftError::NotLeader);
        }
        Ok(ReadIndex {
            index: self.log.last_index() + 1,
            term: self.current_term,
            quorum_acked: HashSet::new(),
        })
    }

    pub fn handle_read_ack(&mut self, read: &mut ReadIndex, from: NodeId) -> bool {
        read.quorum_acked.insert(from);
        read.quorum_acked.len() >= self.majority()
    }

    pub fn read_index_messages(&self) -> Vec<(NodeId, RaftMessage)> {
        self.peers
            .iter()
            .map(|&peer| {
                (
                    peer,
                    RaftMessage::ReadIndexRequest {
                        term: self.current_term,
                        index: self.log.last_index() + 1,
                    },
                )
            })
            .collect()
    }

    pub fn propose_membership_change(&mut self, new_nodes: Vec<NodeId>) -> Result<(), RaftError> {
        if self.state != NodeState::Leader {
            return Err(RaftError::NotLeader);
        }

        if self.config.transition.is_some() {
            return Err(RaftError::MembershipChangeInProgress);
        }

        let current_nodes = self.config.nodes.clone();

        let joint: Vec<NodeId> = current_nodes
            .iter()
            .chain(new_nodes.iter())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        self.config.transition = Some(new_nodes.clone());
        self.config.nodes = joint.clone();

        let _messages: Vec<(NodeId, RaftMessage)> = self
            .peers
            .iter()
            .map(|&peer| {
                (
                    peer,
                    RaftMessage::ConfigChangeRequest {
                        term: self.current_term,
                        leader_id: self.id,
                        new_nodes: joint.clone(),
                    },
                )
            })
            .collect();

        self.propose(
            serde_json::to_vec(&self.config)
                .unwrap_or_else(|_| b"config".to_vec()),
        )?;

        Ok(())
    }

    pub fn finalize_membership_change(&mut self) -> Result<(), RaftError> {
        let new_nodes = self
            .config
            .transition
            .take()
            .ok_or(RaftError::NoTransition)?;
        self.config.nodes = new_nodes;

        self.propose(
            serde_json::to_vec(&self.config)
                .unwrap_or_else(|_| b"config".to_vec()),
        )?;

        Ok(())
    }

    pub fn config(&self) -> &ClusterConfig {
        &self.config
    }

    pub fn pre_vote(&self) -> Option<&PreVote> {
        self.pre_vote.as_ref()
    }

    fn step_down(&mut self, term: u64) {
        if term > self.current_term {
            self.current_term = term;
        }
        self.state = NodeState::Follower;
        self.voted_for = None;
        self.leader_id = None;
        self.pre_vote = None;
        self.votes_received.clear();
        self.pending_transfer = None;
    }

    fn try_commit(&mut self) {
        let majority = self.majority();
        for n in (self.commit_index + 1..=self.log.last_index()).rev() {
            if let Some(term) = self.log.term_for(n) {
                if term != self.current_term {
                    continue;
                }
                let mut count = 1;
                for &peer in &self.peers {
                    if *self.match_index.get(&peer).unwrap_or(&0) >= n {
                        count += 1;
                    }
                }
                if count >= majority {
                    self.commit_index = n;
                    tracing::info!(
                        node = self.id,
                        commit_index = self.commit_index,
                        "committed entry"
                    );
                    return;
                }
            }
        }
    }

    #[allow(clippy::manual_div_ceil)]
    fn majority(&self) -> usize {
        (self.config.nodes.len()) / 2 + 1
    }
}

impl std::fmt::Debug for RaftNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaftNode")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("current_term", &self.current_term)
            .field("voted_for", &self.voted_for)
            .field("commit_index", &self.commit_index)
            .field("last_applied", &self.last_applied)
            .field("leader_id", &self.leader_id)
            .field("peers", &self.peers)
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_node_becomes_leader() {
        let mut node = RaftNode::new(1, vec![]);

        for _ in 0..10 {
            node.tick();
        }

        assert_eq!(node.state(), &NodeState::Leader);
        assert_eq!(node.term(), 1);
    }

    #[test]
    fn test_leader_sends_heartbeats() {
        let mut node = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::PreCandidate);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        assert_eq!(node.state(), &NodeState::Candidate);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(node.state(), &NodeState::Leader);

        let messages = node.tick();
        assert_eq!(messages.len(), 2);

        for (target, msg) in &messages {
            assert!(*target == 2 || *target == 3);
            match msg {
                RaftMessage::AppendEntriesRequest {
                    term,
                    leader_id,
                    entries,
                    ..
                } => {
                    assert_eq!(*term, 1);
                    assert_eq!(*leader_id, 1);
                    assert!(entries.is_empty());
                }
                _ => panic!("expected AppendEntriesRequest, got {:?}", msg),
            }
        }
    }

    #[test]
    fn test_follower_responds_to_vote_request() {
        let mut node = RaftNode::new(2, vec![]);

        let response = node.handle_message(
            1,
            RaftMessage::RequestVoteRequest {
                term: 1,
                candidate_id: 1,
                last_log_index: 0,
                last_log_term: 0,
                is_pre_vote: false,
            },
        );

        assert_eq!(response.len(), 1);
        match &response[0].1 {
            RaftMessage::RequestVoteResponse { term, vote_granted, .. } => {
                assert_eq!(*term, 1);
                assert!(*vote_granted);
            }
            _ => panic!("expected RequestVoteResponse"),
        }
    }

    #[test]
    fn test_reject_vote_for_stale_term() {
        let mut node = RaftNode::new(2, vec![]);

        node.handle_message(
            3,
            RaftMessage::AppendEntriesRequest {
                term: 5,
                leader_id: 3,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
        );

        let response = node.handle_message(
            1,
            RaftMessage::RequestVoteRequest {
                term: 1,
                candidate_id: 1,
                last_log_index: 0,
                last_log_term: 0,
                is_pre_vote: false,
            },
        );

        assert_eq!(response.len(), 1);
        match &response[0].1 {
            RaftMessage::RequestVoteResponse { term, vote_granted, .. } => {
                assert_eq!(*term, 5);
                assert!(!*vote_granted);
            }
            _ => panic!("expected RequestVoteResponse"),
        }
    }

    #[test]
    fn test_node_steps_down_for_higher_term() {
        let mut node = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::PreCandidate);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        assert_eq!(node.state(), &NodeState::Candidate);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(node.state(), &NodeState::Leader);

        node.handle_message(
            2,
            RaftMessage::AppendEntriesRequest {
                term: 5,
                leader_id: 2,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
        );

        assert_eq!(node.state(), &NodeState::Follower);
        assert_eq!(node.term(), 5);
    }

    #[test]
    fn test_candidate_election_with_quorum() {
        let mut node = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::PreCandidate);
        assert_eq!(node.term(), 0);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );

        assert_eq!(node.state(), &NodeState::Candidate);
        assert_eq!(node.term(), 1);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );

        assert_eq!(node.state(), &NodeState::Leader);
        assert_eq!(node.term(), 1);
    }

    #[test]
    fn test_log_replication_basic() {
        let mut leader = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            leader.tick();
        }
        assert_eq!(leader.state(), &NodeState::PreCandidate);

        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        assert_eq!(leader.state(), &NodeState::Candidate);

        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.propose(vec![42]).unwrap();
        assert_eq!(leader.log.last_index(), 1);

        let messages = leader.tick();
        assert!(!messages.is_empty());

        let ae_for_2 = messages
            .iter()
            .find(|(target, _)| *target == 2)
            .expect("should have AppendEntries for peer 2");
        match &ae_for_2.1 {
            RaftMessage::AppendEntriesRequest { entries, .. } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].command, vec![42]);
            }
            _ => panic!("expected AppendEntriesRequest"),
        }

        leader.handle_message(
            2,
            RaftMessage::AppendEntriesResponse {
                term: 1,
                success: true,
                match_index: 1,
            },
        );

        assert_eq!(leader.committed_entries().len(), 1);
        assert_eq!(leader.committed_entries()[0].command, vec![42]);
    }

    #[test]
    fn test_propose_rejected_by_follower() {
        let mut node = RaftNode::new(1, vec![2]);

        let result = node.propose(vec![1, 2, 3]);
        assert!(result.is_err());
        match result.unwrap_err() {
            RaftError::NotLeader => {}
            other => panic!("expected NotLeader, got {:?}", other),
        }
    }

    #[test]
    fn test_pre_vote_prevents_disruption() {
        let mut leader = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.propose(vec![1]).unwrap();

        let response = leader.handle_message(
            4,
            RaftMessage::RequestVoteRequest {
                term: 10,
                candidate_id: 4,
                last_log_index: 0,
                last_log_term: 0,
                is_pre_vote: true,
            },
        );

        assert_eq!(response.len(), 1);
        match &response[0].1 {
            RaftMessage::RequestVoteResponse { vote_granted, is_pre_vote, .. } => {
                assert!(*is_pre_vote);
                assert!(!*vote_granted);
            }
            _ => panic!("expected pre-vote response"),
        }

        assert_eq!(leader.state(), &NodeState::Leader);
        assert_eq!(leader.term(), 1);
    }

    #[test]
    fn test_pre_vote_rejected_for_stale_term() {
        let mut node = RaftNode::new(1, vec![]);

        node.handle_message(
            2,
            RaftMessage::AppendEntriesRequest {
                term: 5,
                leader_id: 2,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
        );

        let response = node.handle_message(
            3,
            RaftMessage::RequestVoteRequest {
                term: 3,
                candidate_id: 3,
                last_log_index: 0,
                last_log_term: 0,
                is_pre_vote: true,
            },
        );

        assert_eq!(response.len(), 1);
        match &response[0].1 {
            RaftMessage::RequestVoteResponse { vote_granted, .. } => {
                assert!(!*vote_granted);
            }
            _ => panic!("expected pre-vote response"),
        }

        assert_eq!(node.term(), 5);
        assert_eq!(node.state(), &NodeState::Follower);
    }

    #[test]
    fn test_pre_vote_does_not_change_state() {
        let mut node = RaftNode::new(1, vec![]);

        node.handle_message(
            2,
            RaftMessage::AppendEntriesRequest {
                term: 5,
                leader_id: 2,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
        );

        assert_eq!(node.voted_for, None);

        let _ = node.handle_message(
            3,
            RaftMessage::RequestVoteRequest {
                term: 5,
                candidate_id: 3,
                last_log_index: 0,
                last_log_term: 0,
                is_pre_vote: true,
            },
        );

        assert_eq!(node.state(), &NodeState::Follower);
        assert_eq!(node.voted_for, None);
    }

    #[test]
    fn test_pre_vote_timeout_falls_back() {
        let mut node = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::PreCandidate);

        node.ticks_since_reset = 0;
        for _ in 0..9 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::PreCandidate);

        node.tick();
        assert_eq!(node.state(), &NodeState::Follower);
    }

    #[test]
    fn test_log_compaction_snapshot() {
        let mut node = RaftNode::new(1, vec![]);

        for _ in 0..10 {
            node.tick();
        }
        for _ in 0..15 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::Leader);

        node.propose(vec![1]).unwrap();
        node.propose(vec![2]).unwrap();
        node.propose(vec![3]).unwrap();

        node.handle_message(
            1,
            RaftMessage::AppendEntriesResponse {
                term: 1,
                success: true,
                match_index: 3,
            },
        );

        let result = node.create_snapshot(2, vec![99, 98]);
        assert!(result.is_ok());

        let snap = node.snapshot().unwrap();
        assert_eq!(snap.last_included_index, 2);
        assert_eq!(snap.last_included_term, 1);
        assert_eq!(snap.data, vec![99, 98]);
        assert_eq!(node.log.last_index(), 3);
        assert!(node.log.get(1).is_none());
        assert!(node.log.get(2).is_none());
        assert_eq!(node.log.get(3).unwrap().command, vec![3]);
    }

    #[test]
    fn test_snapshot_install() {
        let mut node = RaftNode::new(2, vec![]);

        let snapshot = Snapshot {
            data: vec![10, 20],
            last_included_index: 5,
            last_included_term: 2,
            created_at: 1000,
        };

        let result = node.install_snapshot(snapshot);
        assert!(result.is_ok());

        let snap = node.snapshot().unwrap();
        assert_eq!(snap.last_included_index, 5);
        assert_eq!(snap.last_included_term, 2);
        assert_eq!(node.log.last_index(), 5);
    }

    #[test]
    fn test_snapshot_install_already_have_data() {
        let mut node = RaftNode::new(1, vec![]);

        for _ in 0..10 {
            node.tick();
        }
        for _ in 0..15 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::Leader);

        node.propose(vec![1]).unwrap();
        node.propose(vec![2]).unwrap();

        let snapshot = Snapshot {
            data: vec![],
            last_included_index: 1,
            last_included_term: 1,
            created_at: 0,
        };

        let result = node.install_snapshot(snapshot);
        assert!(result.is_ok());
        assert_eq!(node.log.last_index(), 2);
    }

    #[test]
    fn test_snapshot_reject_uncommitted() {
        let mut leader = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.propose(vec![1]).unwrap();

        let result = leader.create_snapshot(1, vec![0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_snapshot_message() {
        let mut follower = RaftNode::new(2, vec![]);

        let snapshot = Snapshot {
            data: vec![42],
            last_included_index: 3,
            last_included_term: 1,
            created_at: 500,
        };

        let response = follower.handle_message(
            1,
            RaftMessage::InstallSnapshotRequest {
                term: 1,
                leader_id: 1,
                snapshot,
            },
        );

        assert_eq!(response.len(), 1);
        match &response[0].1 {
            RaftMessage::InstallSnapshotResponse { term } => {
                assert_eq!(*term, 1);
            }
            _ => panic!("expected InstallSnapshotResponse"),
        }

        assert_eq!(follower.state(), &NodeState::Follower);
        assert_eq!(follower.leader(), Some(1));
        let snap = follower.snapshot().unwrap();
        assert_eq!(snap.last_included_index, 3);
    }

    #[test]
    fn test_leadership_transfer_to_up_to_date_follower() {
        let mut leader = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.match_index.insert(2, 0);
        leader.next_index.insert(2, 1);

        let result = leader.transfer_leadership(2);
        assert!(result.is_ok());
        assert_eq!(leader.state(), &NodeState::Follower);
    }

    #[test]
    fn test_leadership_transfer_rejected_by_non_leader() {
        let mut node = RaftNode::new(1, vec![2]);
        assert_eq!(node.state(), &NodeState::Follower);

        let result = node.transfer_leadership(2);
        assert!(result.is_err());
        match result.unwrap_err() {
            RaftError::NotLeader => {}
            other => panic!("expected NotLeader, got {:?}", other),
        }
    }

    #[test]
    fn test_leadership_transfer_rejected_for_unknown_node() {
        let mut leader = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        let result = leader.transfer_leadership(99);
        assert!(result.is_err());
    }

    #[test]
    fn test_leadership_transfer_pending_replication() {
        let mut leader = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.propose(vec![1]).unwrap();

        leader.match_index.insert(2, 0);
        leader.next_index.insert(2, 1);

        let result = leader.transfer_leadership(2);
        assert!(result.is_ok());
        assert_eq!(leader.state(), &NodeState::Leader);
    }

    #[test]
    fn test_timeout_now_triggers_election() {
        let mut node = RaftNode::new(1, vec![2]);

        node.current_term = 3;
        node.state = NodeState::Follower;

        let result = node.handle_message(
            2,
            RaftMessage::TimeoutNow { term: 3 },
        );

        assert!(!result.is_empty());
        assert_eq!(node.state(), &NodeState::Candidate);
        assert_eq!(node.term(), 4);
    }

    #[test]
    fn test_timeout_now_ignored_for_lower_term() {
        let mut node = RaftNode::new(1, vec![]);

        node.current_term = 5;
        node.state = NodeState::Follower;

        let result = node.handle_message(
            2,
            RaftMessage::TimeoutNow { term: 3 },
        );

        assert!(result.is_empty());
        assert_eq!(node.state(), &NodeState::Follower);
        assert_eq!(node.term(), 5);
    }

    #[test]
    fn test_read_index() {
        let mut leader = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        let read = leader.read_index().unwrap();
        assert_eq!(read.term, leader.term());

        let mut read = read;
        assert!(!leader.handle_read_ack(&mut read, 2));
        assert!(leader.handle_read_ack(&mut read, 3));
    }

    #[test]
    fn test_read_index_rejected_by_follower() {
        let node = RaftNode::new(1, vec![2]);
        let result = node.read_index();
        assert!(result.is_err());
    }

    #[test]
    fn test_read_index_messages() {
        let mut leader = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            leader.tick();
        }
        for _ in 0..15 {
            leader.tick();
        }

        let msgs = leader.read_index_messages();
        assert_eq!(msgs.len(), 2);
        for (target, msg) in &msgs {
            assert!(*target == 2 || *target == 3);
            match msg {
                RaftMessage::ReadIndexRequest { term, .. } => {
                    assert_eq!(*term, leader.term());
                }
                _ => panic!("expected ReadIndexRequest"),
            }
        }
    }

    #[test]
    fn test_membership_change_joint_consensus() {
        let mut leader = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            leader.tick();
        }
        assert_eq!(leader.state(), &NodeState::PreCandidate);

        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        assert_eq!(leader.state(), &NodeState::Candidate);

        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        let result = leader.propose_membership_change(vec![2, 3, 4]);
        assert!(result.is_ok());

        let config = leader.config();
        assert!(config.transition.is_some());
        let joint = &config.nodes;
        assert!(joint.contains(&1));
        assert!(joint.contains(&2));
        assert!(joint.contains(&3));
        assert!(joint.contains(&4));
    }

    #[test]
    fn test_membership_change_finalize() {
        let mut leader = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.propose_membership_change(vec![2, 3, 4]).unwrap();

        let result = leader.finalize_membership_change();
        assert!(result.is_ok());

        let config = leader.config();
        assert!(config.transition.is_none());
        assert_eq!(config.nodes, vec![2, 3, 4]);
    }

    #[test]
    fn test_membership_change_rejected_by_follower() {
        let mut node = RaftNode::new(1, vec![2]);
        let result = node.propose_membership_change(vec![3]);
        assert!(result.is_err());
    }

    #[test]
    fn test_membership_change_rejected_when_in_progress() {
        let mut leader = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        leader.propose_membership_change(vec![3]).unwrap();

        let result = leader.propose_membership_change(vec![4]);
        assert!(result.is_err());
        match result.unwrap_err() {
            RaftError::MembershipChangeInProgress => {}
            other => panic!("expected MembershipChangeInProgress, got {:?}", other),
        }
    }

    #[test]
    fn test_finalize_without_transition() {
        let mut leader = RaftNode::new(1, vec![2]);

        for _ in 0..10 {
            leader.tick();
        }
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 0,
                vote_granted: true,
                is_pre_vote: true,
            },
        );
        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
                is_pre_vote: false,
            },
        );
        assert_eq!(leader.state(), &NodeState::Leader);

        let result = leader.finalize_membership_change();
        assert!(result.is_err());
        match result.unwrap_err() {
            RaftError::NoTransition => {}
            other => panic!("expected NoTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_pre_vote_respects_log_completeness() {
        let mut voter = RaftNode::new(2, vec![]);

        voter.handle_message(
            1,
            RaftMessage::AppendEntriesRequest {
                term: 1,
                leader_id: 1,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![LogEntry {
                    index: 1,
                    term: 1,
                    command: vec![1],
                }],
                leader_commit: 1,
            },
        );

        let response = voter.handle_message(
            3,
            RaftMessage::RequestVoteRequest {
                term: 1,
                candidate_id: 3,
                last_log_index: 0,
                last_log_term: 0,
                is_pre_vote: true,
            },
        );

        assert_eq!(response.len(), 1);
        match &response[0].1 {
            RaftMessage::RequestVoteResponse { vote_granted, is_pre_vote, .. } => {
                assert!(*is_pre_vote);
                assert!(!*vote_granted);
            }
            _ => panic!("expected pre-vote response"),
        }
    }
}
