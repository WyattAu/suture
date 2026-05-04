use serde::{Deserialize, Serialize};
use suture_raft::{NodeState, RaftMessage, RaftNode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    pub node_id: u64,
    pub peers: Vec<u64>,
    pub election_timeout: u64,
    pub heartbeat_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HubCommand {
    CreateRepo {
        repo_id: String,
    },
    DeleteRepo {
        repo_id: String,
    },
    StoreBlob {
        hash: String,
        data: Vec<u8>,
    },
    DeleteBlob {
        hash: String,
    },
    CreateBranch {
        repo_id: String,
        branch: String,
        target: String,
    },
    UpdateBranch {
        repo_id: String,
        branch: String,
        target: String,
    },
    DeleteBranch {
        repo_id: String,
        branch: String,
    },
    StorePatch {
        repo_id: String,
        patch_id: String,
        patch_data: Vec<u8>,
    },
}

pub struct RaftHub {
    node: RaftNode,
    config: RaftConfig,
}

impl RaftHub {
    pub fn new(config: RaftConfig) -> Self {
        let node = RaftNode::new(config.node_id, config.peers.clone());
        Self { node, config }
    }

    pub fn state(&self) -> &NodeState {
        self.node.state()
    }

    pub fn leader(&self) -> Option<u64> {
        self.node.leader()
    }

    pub fn node_id(&self) -> u64 {
        self.config.node_id
    }

    pub fn tick(&mut self) -> Vec<(u64, RaftMessage)> {
        self.node.tick()
    }

    pub fn handle_message(&mut self, from: u64, msg: RaftMessage) -> Vec<(u64, RaftMessage)> {
        self.node.handle_message(from, msg)
    }

    pub fn propose(&mut self, command: HubCommand) -> Result<(), suture_raft::RaftError> {
        let data = serde_json::to_vec(&command)
            .map_err(|e| suture_raft::RaftError::Transport(e.to_string()))?;
        self.node.propose(data)
    }

    pub fn committed_commands(&self) -> Vec<HubCommand> {
        self.node
            .committed_entries()
            .iter()
            .filter_map(|entry| serde_json::from_slice(&entry.command).ok())
            .collect()
    }

    pub fn advance_applied(&mut self, count: usize) {
        self.node.advance_applied(count)
    }

    pub fn term(&self) -> u64 {
        self.node.term()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suture_raft::RaftMessage;

    #[test]
    fn test_raft_hub_creation() {
        let config = RaftConfig {
            node_id: 1,
            peers: vec![2, 3],
            election_timeout: 10,
            heartbeat_interval: 3,
        };
        let hub = RaftHub::new(config);
        assert_eq!(hub.node_id(), 1);
        assert_eq!(hub.state(), &NodeState::Follower);
        assert_eq!(hub.leader(), None);
    }

    #[test]
    fn test_hub_command_serialization() {
        let commands = vec![
            HubCommand::CreateRepo {
                repo_id: "my-repo".to_string(),
            },
            HubCommand::DeleteRepo {
                repo_id: "my-repo".to_string(),
            },
            HubCommand::StoreBlob {
                hash: "abc123".to_string(),
                data: vec![1, 2, 3],
            },
            HubCommand::DeleteBlob {
                hash: "abc123".to_string(),
            },
            HubCommand::CreateBranch {
                repo_id: "my-repo".to_string(),
                branch: "main".to_string(),
                target: "deadbeef".to_string(),
            },
            HubCommand::UpdateBranch {
                repo_id: "my-repo".to_string(),
                branch: "main".to_string(),
                target: "cafebabe".to_string(),
            },
            HubCommand::DeleteBranch {
                repo_id: "my-repo".to_string(),
                branch: "old".to_string(),
            },
            HubCommand::StorePatch {
                repo_id: "my-repo".to_string(),
                patch_id: "patch-1".to_string(),
                patch_data: vec![4, 5, 6],
            },
        ];

        for cmd in commands {
            let json = serde_json::to_vec(&cmd).expect("serialize");
            let decoded: HubCommand = serde_json::from_slice(&json).expect("deserialize");
            assert_eq!(cmd, decoded);
        }
    }

    #[test]
    fn test_raft_hub_tick_produces_messages() {
        let config = RaftConfig {
            node_id: 1,
            peers: vec![2, 3],
            election_timeout: 10,
            heartbeat_interval: 3,
        };
        let mut hub = RaftHub::new(config);

        let mut found_vote_request = false;
        for _ in 0..20 {
            let messages = hub.tick();
            for (_, msg) in messages {
                if matches!(msg, RaftMessage::RequestVoteRequest { .. }) {
                    found_vote_request = true;
                }
            }
        }
        assert!(found_vote_request);
    }

    #[test]
    fn test_raft_hub_propose_as_leader() {
        let config = RaftConfig {
            node_id: 1,
            peers: vec![],
            election_timeout: 10,
            heartbeat_interval: 3,
        };
        let mut hub = RaftHub::new(config);

        for _ in 0..20 {
            hub.tick();
            if hub.state() == &NodeState::Leader {
                break;
            }
        }
        assert_eq!(hub.state(), &NodeState::Leader);

        let cmd = HubCommand::CreateRepo {
            repo_id: "test-repo".to_string(),
        };
        hub.propose(cmd.clone())
            .expect("propose should succeed as leader");

        let committed = hub.committed_commands();
        assert!(
            committed.contains(&cmd),
            "committed commands should contain the proposed command"
        );
        hub.advance_applied(committed.len());
    }
}
