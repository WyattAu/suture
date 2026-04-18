use clap::Parser;
use suture_hub::SutureHubServer;

#[derive(Parser)]
#[command(
    name = "suture-hub",
    version,
    about = "Suture Hub — distributed patch sync server"
)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:50051")]
    addr: String,

    /// Path to SQLite database file. Uses in-memory storage if omitted.
    #[arg(long)]
    db: Option<String>,

    /// Replication role: leader, follower, or standalone (default).
    /// Leader pushes replication log to followers periodically.
    /// Followers accept replication entries from the leader.
    #[arg(long, default_value = "standalone")]
    replication_role: String,

    #[arg(long, env = "SUTURE_BLOB_BACKEND", default_value = "sqlite")]
    blob_backend: String,

    #[arg(long, env = "S3_ENDPOINT")]
    s3_endpoint: Option<String>,

    #[arg(long, env = "S3_BUCKET")]
    s3_bucket: Option<String>,

    #[arg(long, env = "S3_REGION", default_value = "us-east-1")]
    s3_region: String,

    #[arg(long, env = "S3_ACCESS_KEY")]
    s3_access_key: Option<String>,

    #[arg(long, env = "S3_SECRET_KEY")]
    s3_secret_key: Option<String>,

    #[arg(long, env = "S3_PREFIX", default_value = "suture/blobs/")]
    s3_prefix: String,

    // Raft consensus flags (gated on raft-cluster feature)
    #[cfg(feature = "raft-cluster")]
    #[arg(long)]
    /// Enable Raft consensus for high-availability clustering.
    raft: bool,

    #[cfg(feature = "raft-cluster")]
    #[arg(long, requires = "raft")]
    /// This node's unique Raft ID (required with --raft).
    raft_node_id: Option<u64>,

    #[cfg(feature = "raft-cluster")]
    #[arg(long, requires = "raft", value_name = "ID:ADDR,...")]
    /// Raft peer addresses as id:addr pairs, comma-separated.
    /// Example: "2:127.0.0.1:9002,3:127.0.0.1:9003"
    raft_peers: Option<String>,

    #[cfg(feature = "raft-cluster")]
    #[arg(long, default_value_t = 7946)]
    /// Port for Raft TCP transport.
    raft_port: u16,

    #[cfg(feature = "raft-cluster")]
    #[arg(long, default_value_t = 10)]
    /// Raft election timeout in ticks.
    raft_election_timeout: u64,

    #[cfg(feature = "raft-cluster")]
    #[arg(long, default_value_t = 3)]
    /// Raft heartbeat interval in ticks.
    raft_heartbeat_interval: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    #[allow(unused_mut)]
    let mut hub = if let Some(db_path) = args.db {
        SutureHubServer::with_db(std::path::Path::new(&db_path))?
    } else {
        SutureHubServer::new()
    };

    hub.set_replication_role(&args.replication_role);

    // S3 blob backend setup
    if args.blob_backend == "s3" {
        #[cfg(feature = "s3-backend")]
        {
            let endpoint = args
                .s3_endpoint
                .as_deref()
                .ok_or("--s3-endpoint is required when --blob-backend s3")?;
            let bucket = args
                .s3_bucket
                .as_deref()
                .ok_or("--s3-bucket is required when --blob-backend s3")?;
            let access_key = args
                .s3_access_key
                .as_deref()
                .ok_or("--s3-access-key is required when --blob-backend s3")?;
            let secret_key = args
                .s3_secret_key
                .as_deref()
                .ok_or("--s3-secret-key is required when --blob-backend s3")?;

            let config = suture_s3::S3Config {
                endpoint: endpoint.to_string(),
                bucket: bucket.to_string(),
                region: args.s3_region.clone(),
                access_key: access_key.to_string(),
                secret_key: secret_key.to_string(),
                prefix: args.s3_prefix.clone(),
                force_path_style: true,
            };
            let validation_err = match config.validate() {
                Ok(()) => None,
                Err(e) => {
                    let msg = format!("invalid S3 config: {e}");
                    Some(msg)
                }
            };
            if let Some(err) = validation_err {
                return Err(err.into());
            }

            let adapter = suture_hub::blob_backend::S3BlobBackendAdapter::new(config);
            hub.set_blob_backend(std::sync::Arc::new(adapter));
            tracing::info!(
                "blob backend: s3 (endpoint={}, bucket={}, prefix={})",
                endpoint,
                bucket,
                args.s3_prefix
            );
        }
        #[cfg(not(feature = "s3-backend"))]
        {
            return Err(
                "s3-backend feature is not enabled; rebuild with --features s3-backend".into(),
            );
        }
    } else {
        tracing::info!("blob backend: sqlite");
    }

    // Raft consensus setup
    #[cfg(feature = "raft-cluster")]
    if args.raft {
        use suture_hub::raft_network::RaftTcpTransport;
        use suture_hub::raft_runtime::RaftRuntime;
        use suture_hub::raft::RaftConfig;
        use std::collections::HashMap;
        use std::net::SocketAddr;

        let node_id = args
            .raft_node_id
            .ok_or("--raft-node-id is required when --raft is set")?;

        // Parse peers: "2:127.0.0.1:9002,3:127.0.0.1:9003"
        let mut peers = Vec::new();
        let mut peer_addrs = HashMap::new();

        if let Some(ref peers_str) = args.raft_peers {
            for pair in peers_str.split(',') {
                let pair = pair.trim();
                let parts: Vec<&str> = pair.splitn(2, ':').collect();
                if parts.len() != 2 {
                    let msg = format!(
                        "invalid raft peer format '{pair}', expected ID:ADDR"
                    );
                    return Err(msg.into());
                }
                let id_str = parts[0];
                let addr_str = parts[1];

                let peer_id: u64 = match id_str.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        let msg = format!(
                            "invalid raft peer ID '{id_str}', expected integer"
                        );
                        return Err(msg.into());
                    }
                };

                let peer_addr: SocketAddr = match addr_str.parse() {
                    Ok(a) => a,
                    Err(_) => {
                        let msg = format!("invalid raft peer address '{addr_str}'");
                        return Err(msg.into());
                    }
                };

                peers.push(peer_id);
                peer_addrs.insert(peer_id, peer_addr);
            }
        }

        let config = RaftConfig {
            node_id,
            peers: peers.clone(),
            election_timeout: args.raft_election_timeout,
            heartbeat_interval: args.raft_heartbeat_interval,
        };

        let transport = RaftTcpTransport::new(node_id, peer_addrs);
        let bind_addr = format!("0.0.0.0:{}", args.raft_port);
        let raft_addr: SocketAddr = match bind_addr.parse() {
            Ok(a) => a,
            Err(_) => {
                let msg = format!("invalid raft bind address: {bind_addr}");
                return Err(msg.into());
            }
        };
        transport.listen(raft_addr).await?;
        tracing::info!(
            "raft: node {} listening on port {}, peers={:?}",
            node_id,
            args.raft_port,
            peers
        );

        let (_runtime, _cmd_tx) = RaftRuntime::spawn(config);
        tracing::info!("raft runtime started (single-node mode; TCP transport ready for multi-node)");
    }

    suture_hub::server::run_server(hub, &args.addr).await?;

    Ok(())
}
