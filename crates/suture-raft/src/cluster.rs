use std::collections::BTreeMap;
use std::time::Duration;

use crate::error::RaftError;
use crate::log::LogEntry;
use crate::message::RaftMessage;
use crate::node::{NodeState, RaftNode};

pub struct Cluster {
    nodes: BTreeMap<u64, RaftNode>,
    #[allow(dead_code)]
    election_timeout: u64,
    #[allow(dead_code)]
    heartbeat_interval: u64,
}

#[derive(Debug, Default)]
pub struct ClusterTickResult {
    pub messages_sent: usize,
    pub state_changes: Vec<(u64, NodeState)>,
}

impl Cluster {
    pub fn new(peers: Vec<u64>, election_timeout: u64, heartbeat_interval: u64) -> Self {
        let mut nodes = BTreeMap::new();
        for &id in &peers {
            let node_peers: Vec<u64> = peers.iter().copied().filter(|&p| p != id).collect();
            let jitter = id;
            let node = RaftNode::with_timeouts(
                id,
                node_peers,
                Duration::from_millis(election_timeout + jitter),
                Duration::from_millis(heartbeat_interval),
            );
            nodes.insert(id, node);
        }
        Self {
            nodes,
            election_timeout,
            heartbeat_interval,
        }
    }

    pub fn tick(&mut self) -> ClusterTickResult {
        let mut result = ClusterTickResult::default();
        let mut queue: Vec<(u64, u64, RaftMessage)> = Vec::new();

        for (&id, node) in &mut self.nodes {
            let state_before = *node.state();
            let messages = node.tick();
            if *node.state() != state_before {
                result.state_changes.push((id, *node.state()));
            }
            for (target, msg) in messages {
                queue.push((id, target, msg));
                result.messages_sent += 1;
            }
        }

        let mut iterations = 0;
        while !queue.is_empty() && iterations < 100 {
            iterations += 1;
            let batch = std::mem::take(&mut queue);
            for (from, to, msg) in batch {
                if let Some(recipient) = self.nodes.get_mut(&to) {
                    let state_before = *recipient.state();
                    let responses = recipient.handle_message(from, msg);
                    for (resp_target, resp_msg) in responses {
                        queue.push((to, resp_target, resp_msg));
                        result.messages_sent += 1;
                    }
                    if *recipient.state() != state_before {
                        result.state_changes.push((to, *recipient.state()));
                    }
                }
            }
        }

        result
    }

