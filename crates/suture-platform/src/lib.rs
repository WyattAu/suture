pub mod auth;
pub mod billing;
pub mod db;
pub mod merge_api;
pub mod middleware;
pub mod oauth;
pub mod orgs;
pub mod rate_limit;
pub mod server;
pub mod stripe;
pub mod web_ui;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub addr: String,
    pub db_path: String,
    pub hub_db_path: String,
    pub jwt_secret: String,
    pub stripe_key: Option<String>,
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    server::start(config).await
}
