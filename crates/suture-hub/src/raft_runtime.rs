use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use suture_raft::NodeState;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::raft::{HubCommand, RaftConfig, RaftHub};
use crate::raft_network::RaftTcpTransport;

/// Manages the Raft background task for consensus.
///
/// When spawned, runs a tick loop and provides channels for:
/// - Proposing commands (leader only)
/// - Receiving committed commands (for applying to storage)
/// - Checking leader status
///
/// When a `RaftTcpTransport` is provided, outgoing messages are sent over TCP
/// and incoming TCP messages are fed into the Raft node automatically.
pub struct RaftRuntime {
    hub: Arc<StdMutex<RaftHub>>,
    cmd_tx: mpsc::Sender<HubCommand>,
    applied_rx: StdMutex<mpsc::Receiver<HubCommand>>,
    shutdown_tx: broadcast::Sender<()>,
    is_leader: Arc<StdMutex<bool>>,
    leader_id: Arc<StdMutex<Option<u64>>>,
}

impl RaftRuntime {
    /// Spawn the Raft runtime without network transport (single-node mode).
    pub fn spawn(config: RaftConfig) -> (Self, mpsc::Sender<HubCommand>) {
        Self::spawn_inner(config, None)
    }

    /// Spawn the Raft runtime with TCP transport (multi-node mode).
    pub fn spawn_with_transport(
        config: RaftConfig,
        transport: Arc<RaftTcpTransport>,
    ) -> (Self, mpsc::Sender<HubCommand>) {
        Self::spawn_inner(config, Some(transport))
    }

    fn spawn_inner(
        config: RaftConfig,
        transport: Option<Arc<RaftTcpTransport>>,
    ) -> (Self, mpsc::Sender<HubCommand>) {
        let hub = Arc::new(StdMutex::new(RaftHub::new(config.clone())));
        let (cmd_tx, cmd_rx) = mpsc::channel::<HubCommand>(256);
        let (applied_tx, applied_rx) = mpsc::channel::<HubCommand>(256);
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

        let is_leader = Arc::new(StdMutex::new(false));
        let leader_id = Arc::new(StdMutex::new(None));
        let hub_ref = Arc::clone(&hub);
        let is_leader_ref = Arc::clone(&is_leader);
        let leader_id_ref = Arc::clone(&leader_id);
        let transport_for_tick = transport.clone();
        let has_transport = transport.is_some();

        // Background task: receive from TCP, handle in hub, send responses.
        // Must drop MutexGuard before any .await to satisfy Send bound.
        if let Some(ref trans) = transport {
            let hub_for_rx = Arc::clone(&hub);
            let trans_clone = Arc::clone(trans);
            let mut shutdown_sub = shutdown_tx.subscribe();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_sub.recv() => {
                            debug!("raft: tcp receiver task shutting down");
                            break;
                        }
                        result = trans_clone.receive() => {
                            match result {
                                Ok((from, msg)) => {
                                    // Lock hub, process message, get response, DROP lock, then send
                                    let responses = {
                                        let mut hub = hub_for_rx.lock().unwrap_or_else(|e| e.into_inner());
                                        hub.handle_message(from, msg)
                                    };
                                    for (target, resp) in responses {
                                        if let Err(e) = trans_clone.send_to_peer(target, resp).await
                                        {
                                            warn!("raft: failed to send response to {target}: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    debug!("raft: tcp receive error: {e}");
                                    break;
                                }
                            }
                        }
                    }
                }
            });
        }

