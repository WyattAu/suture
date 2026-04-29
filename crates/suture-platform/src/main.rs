// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "suture-platform", about = "Hosted Suture platform")]
struct Args {
    /// Listen address
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,

    /// Database path (SQLite)
    #[arg(long, default_value = "platform.db")]
    db: String,

    /// Hub database path
    #[arg(long, default_value = "hub.db")]
    hub_db: String,

    /// JWT secret (required in production)
    #[arg(long)]
    jwt_secret: Option<String>,

    /// Stripe secret key (enables billing)
    #[arg(long)]
    stripe_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,suture_platform=debug".into()),
        )
        .init();

    let args = Args::parse();
    info!("Starting Suture Platform on {}", args.addr);

    suture_platform::run(suture_platform::Config {
        addr: args.addr,
        db_path: args.db,
        hub_db_path: args.hub_db,
        jwt_secret: args.jwt_secret.unwrap_or_else(|| "dev-secret-change-me".into()),
        stripe_key: args.stripe_key,
    })
    .await?;

    Ok(())
}
