// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use axum::{routing::get, Json, Router};
use tracing::debug;

use crate::{health::HealthResponse, DEFAULT_PORT};

pub fn get_mysten_service<S>(app_name: &str, app_version: &str) -> Router<S>
where
    S: Send + Clone + Sync + 'static,
{
    // build our application with a single route
    Router::new().route(
        "/health",
        get(Json(HealthResponse::new(app_name, app_version))),
    )
}

pub async fn serve(app: Router) -> Result<()> {
    // run it with hyper on localhost:3000
    debug!("listening on http://localhost:{}", DEFAULT_PORT);
    axum::Server::bind(&format!("0.0.0.0:{}", DEFAULT_PORT).parse()?)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
