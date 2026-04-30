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

    #[error("invalid log index: {0}")]
    InvalidIndex(u64),

    #[error("membership change already in progress")]
    MembershipChangeInProgress,

    #[error("not a pre-candidate")]
    NotPreCandidate,

    #[error("no membership transition in progress")]
    NoTransition,
}