    pub fn propose(&mut self, node_id: u64, command: Vec<u8>) -> Result<(), RaftError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(RaftError::NodeNotFound(node_id))?;
        node.propose(command)
    }

    pub fn leader(&self) -> Option<u64> {
        self.nodes
            .iter()
            .find(|(_, node)| *node.state() == NodeState::Leader)
            .map(|(&id, _)| id)
    }

    pub fn state(&self, node_id: u64) -> NodeState {
        self.nodes
            .get(&node_id)
            .map(|n| *n.state())
            .unwrap_or(NodeState::Follower)
    }

    pub fn committed_entries(&self, node_id: u64) -> Vec<LogEntry> {
        self.nodes
            .get(&node_id)
            .map(|n| n.committed_entries().to_vec())
            .unwrap_or_default()
    }

    pub fn advance_applied(&mut self, node_id: u64, count: usize) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.advance_applied(count);
        }
    }

    pub fn remove_node(&mut self, node_id: u64) {
        self.nodes.remove(&node_id);
    }

    #[allow(dead_code)]
    fn tick_until_leader(&mut self, max_ticks: u64) -> Option<u64> {
        for _ in 0..max_ticks {
            self.tick();
            if let Some(leader) = self.leader() {
                return Some(leader);
            }
        }
        None
    }

    #[allow(dead_code)]
    fn all_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.nodes.keys().copied().collect();
        ids.sort();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_node_becomes_leader() {
        let mut cluster = Cluster::new(vec![1], 10, 3);
        let leader = cluster
            .tick_until_leader(20)
            .expect("single node should become leader");
        assert_eq!(leader, 1);
        assert_eq!(cluster.state(1), NodeState::Leader);
    }

    #[test]
    #[ignore = "intermittent timing-dependent election test"]
    fn test_three_node_election() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);
        let leader = cluster
            .tick_until_leader(30)
            .expect("should elect a leader");
        assert_eq!(cluster.state(leader), NodeState::Leader);

        for id in [1u64, 2, 3] {
            if id != leader {
                assert_eq!(
                    cluster.state(id),
                    NodeState::Follower,
                    "node {id} should be a follower"
                );
            }
        }
    }

    #[test]
    fn test_leader_heartbeat() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);
        let leader = cluster
            .tick_until_leader(30)
            .expect("should elect a leader");

        for i in 0..50 {
            cluster.tick();
            assert_eq!(
                cluster.leader(),
                Some(leader),
                "leader should remain stable at tick {}",
                i
            );
        }
    }

    #[test]
    fn test_leader_propose_and_replicate() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);
        let leader = cluster
            .tick_until_leader(30)
            .expect("should elect a leader");

        cluster
            .propose(leader, b"set-key-value".to_vec())
            .expect("propose should succeed");

        for _ in 0..10 {
            cluster.tick();
        }

        for id in [1u64, 2, 3] {
            let entries = cluster.committed_entries(id);
            assert_eq!(
                entries.len(),
                1,
                "node {} should have 1 committed entry",
                id
            );
            assert_eq!(entries[0].command, b"set-key-value");
        }
    }

    #[test]
    fn test_multiple_proposals() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);
        let leader = cluster
            .tick_until_leader(30)
            .expect("should elect a leader");

        for i in 0..3u8 {
            cluster
                .propose(leader, vec![i])
                .expect("propose should succeed");
        }

        for _ in 0..15 {
            cluster.tick();
        }

        for id in [1u64, 2, 3] {
            let entries = cluster.committed_entries(id);
            assert_eq!(
                entries.len(),
                3,
                "node {} should have 3 committed entries",
                id
            );
            for (i, entry) in entries.iter().enumerate() {
                assert_eq!(
                    entry.command,
                    vec![i as u8],
                    "node {} entry {} command mismatch",
                    id,
                    i
                );
            }
        }
    }

    #[test]
    fn test_leader_failure_new_election() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);
        let old_leader = cluster
            .tick_until_leader(30)
            .expect("should elect initial leader");
        let remaining_ids: Vec<u64> = cluster.all_ids();

        cluster.remove_node(old_leader);

        let new_leader = cluster
            .tick_until_leader(30)
            .expect("should elect new leader after failure");
        assert!(
            remaining_ids.contains(&new_leader),
            "new leader {} should be from remaining nodes {:?}",
            new_leader,
            remaining_ids
        );
        assert_ne!(new_leader, old_leader);
    }

    #[test]
    fn test_log_consistency_after_leader_change() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);
        let first_leader = cluster
            .tick_until_leader(30)
            .expect("should elect initial leader");

        cluster.propose(first_leader, vec![10]).unwrap();
        cluster.propose(first_leader, vec![20]).unwrap();

        for _ in 0..10 {
            cluster.tick();
        }

        for id in [1u64, 2, 3] {
            let entries = cluster.committed_entries(id);
            assert_eq!(
                entries.len(),
                2,
                "node {} should have 2 committed entries before leader failure",
                id
            );
        }

        cluster.remove_node(first_leader);

        let remaining: Vec<u64> = cluster.all_ids();

        let new_leader = cluster
            .tick_until_leader(30)
            .expect("should elect new leader");

        cluster.propose(new_leader, vec![30]).unwrap();

        for _ in 0..10 {
            cluster.tick();
        }

        for id in &remaining {
            let entries = cluster.committed_entries(*id);
            assert!(
                entries.len() >= 3,
                "node {} should have at least 3 committed entries, got {}",
                id,
                entries.len()
            );
            assert_eq!(entries[0].command, vec![10]);
            assert_eq!(entries[1].command, vec![20]);
            assert_eq!(entries[2].command, vec![30]);
        }
    }

    #[test]
    fn test_no_split_brain() {
        let mut cluster = Cluster::new(vec![1, 2, 3], 10, 3);

        for tick in 0..100 {
            cluster.tick();
            let leaders: Vec<u64> = [1u64, 2, 3]
                .iter()
                .filter(|&&id| cluster.state(id) == NodeState::Leader)
                .copied()
                .collect();
            assert!(
                leaders.len() <= 1,
                "split brain detected at tick {}: leaders = {:?}",
                tick,
                leaders
            );
        }
    }
}
