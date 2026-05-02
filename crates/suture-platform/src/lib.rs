// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

pub mod analytics;
pub mod auth;
pub mod billing;
pub mod db;
pub mod merge_api;
pub mod middleware;
pub mod oauth;
pub mod orgs;
pub mod plugins;
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
    #[serde(default)]
    pub platform_url: String,
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    server::start(config).await
}
