use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use suture_raft::NodeState;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::raft::{HubCommand, RaftConfig, RaftHub};

/// Manages the Raft background task for consensus.
///
/// When spawned, runs a tick loop and provides channels for:
/// - Proposing commands (leader only)
/// - Receiving committed commands (for applying to storage)
/// - Checking leader status
pub struct RaftRuntime {
    hub: Arc<StdMutex<RaftHub>>,
    cmd_tx: mpsc::Sender<HubCommand>,
    applied_rx: StdMutex<mpsc::Receiver<HubCommand>>,
    shutdown_tx: broadcast::Sender<()>,
    is_leader: Arc<StdMutex<bool>>,
    leader_id: Arc<StdMutex<Option<u64>>>,
}

impl RaftRuntime {
    /// Spawn the Raft runtime. Returns the runtime handle and a command sender.
    ///
    /// Use `cmd_tx` to propose commands. The runtime will replicate them
    /// via Raft and send committed commands through `applied_rx`.
    pub fn spawn(config: RaftConfig) -> (Self, mpsc::Sender<HubCommand>) {
        let hub = Arc::new(StdMutex::new(RaftHub::new(config.clone())));
        let (cmd_tx, cmd_rx) = mpsc::channel::<HubCommand>(256);
        let (applied_tx, applied_rx) = mpsc::channel::<HubCommand>(256);
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

        let is_leader = Arc::new(StdMutex::new(false));
        let leader_id = Arc::new(StdMutex::new(None));
        let hub_ref = Arc::clone(&hub);
        let is_leader_ref = Arc::clone(&is_leader);
        let leader_id_ref = Arc::clone(&leader_id);

        tokio::spawn(async move {
            let mut tick_interval = tokio::time::interval(Duration::from_millis(100));
            let mut rx = cmd_rx;

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("raft runtime shutting down");
                        break;
                    }
                    Some(cmd) = rx.recv() => {
                        let mut hub = hub_ref.lock().unwrap();
                        match hub.propose(cmd) {
                            Ok(()) => debug!("raft: command proposed"),
                            Err(e) => warn!("raft: propose failed (not leader?): {e}"),
                        }
                    }
                    _ = tick_interval.tick() => {
                        let mut hub = hub_ref.lock().unwrap();

                        let state = *hub.state();
                        *is_leader_ref.lock().unwrap() = state == NodeState::Leader;
                        *leader_id_ref.lock().unwrap() = hub.leader();

                        // Tick the Raft node
                        let messages = hub.tick();
                        drop(hub);

                        // Route outgoing messages (would go over TCP in multi-node deployment)
                        for (_target, _msg) in &messages {
                            debug!("raft: outgoing message (would be sent via TCP in production)");
                        }

                        // Collect and forward committed commands
                        let mut hub = hub_ref.lock().unwrap();
                        let committed = hub.committed_commands();
                        hub.advance_applied(committed.len());
                        drop(hub);
                        for cmd in committed {
                            if applied_tx.try_send(cmd).is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                }
            }
        });

        info!(
            "raft runtime started (node={}, peers={:?})",
            config.node_id,
            config.peers
        );

        let runtime = Self {
            hub,
            cmd_tx: cmd_tx.clone(),
            applied_rx: StdMutex::new(applied_rx),
            shutdown_tx,
            is_leader,
            leader_id,
        };

        (runtime, cmd_tx)
    }

    /// Propose a command through Raft consensus.
    /// Only the leader can propose. Returns error if not leader.
    pub fn propose(&self, cmd: HubCommand) -> Result<(), suture_raft::RaftError> {
        let mut hub = self.hub.lock().unwrap();
        hub.propose(cmd)
    }

    /// Try to apply committed commands.
    /// Returns a list of commands that have been committed by Raft
    /// and are ready to be applied to HubStorage.
    pub fn try_apply_committed(&self) -> Vec<HubCommand> {
        let mut rx = self.applied_rx.lock().unwrap();
        let mut result = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            result.push(cmd);
        }
        result
    }

    /// Get the current leader ID (if known).
    pub fn leader(&self) -> Option<u64> {
        *self.leader_id.lock().unwrap()
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        *self.is_leader.lock().unwrap()
    }

    /// Get this node's Raft state.
    pub fn state(&self) -> NodeState {
        *self.hub.lock().unwrap().state()
    }

    /// Get this node's current term.
    pub fn term(&self) -> u64 {
        self.hub.lock().unwrap().term()
    }

    /// Shut down the runtime.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Get the node ID.
    pub fn node_id(&self) -> u64 {
        self.hub.lock().unwrap().node_id()
    }

    /// Get a reference to the command sender for proposing commands.
    pub fn cmd_sender(&self) -> mpsc::Sender<HubCommand> {
        self.cmd_tx.clone()
    }
}

impl Drop for RaftRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RaftConfig {
        RaftConfig {
            node_id: 1,
            peers: vec![],
            election_timeout: 10,
            heartbeat_interval: 3,
        }
    }

    #[tokio::test]
    async fn test_single_node_becomes_leader() {
        let (rt, _tx) = RaftRuntime::spawn(test_config());

        // Wait for election (single node should win immediately)
        tokio::time::sleep(Duration::from_millis(1500)).await;

        assert!(rt.is_leader(), "single node should be leader");
        assert_eq!(rt.leader(), Some(1));
    }

    #[tokio::test]
    async fn test_propose_and_apply() {
        let (rt, tx) = RaftRuntime::spawn(test_config());

        // Wait for election
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Propose a command
        tx.send(HubCommand::CreateRepo {
            repo_id: "test-repo".to_string(),
        })
        .await
        .unwrap();

        // Wait for commitment
        tokio::time::sleep(Duration::from_millis(500)).await;

        let applied = rt.try_apply_committed();
        assert_eq!(applied.len(), 1);
        assert_eq!(
            applied[0],
            HubCommand::CreateRepo {
                repo_id: "test-repo".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn test_shutdown() {
        let (rt, _tx) = RaftRuntime::spawn(test_config());
        tokio::time::sleep(Duration::from_millis(100)).await;
        rt.shutdown();
        // Should not panic on drop
    }
}
