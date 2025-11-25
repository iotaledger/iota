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

        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(CONTENT_TYPE, "text/plain")
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap()),
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
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "text/plain")
            .body(Full::new(Bytes::from("Invalid path format")))
            .unwrap());
    }

    let commit_ref = parts[2];

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(commit_ref).await {
        Ok(resolved_commit) => {
            let json = serde_json::to_string(&resolved_commit).unwrap();
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(json)))
                .unwrap())
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(CONTENT_TYPE, "text/plain")
                .body(Full::new(Bytes::from(format!(
                    "Invalid commit/branch/tag: {e}"
                ))))
                .unwrap())
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
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "text/plain")
            .body(Full::new(Bytes::from("Invalid path format")))
            .unwrap());
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
            let json = serde_json::to_string(&response).unwrap();

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(json)))
                .unwrap())
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(CONTENT_TYPE, "text/plain")
                .body(Full::new(Bytes::from(format!(
                    "Invalid commit/branch/tag: {e}"
                ))))
                .unwrap())
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
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "text/plain")
            .body(Full::new(Bytes::from("Invalid path format")))
            .unwrap());
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
                    Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .header(CONTENT_TYPE, "text/plain")
                        .body(Full::new(Bytes::from(format!("Binary not found: {e}"))))
                        .unwrap())
                }
            }
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(CONTENT_TYPE, "text/plain")
                .body(Full::new(Bytes::from(format!(
                    "Invalid commit/branch/tag: {e}"
                ))))
                .unwrap())
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
                    Ok(build_response) => {
                        let json = serde_json::to_string(&build_response).unwrap();
                        Ok(Response::builder()
                            .status(StatusCode::ACCEPTED)
                            .header(CONTENT_TYPE, "application/json")
                            .body(Full::new(Bytes::from(json)))
                            .unwrap())
                    }
                    Err(e) => {
                        error!("Failed to start build: {e}");
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .header(CONTENT_TYPE, "text/plain")
                            .body(Full::new(Bytes::from(format!(
                                "Failed to start build: {e}",
                            ))))
                            .unwrap())
                    }
                }
            }
            Err(e) => {
                error!("Failed to resolve commit '{}': {e}", build_request.commit);
                Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(CONTENT_TYPE, "text/plain")
                    .body(Full::new(Bytes::from(format!(
                        "Invalid commit/branch/tag: {e}"
                    ))))
                    .unwrap())
            }
        },
        Err(e) => {
            warn!("Invalid build request: {e}");
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(CONTENT_TYPE, "text/plain")
                .body(Full::new(Bytes::from(format!("Invalid request: {e}"))))
                .unwrap())
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
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, "text/plain")
            .body(Full::new(Bytes::from("Invalid path format")))
            .unwrap());
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
                Some(status) => {
                    let json = serde_json::to_string(&status).unwrap();
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, "application/json")
                        .body(Full::new(Bytes::from(json)))
                        .unwrap())
                }
                None => Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(CONTENT_TYPE, "text/plain")
                    .body(Full::new(Bytes::from("Build status not found")))
                    .unwrap()),
            }
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(CONTENT_TYPE, "text/plain")
                .body(Full::new(Bytes::from(format!(
                    "Invalid commit/branch/tag: {e}"
                ))))
                .unwrap())
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
