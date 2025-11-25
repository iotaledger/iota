// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{convert::Infallible, net::SocketAddr, sync::Arc};

use anyhow::Result;
use http_body_util::{BodyExt, Full};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header::CONTENT_TYPE,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::{cache::BuildCache, types::BuildRequest};

/// Helper function to create a bad request response
fn bad_request(msg: impl Into<String>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from(msg.into())))
        .unwrap()
}

/// Helper function to create an internal server error response
fn internal_error(msg: impl Into<String>) -> Response<Full<Bytes>> {
    let msg = msg.into();
    error!("{msg}");
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from(msg)))
        .unwrap()
}

/// Helper function to create a not found response
fn not_found(msg: impl Into<String>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from(msg.into())))
        .unwrap()
}

/// Helper function to create a JSON success response
fn json_response(data: impl serde::Serialize, code: StatusCode) -> Response<Full<Bytes>> {
    let json = serde_json::to_string(&data).unwrap();
    Response::builder()
        .status(code)
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap()
}

/// HTTP server for the build cache
pub struct BuildCacheServer {
    cache: Arc<BuildCache>,
}

impl BuildCacheServer {
    /// Create a new build cache server
    pub fn new(cache_dir: String, workspace_dir: String, repository_url: String) -> Result<Self> {
        let cache = BuildCache::new(cache_dir, workspace_dir, repository_url)?;

        Ok(Self {
            cache: Arc::new(cache),
        })
    }

    /// Run the HTTP server
    pub async fn run(&self, addr: SocketAddr) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("Build cache server listening on {addr}");

        loop {
            let (stream, _) = listener.accept().await?;
            let cache = Arc::clone(&self.cache);

            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                let service = service_fn(move |req| {
                    let cache = Arc::clone(&cache);
                    handle_request(req, cache)
                });

                if let Err(err) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .await
                {
                    error!("Error serving connection: {err:?}");
                }
            });
        }
    }
}

/// Handle HTTP requests
async fn handle_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method();
    let path = req.uri().path();

    info!("{method} {path}");

    let response = match (method, path) {
        // GET /resolve/{commit}
        (&Method::GET, "/resolve/{commit}") => handle_resolve_request(req, cache).await,

        // GET /check/{commit}/{cpu_target}?binaries=bin1,bin2,bin3
        (&Method::GET, path) if path.starts_with("/check/") => {
            handle_check_request(req, cache).await
        }

        // GET /download/{commit}/{cpu_target}/{binary_name}
        (&Method::GET, path) if path.starts_with("/download/") => {
            handle_download_request(req, cache).await
        }

        // POST /build
        (&Method::POST, "/build") => handle_build_request(req, cache).await,

        // GET /status/{commit}/{cpu_target}?binaries=bin1,bin2,bin3
        (&Method::GET, path) if path.starts_with("/status/") => {
            handle_status_request(req, cache).await
        }

        // Health check
        (&Method::GET, "/health") => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/plain")
            .body(Full::new(Bytes::from("OK")))
            .unwrap()),

        _ => Ok(not_found("Not Found")),
    };

    response
}

async fn handle_resolve_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() < 3 {
        return Ok(bad_request("Invalid path format"));
    }

    let commit_ref = parts[2];

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(commit_ref).await {
        Ok(resolved_commit) => Ok(json_response(resolved_commit, StatusCode::OK)),
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(bad_request(format!("Invalid commit/branch/tag: {e}")))
        }
    }
}

/// Handle binary availability check requests
async fn handle_check_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() < 4 {
        return Ok(bad_request("Invalid path format"));
    }

    let commit_ref = parts[2];
    let cpu_target = parts[3];

    // Parse query parameters for binaries
    let query = req.uri().query().unwrap_or("");
    let binaries = parse_binaries_from_query(query);

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(commit_ref).await {
        Ok(resolved_commit) => {
            let response = cache
                .check_binaries(&resolved_commit, cpu_target, &binaries)
                .await;
            Ok(json_response(response, StatusCode::OK))
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(bad_request(format!("Invalid commit/branch/tag: {e}")))
        }
    }
}

/// Handle binary download requests
async fn handle_download_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() < 5 {
        return Ok(bad_request("Invalid path format"));
    }

    let commit_ref = parts[2];
    let cpu_target = parts[3];
    let binary_name = parts[4];

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(commit_ref).await {
        Ok(resolved_commit) => {
            match cache
                .get_binary_data(&resolved_commit, cpu_target, binary_name)
                .await
            {
                Ok(data) => Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "application/octet-stream")
                    .header("x-iota-build-commit-hash", resolved_commit)
                    .body(Full::new(Bytes::from(data)))
                    .unwrap()),
                Err(e) => {
                    warn!("Binary not found: {e}");
                    Ok(not_found(format!("Binary not found: {e}")))
                }
            }
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(bad_request(format!("Invalid commit/branch/tag: {e}")))
        }
    }
}

/// Handle build requests
async fn handle_build_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let body = req.collect().await.unwrap().to_bytes();

    // Parse the request body as BuildRequest
    match serde_json::from_slice::<BuildRequest>(&body) {
        // Resolve branch/tag/commit to actual commit hash
        Ok(build_request) => match cache.resolve_commit(build_request.commit.as_str()).await {
            Ok(resolved_commit) => {
                // Start the build
                match cache
                    .start_build(
                        &resolved_commit,
                        &build_request.cpu_target,
                        &build_request.binaries,
                    )
                    .await
                {
                    Ok(build_response) => Ok(json_response(build_response, StatusCode::ACCEPTED)),
                    Err(e) => Ok(internal_error(format!("Failed to start build: {e}"))),
                }
            }
            Err(e) => {
                error!("Failed to resolve commit '{}': {e}", build_request.commit);
                Ok(bad_request(format!("Invalid commit/branch/tag: {e}")))
            }
        },
        Err(e) => {
            warn!("Invalid build request: {e}");
            Ok(bad_request(format!("Invalid request: {e}")))
        }
    }
}

/// Handle build status requests
async fn handle_status_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() < 4 {
        return Ok(bad_request("Invalid path format"));
    }

    let commit_ref = parts[2];
    let cpu_target = parts[3];

    // Parse query parameters for binaries
    let query = req.uri().query().unwrap_or("");
    let binaries = parse_binaries_from_query(query);

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(commit_ref).await {
        Ok(resolved_commit) => {
            match cache
                .get_build_status(&resolved_commit, cpu_target, &binaries)
                .await
            {
                Some(status) => Ok(json_response(status, StatusCode::OK)),
                None => Ok(not_found("Build status not found")),
            }
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(bad_request(format!("Invalid commit/branch/tag: {e}")))
        }
    }
}

/// Parse binaries from query string
fn parse_binaries_from_query(query: &str) -> Vec<String> {
    for part in query.split('&') {
        if let Some((key, value)) = part.split_once('=') {
            if key == "binaries" {
                return value.split(',').map(|s| s.to_string()).collect();
            }
        }
    }
    vec![]
}
