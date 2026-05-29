//! TCP-based transport for Raft consensus messages.
//!
//! Wire format: 4-byte big-endian length prefix + JSON [`WireFrame`].
//! Each frame includes the sender's node ID and the Raft message payload.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use suture_raft::{RaftError, RaftMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Wire frame: sender node ID + Raft message.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireFrame {
    from: u64,
    message: RaftMessage,
}

/// TCP-based Raft transport.
///
/// Provides `listen()` to accept incoming messages and `send_to_peer()` to
/// send messages to specific peers. Peer addresses must be registered at
/// construction time.
pub struct RaftTcpTransport {
    node_id: u64,
    peers: tokio::sync::Mutex<HashMap<u64, SocketAddr>>,
    recv_rx: tokio::sync::Mutex<mpsc::Receiver<(u64, RaftMessage)>>,
    recv_tx: mpsc::Sender<(u64, RaftMessage)>,
    connected: tokio::sync::Mutex<HashSet<u64>>,
}

impl RaftTcpTransport {
    pub fn new(node_id: u64, peers: HashMap<u64, SocketAddr>) -> Self {
        let (recv_tx, recv_rx) = mpsc::channel(256);
        Self {
            node_id,
            peers: tokio::sync::Mutex::new(peers),
            recv_rx: tokio::sync::Mutex::new(recv_rx),
            recv_tx,
            connected: tokio::sync::Mutex::new(HashSet::new()),
        }
    }

    pub async fn listen(&self, addr: SocketAddr) -> Result<SocketAddr, RaftError> {
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let msg = format!("failed to bind TCP listener on {addr}: {e}");
                return Err(RaftError::Transport(msg));
            }
        };

        let local_addr = match listener.local_addr() {
            Ok(a) => a,
            Err(e) => {
                let msg = format!("failed to get local address: {e}");
                return Err(RaftError::Transport(msg));
            }
        };

        let recv_tx = self.recv_tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _peer_addr)) => {
                        let tx = recv_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_incoming(stream, tx).await {
                                warn!("raft tcp handler error: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        warn!("raft tcp accept error: {e}");
                    }
                }
            }
        });

        debug!(
            node = self.node_id,
            addr = %local_addr,
            "raft tcp transport listening"
        );
        Ok(local_addr)
    }

    pub async fn send_to_peer(&self, target: u64, message: RaftMessage) -> Result<(), RaftError> {
        let addr = {
            let peers = self.peers.lock().await;
            match peers.get(&target) {
                Some(a) => *a,
                None => {
                    let msg = format!("no address registered for raft peer {target}");
                    return Err(RaftError::Transport(msg));
                }
            }
        };

        let frame = WireFrame {
            from: self.node_id,
            message,
        };
        match send_wire(&addr, &frame).await {
            Ok(()) => {
                self.connected.lock().await.insert(target);
                Ok(())
            }
            Err(first_err) => {
                self.connected.lock().await.remove(&target);
                info!(
                    node = self.node_id,
                    target, "raft: send failed, retrying once"
                );
                match send_wire(&addr, &frame).await {
                    Ok(()) => {
                        self.connected.lock().await.insert(target);
                        Ok(())
                    }
                    Err(_) => {
                        let msg = format!("send to peer {target} failed after retry: {first_err}");
                        Err(RaftError::Transport(msg))
                    }
                }
            }
        }
    }

    pub async fn receive(&self) -> Result<(u64, RaftMessage), RaftError> {
        let mut rx = self.recv_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| RaftError::Transport("receive channel closed".to_string()))
    }

    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    pub async fn peer_count(&self) -> usize {
        self.peers.lock().await.len()
    }

    pub fn is_connected(&self, node_id: u64) -> bool {
        self.connected.blocking_lock().contains(&node_id)
    }

    pub async fn reconnect(&self, node_id: u64) -> Result<(), RaftError> {
        let addr = {
            let peers = self.peers.lock().await;
            match peers.get(&node_id) {
                Some(a) => *a,
                None => {
                    let msg = format!("no address registered for raft peer {node_id}");
                    return Err(RaftError::Transport(msg));
                }
            }
        };

        match TcpStream::connect(addr).await {
            Ok(stream) => {
                drop(stream);
                self.connected.lock().await.insert(node_id);
                debug!(
                    node = self.node_id,
                    target = node_id,
                    "raft: reconnected to peer"
                );
                Ok(())
            }
            Err(e) => {
                self.connected.lock().await.remove(&node_id);
                let msg = format!("reconnect to peer {node_id} at {addr} failed: {e}");
                Err(RaftError::Transport(msg))
            }
        }
    }

    pub async fn add_peer(&self, node_id: u64, addr: SocketAddr) {
        self.peers.lock().await.insert(node_id, addr);
    }

    pub async fn remove_peer(&self, node_id: u64) {
        self.peers.lock().await.remove(&node_id);
        self.connected.lock().await.remove(&node_id);
    }
}