        // Main tick loop
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
                        let mut hub = hub_ref.lock().unwrap_or_else(|e| e.into_inner());
                        match hub.propose(cmd) {
                            Ok(()) => debug!("raft: command proposed"),
                            Err(e) => warn!("raft: propose failed (not leader?): {e}"),
                        }
                    }
                    _ = tick_interval.tick() => {
                        // Lock hub, tick, collect state + messages, DROP lock
                        let (state, leader, messages) = {
                            let mut hub = hub_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let s = *hub.state();
                            let l = hub.leader();
                            let msgs = hub.tick();
                            (s, l, msgs)
                        };
                        // Update shared state (no lock needed, these are separate mutexes)
                        *is_leader_ref.lock().unwrap_or_else(|e| e.into_inner()) = state == NodeState::Leader;
                        *leader_id_ref.lock().unwrap_or_else(|e| e.into_inner()) = leader;

                        // Send outgoing messages via TCP (no hub lock held)
                        if let Some(ref trans) = transport_for_tick {
                            for (target, msg) in &messages {
                                if let Err(e) = trans.send_to_peer(*target, msg.clone()).await {
                                    debug!("raft: failed to send to peer {target}: {e}");
                                }
                            }
                        }

                        // Collect committed commands (brief lock)
                        let committed = {
                            let mut hub = hub_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let c = hub.committed_commands();
                            hub.advance_applied(c.len());
                            c.to_vec()
                        };
                        for cmd in committed {
                            if applied_tx.try_send(cmd).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });

        info!(
            "raft runtime started (node={}, peers={:?}, transport={})",
            config.node_id,
            config.peers,
            if has_transport { "tcp" } else { "none" }
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
    pub fn propose(&self, cmd: HubCommand) -> Result<(), suture_raft::RaftError> {
        let mut hub = self.hub.lock().unwrap_or_else(|e| e.into_inner());
        hub.propose(cmd)
    }

    /// Try to apply committed commands.
    pub fn try_apply_committed(&self) -> Vec<HubCommand> {
        let mut rx = self.applied_rx.lock().unwrap_or_else(|e| e.into_inner());
        let mut result = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            result.push(cmd);
        }
        result
    }

    /// Get the current leader ID (if known).
    pub fn leader(&self) -> Option<u64> {
        *self.leader_id.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        *self.is_leader.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Get this node's Raft state.
    pub fn state(&self) -> NodeState {
        *self.hub.lock().unwrap_or_else(|e| e.into_inner()).state()
    }

    /// Get this node's current term.
    pub fn term(&self) -> u64 {
        self.hub.lock().unwrap_or_else(|e| e.into_inner()).term()
    }

    /// Shut down the runtime.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Get the node ID.
    pub fn node_id(&self) -> u64 {
        self.hub.lock().unwrap_or_else(|e| e.into_inner()).node_id()
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
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(rt.is_leader(), "single node should be leader");
        assert_eq!(rt.leader(), Some(1));
    }

    #[tokio::test]
    async fn test_propose_and_apply() {
        let (rt, tx) = RaftRuntime::spawn(test_config());
        tokio::time::sleep(Duration::from_millis(1500)).await;
        tx.send(HubCommand::CreateRepo {
            repo_id: "test-repo".to_string(),
        })
        .await
        .unwrap();
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
    }

    /// Multi-node test: 3 nodes over real TCP, leader election + log replication.
    #[tokio::test]
    async fn test_three_node_cluster_over_tcp() {
        use std::collections::HashMap;
        use std::net::SocketAddr;

        // Bind 3 listeners on ephemeral ports
        let l1 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let l2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let l3 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a1 = l1.local_addr().unwrap();
        let a2 = l2.local_addr().unwrap();
        let a3 = l3.local_addr().unwrap();
        drop(l1);
        drop(l2);
        drop(l3); // free ports for transport listeners

        let p1: HashMap<u64, SocketAddr> = [(2, a2), (3, a3)].into_iter().collect();
        let p2: HashMap<u64, SocketAddr> = [(1, a1), (3, a3)].into_iter().collect();
        let p3: HashMap<u64, SocketAddr> = [(1, a1), (2, a2)].into_iter().collect();

        let t1 = Arc::new(RaftTcpTransport::new(1, p1));
        let t2 = Arc::new(RaftTcpTransport::new(2, p2));
        let t3 = Arc::new(RaftTcpTransport::new(3, p3));

        t1.listen(a1).await.unwrap();
        t2.listen(a2).await.unwrap();
        t3.listen(a3).await.unwrap();

        let c = |id: u64, peers: Vec<u64>| RaftConfig {
            node_id: id,
            peers,
            election_timeout: 10,
            heartbeat_interval: 3,
        };
        let (rt1, _) = RaftRuntime::spawn_with_transport(c(1, vec![2, 3]), Arc::clone(&t1));
        let (rt2, _) = RaftRuntime::spawn_with_transport(c(2, vec![1, 3]), Arc::clone(&t2));
        let (rt3, _) = RaftRuntime::spawn_with_transport(c(3, vec![1, 2]), Arc::clone(&t3));

        // Wait for leader election
        tokio::time::sleep(Duration::from_secs(3)).await;

        let leader = if rt1.is_leader() {
            &rt1 as &RaftRuntime
        } else if rt2.is_leader() {
            &rt2 as &RaftRuntime
        } else if rt3.is_leader() {
            &rt3 as &RaftRuntime
        } else {
            panic!(
                "no leader: n1={:?} n2={:?} n3={:?}",
                rt1.state(),
                rt2.state(),
                rt3.state()
            );
        };

        leader
            .propose(HubCommand::CreateRepo {
                repo_id: "cluster-test".to_string(),
            })
            .expect("propose on leader");

        tokio::time::sleep(Duration::from_secs(2)).await;

        let applied = leader.try_apply_committed();
        assert!(!applied.is_empty(), "leader should have committed commands");
        assert_eq!(
            applied[0],
            HubCommand::CreateRepo {
                repo_id: "cluster-test".to_string(),
            }
        );

        rt1.shutdown();
        rt2.shutdown();
        rt3.shutdown();
    }
}
