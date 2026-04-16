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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let hub = if let Some(db_path) = args.db {
        SutureHubServer::with_db(std::path::Path::new(&db_path))?
    } else {
        SutureHubServer::new()
    };

    hub.set_replication_role(&args.replication_role);

    suture_hub::server::run_server(hub, &args.addr).await?;

    Ok(())
}