async fn send_wire(addr: &SocketAddr, frame: &WireFrame) -> Result<(), RaftError> {
    let mut stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("failed to connect to raft peer at {addr}: {e}");
            return Err(RaftError::Transport(msg));
        }
    };

    let json = match serde_json::to_vec(frame) {
        Ok(j) => j,
        Err(e) => {
            let msg = format!("failed to serialize raft message: {e}");
            return Err(RaftError::Transport(msg));
        }
    };

    let len = json.len() as u32;
    if let Err(e) = stream.write_all(&len.to_be_bytes()).await {
        let msg = format!("failed to write length prefix: {e}");
        return Err(RaftError::Transport(msg));
    }
    if let Err(e) = stream.write_all(&json).await {
        let msg = format!("failed to write raft message: {e}");
        return Err(RaftError::Transport(msg));
    }
    if let Err(e) = stream.flush().await {
        let msg = format!("failed to flush raft message: {e}");
        return Err(RaftError::Transport(msg));
    }

    debug!(addr = %addr, len = json.len(), "raft message sent via tcp");
    Ok(())
}

async fn handle_incoming(
    mut stream: TcpStream,
    tx: mpsc::Sender<(u64, RaftMessage)>,
) -> Result<(), RaftError> {
    loop {
        let mut len_buf = [0u8; 4];
        if let Err(e) = stream.read_exact(&mut len_buf).await {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Err(RaftError::Transport(
                    "connection closed by peer".to_string(),
                ));
            }
            let msg = format!("failed to read length prefix: {e}");
            return Err(RaftError::Transport(msg));
        }
        let len = u32::from_be_bytes(len_buf);

        if len == 0 || len > 64 * 1024 * 1024 {
            let msg = format!("invalid frame length: {len}");
            return Err(RaftError::Transport(msg));
        }

        let mut buf = vec![0u8; len as usize];
        if let Err(e) = stream.read_exact(&mut buf).await {
            let msg = format!("failed to read frame payload: {e}");
            return Err(RaftError::Transport(msg));
        }

        let frame: WireFrame = match serde_json::from_slice(&buf) {
            Ok(f) => f,
            Err(e) => {
                let msg = format!("failed to deserialize raft message: {e}");
                return Err(RaftError::Transport(msg));
            }
        };

        debug!(from = frame.from, "raft message received via tcp");

        if tx.send((frame.from, frame.message)).await.is_err() {
            // Receiver dropped, transport is shutting down
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suture_raft::RaftMessage;

    #[test]
    fn test_wire_frame_serialization_roundtrip() {
        let frame = WireFrame {
            from: 42,
            message: RaftMessage::RequestVoteRequest {
                term: 5,
                candidate_id: 42,
                last_log_index: 10,
                last_log_term: 5,
                is_pre_vote: false,
            },
        };

        let json = serde_json::to_vec(&frame).expect("serialize");
        let decoded: WireFrame = serde_json::from_slice(&json).expect("deserialize");

        assert_eq!(decoded.from, 42);
        assert_eq!(frame.message, decoded.message);
    }

    #[test]
    fn test_wire_frame_all_message_types() {
        let messages = vec![
            RaftMessage::AppendEntriesRequest {
                term: 1,
                leader_id: 1,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
            RaftMessage::AppendEntriesResponse {
                term: 1,
                success: true,
                match_index: 5,
            },
            RaftMessage::RequestVoteRequest {
                term: 2,
                candidate_id: 3,
                last_log_index: 7,
                last_log_term: 2,
                is_pre_vote: false,
            },
            RaftMessage::RequestVoteResponse {
                term: 2,
                vote_granted: false,
                is_pre_vote: false,
            },
        ];

        for message in messages {
            let frame = WireFrame {
                from: 1,
                message: message.clone(),
            };
            let json = serde_json::to_vec(&frame).expect("serialize");
            let decoded: WireFrame = serde_json::from_slice(&json).expect("deserialize");
            assert_eq!(decoded.from, 1);
            assert_eq!(
                decoded.message, message,
                "roundtrip failed for {:?}",
                message
            );
        }
    }

    #[test]
    fn test_transport_creation() {
        let mut peers = HashMap::new();
        peers.insert(2, "127.0.0.1:9002".parse().unwrap());
        peers.insert(3, "127.0.0.1:9003".parse().unwrap());

        let transport = RaftTcpTransport::new(1, peers);
        assert_eq!(transport.node_id(), 1);
    }

    #[tokio::test]
    async fn test_send_to_unknown_peer() {
        let transport = RaftTcpTransport::new(1, HashMap::new());

        let result = transport
            .send_to_peer(
                99,
                RaftMessage::RequestVoteRequest {
                    term: 1,
                    candidate_id: 1,
                    last_log_index: 0,
                    last_log_term: 0,
                    is_pre_vote: false,
                },
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            RaftError::Transport(msg) => {
                assert!(msg.contains("99"), "error should mention peer ID: {msg}");
            }
            other => panic!("expected Transport error, got: {other:?}"),
        }
    }
}
