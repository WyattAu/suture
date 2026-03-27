use clap::Parser;
use suture_hub::SutureHubServer;

#[derive(Parser)]
#[command(name = "suture-hub", version, about = "Suture Hub — distributed patch sync server")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:50051")]
    addr: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let hub = SutureHubServer::new();
    suture_hub::server::run_server(hub, &args.addr).await?;

    Ok(())
}
