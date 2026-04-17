use thiserror::Error;

#[derive(Error, Debug)]
pub enum RaftError {
    #[error("not the leader")]
    NotLeader,

    #[error("transport error: {0}")]
    Transport(String),

    #[error("log error: {0}")]
    Log(String),

    #[error("stale term: current={current}, received={received}")]
    StaleTerm { current: u64, received: u64 },

    #[error("node {0} not found")]
    NodeNotFound(u64),
}
