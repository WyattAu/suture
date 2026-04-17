use async_trait::async_trait;

use crate::error::RaftError;
use crate::message::RaftMessage;

#[async_trait]
pub trait RaftTransport: Send + Sync {
    async fn send(&self, target: u64, message: RaftMessage) -> Result<(), RaftError>;
    async fn receive(&self) -> Result<(u64, RaftMessage), RaftError>;
}
