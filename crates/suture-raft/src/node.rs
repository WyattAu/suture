use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::error::RaftError;
use crate::log::{LogEntry, RaftLog};
use crate::message::RaftMessage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

pub struct RaftNode {
    id: u64,
    state: NodeState,
    current_term: u64,
    voted_for: Option<u64>,
    log: RaftLog,
    commit_index: u64,
    last_applied: u64,
    next_index: HashMap<u64, u64>,
    match_index: HashMap<u64, u64>,
    leader_id: Option<u64>,
    peers: Vec<u64>,
    election_timeout: Duration,
    heartbeat_interval: Duration,
    ticks_since_reset: u64,
    votes_received: HashSet<u64>,
}

impl RaftNode {
    pub fn new(id: u64, peers: Vec<u64>) -> Self {
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
            election_timeout: Duration::from_millis(10),
            heartbeat_interval: Duration::from_millis(3),
            ticks_since_reset: 0,
            votes_received: HashSet::new(),
        }
    }

    pub fn with_timeouts(
        id: u64,
        peers: Vec<u64>,
        election_timeout: Duration,
        heartbeat_interval: Duration,
    ) -> Self {
        Self {
            election_timeout,
            heartbeat_interval,
            ..Self::new(id, peers)
        }
    }

    pub fn tick(&mut self) -> Vec<(u64, RaftMessage)> {
        self.ticks_since_reset += 1;

        match self.state {
            NodeState::Follower | NodeState::Candidate => {
                let timeout = self.election_timeout.as_millis() as u64;
                if self.ticks_since_reset >= timeout {
                    return self.start_election();
                }
                Vec::new()
            }
            NodeState::Leader => {
                let interval = self.heartbeat_interval.as_millis() as u64;
                if self.ticks_since_reset >= interval {
                    self.ticks_since_reset = 0;
                    return self.replicate_log();
                }
                Vec::new()
            }
        }
    }

    pub fn handle_message(&mut self, from: u64, msg: RaftMessage) -> Option<RaftMessage> {
        match msg {
            RaftMessage::AppendEntriesRequest {
                term,
                leader_id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => self.handle_append_entries(
                from,
                term,
                leader_id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            ),
            RaftMessage::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => {
                self.handle_append_entries_response(from, term, success, match_index);
                None
            }
            RaftMessage::RequestVoteRequest {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => self.handle_request_vote(term, candidate_id, last_log_index, last_log_term),
            RaftMessage::RequestVoteResponse { term, vote_granted } => {
                self.handle_request_vote_response(from, term, vote_granted);
                None
            }
        }
    }

    pub fn state(&self) -> &NodeState {
        &self.state
    }

    pub fn term(&self) -> u64 {
        self.current_term
    }

    pub fn leader(&self) -> Option<u64> {
        self.leader_id
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

    fn start_election(&mut self) -> Vec<(u64, RaftMessage)> {
        self.current_term += 1;
        self.state = NodeState::Candidate;
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);
        self.ticks_since_reset = 0;
        self.leader_id = None;

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

    fn replicate_log(&mut self) -> Vec<(u64, RaftMessage)> {
        let mut messages = Vec::new();

        for &peer in &self.peers {
            let next_idx = self.next_index.get(&peer).copied().unwrap_or(1);
            let prev_log_index = next_idx.saturating_sub(1);
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
        _from: u64,
        term: u64,
        leader_id: u64,
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
        from: u64,
        term: u64,
        success: bool,
        match_index: u64,
    ) {
        if term > self.current_term {
            self.current_term = term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            self.leader_id = None;
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
            if current > 1 {
                self.next_index.insert(from, current - 1);
            }
        }
    }

    fn handle_request_vote(
        &mut self,
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    ) -> Option<RaftMessage> {
        if term < self.current_term {
            return Some(RaftMessage::RequestVoteResponse {
                term: self.current_term,
                vote_granted: false,
            });
        }

        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
            self.state = NodeState::Follower;
            self.leader_id = None;
        }

        let can_vote = match self.voted_for {
            Some(v) if v != candidate_id => false,
            _ => {
                let my_last_term = self.log.last_term();
                let my_last_index = self.log.last_index();

                last_log_term > my_last_term
                    || (last_log_term == my_last_term && last_log_index >= my_last_index)
            }
        };

        if can_vote {
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
        })
    }

    fn handle_request_vote_response(&mut self, from: u64, term: u64, vote_granted: bool) {
        if term > self.current_term {
            self.current_term = term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            self.leader_id = None;
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

    fn majority(&self) -> usize {
        self.peers.len().div_ceil(2)
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
        assert_eq!(node.leader(), Some(1));
    }

    #[test]
    fn test_leader_sends_heartbeats() {
        let mut node = RaftNode::new(1, vec![2, 3]);

        for _ in 0..10 {
            node.tick();
        }
        assert_eq!(node.state(), &NodeState::Candidate);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
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
            },
        );

        match response {
            Some(RaftMessage::RequestVoteResponse { term, vote_granted }) => {
                assert_eq!(term, 1);
                assert!(vote_granted);
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
            },
        );

        match response {
            Some(RaftMessage::RequestVoteResponse { term, vote_granted }) => {
                assert_eq!(term, 5);
                assert!(!vote_granted);
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
        assert_eq!(node.state(), &NodeState::Candidate);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
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
        assert_eq!(node.state(), &NodeState::Candidate);
        assert_eq!(node.term(), 1);

        node.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
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
        assert_eq!(leader.state(), &NodeState::Candidate);

        leader.handle_message(
            2,
            RaftMessage::RequestVoteResponse {
                term: 1,
                vote_granted: true,
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
}
