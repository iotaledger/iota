// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{convert::Infallible, net::SocketAddr, sync::Arc};

use anyhow::Result;
use futures::StreamExt;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Frame, Incoming},
    header::{CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;
use tracing::{error, info, warn};

// Type alias for response body - either Full for small responses or streaming
// for large files
type ResponseBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

use crate::{cache::BuildCache, types::BuildRequest};

fn full_body_response(
    data: impl Into<String>,
    content_type: &str,
    code: StatusCode,
) -> Response<ResponseBody> {
    let bytes = Bytes::from(data.into());
    Response::builder()
        .status(code)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, bytes.len().to_string())
        .body(Full::new(bytes).map_err(|never| match never {}).boxed())
        .unwrap()
}

/// Helper function to create a text response
fn text_response(msg: impl Into<String>, code: StatusCode) -> Response<ResponseBody> {
    full_body_response(msg, "text/plain", code)
}

/// Helper function to create a bad request response
fn bad_request(msg: impl Into<String>) -> Response<ResponseBody> {
    text_response(msg, StatusCode::BAD_REQUEST)
}

/// Helper function to create an internal server error response
fn internal_error(msg: impl Into<String>) -> Response<ResponseBody> {
    let msg = msg.into();
    error!("{msg}");
    text_response(msg, StatusCode::INTERNAL_SERVER_ERROR)
}

/// Helper function to create a not found response
fn not_found(msg: impl Into<String>) -> Response<ResponseBody> {
    text_response(msg, StatusCode::NOT_FOUND)
}

/// Helper function to create a JSON success response
fn json_response(data: impl serde::Serialize, code: StatusCode) -> Response<ResponseBody> {
    let json = serde_json::to_string(&data).unwrap();
    full_body_response(json, "application/json", code)
}

/// HTTP server for the build cache
pub struct BuildCacheServer {
    cache: Arc<BuildCache>,
}

impl BuildCacheServer {
    /// Create a new build cache server
    pub fn new(
        cache_dir: String,
        workspace_dir: String,
        repository_url: String,
        allowed_cpu_targets: Vec<String>,
        max_cached_commits: usize,
        max_workspace_size_gb: u64,
    ) -> Result<Self> {
        let cache = BuildCache::new(
            cache_dir,
            workspace_dir,
            repository_url,
            allowed_cpu_targets,
            max_cached_commits,
            max_workspace_size_gb,
        )?;

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
) -> Result<Response<ResponseBody>, Infallible> {
    let method = req.method();
    let path = req.uri().path();

    info!("{method} {path}");

    let response = match (method, path) {
        // GET /resolve?commit=<commit>
        (&Method::GET, "/resolve") => handle_resolve_request(req, cache).await,

        // GET /check?commit=<commit>&cpu_target=<target>&binaries=bin1,bin2,bin3
        (&Method::GET, "/check") => handle_check_request(req, cache).await,

        // GET /download?commit=<commit>&cpu_target=<target>&binary=<name>
        (&Method::GET, "/download") => handle_download_request(req, cache).await,

        // POST /build
        (&Method::POST, "/build") => handle_build_request(req, cache).await,

        // GET /status?commit=<commit>&cpu_target=<target>&binaries=bin1,bin2,bin3
        (&Method::GET, "/status") => handle_status_request(req, cache).await,

        // Health check
        (&Method::GET, "/health") => Ok(text_response("OK", StatusCode::OK)),

        _ => Ok(not_found("Not Found")),
    };

    response
}

async fn handle_resolve_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<ResponseBody>, Infallible> {
    let query_params = parse_query_params(req.uri());
    let commit_ref = match get_commit_param(&query_params) {
        Ok(commit) => commit,
        Err(response) => return Ok(response),
    };

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(&commit_ref).await {
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
) -> Result<Response<ResponseBody>, Infallible> {
    // Parse query parameters
    let query_params = parse_query_params(req.uri());
    let commit_ref = match get_commit_param(&query_params) {
        Ok(commit) => commit,
        Err(response) => return Ok(response),
    };
    let cpu_target = match get_cpu_target_param(&query_params) {
        Ok(target) => target,
        Err(response) => return Ok(response),
    };
    let toolchain = get_toolchain_param(&query_params);
    let features = get_features_param(&query_params);
    let binaries = match get_binaries_param(&query_params) {
        Ok(binaries) => binaries,
        Err(response) => return Ok(response),
    };

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(&commit_ref).await {
        Ok(resolved_commit) => {
            match cache
                .check_binaries(
                    &resolved_commit,
                    &cpu_target,
                    toolchain.as_deref(),
                    &features,
                    &binaries,
                )
                .await
            {
                Ok(response) => Ok(json_response(response, StatusCode::OK)),
                Err(e) => {
                    error!("Failed to check binaries: {e}");
                    Ok(bad_request(format!("Failed to check binaries: {e}")))
                }
            }
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
) -> Result<Response<ResponseBody>, Infallible> {
    // Parse query parameters
    let query_params = parse_query_params(req.uri());
    let commit_ref = match get_commit_param(&query_params) {
        Ok(commit) => commit,
        Err(response) => return Ok(response),
    };
    let cpu_target = match get_cpu_target_param(&query_params) {
        Ok(target) => target,
        Err(response) => return Ok(response),
    };
    let toolchain = get_toolchain_param(&query_params);
    let features = get_features_param(&query_params);
    let binary_name = match get_binary_param(&query_params) {
        Ok(name) => name,
        Err(response) => return Ok(response),
    };

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(&commit_ref).await {
        Ok(resolved_commit) => {
            match cache
                .get_binary_info(
                    &resolved_commit,
                    &cpu_target,
                    toolchain.as_deref(),
                    &features,
                    &binary_name,
                )
                .await
            {
                Ok((binary_path, file_size, sha256_hash)) => {
                    // Generate ETag based on SHA256 hash
                    let etag = format!("\"sha256:{sha256_hash}\"");

                    // Check if client has cached version
                    if let Some(if_none_match) = req.headers().get(IF_NONE_MATCH) {
                        if let Ok(client_etag) = if_none_match.to_str() {
                            if client_etag == etag {
                                return Ok(Response::builder()
                                    .status(StatusCode::NOT_MODIFIED)
                                    .header(ETAG, etag)
                                    .header("cache-control", "public, max-age=31536000")
                                    .body(
                                        Full::new(Bytes::new())
                                            .map_err(|never| match never {})
                                            .boxed(),
                                    )
                                    .unwrap());
                            }
                        }
                    }

                    // Handle full file download with resumable support
                    handle_full_file_download(&binary_path, file_size, &resolved_commit, &etag)
                        .await
                }
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

/// Handle full file download with streaming for large files
async fn handle_full_file_download(
    binary_path: &std::path::Path,
    file_size: u64,
    resolved_commit: &str,
    etag: &str,
) -> Result<Response<ResponseBody>, Infallible> {
    match tokio::fs::File::open(binary_path).await {
        Ok(file) => {
            let reader_stream = ReaderStream::new(file);
            let stream_body = StreamBody::new(reader_stream.map(|result| result.map(Frame::data)));

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(CONTENT_LENGTH, file_size.to_string())
                .header(ETAG, etag)
                .header("x-iota-build-commit-hash", resolved_commit)
                .header("cache-control", "public, max-age=31536000") // Cache for 1 year since content is immutable
                .body(BodyExt::boxed(stream_body))
                .unwrap())
        }
        Err(e) => Ok(internal_error(format!("Failed to open file: {e}"))),
    }
}

/// Handle build requests
async fn handle_build_request(
    req: Request<Incoming>,
    cache: Arc<BuildCache>,
) -> Result<Response<ResponseBody>, Infallible> {
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
                        build_request.toolchain.as_deref(),
                        &build_request.features,
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
) -> Result<Response<ResponseBody>, Infallible> {
    // Parse query parameters
    let query_params = parse_query_params(req.uri());
    let commit_ref = match get_commit_param(&query_params) {
        Ok(commit) => commit,
        Err(response) => return Ok(response),
    };
    let cpu_target = match get_cpu_target_param(&query_params) {
        Ok(target) => target,
        Err(response) => return Ok(response),
    };
    let toolchain = get_toolchain_param(&query_params);
    let features = get_features_param(&query_params);
    let binaries = match get_binaries_param(&query_params) {
        Ok(binaries) => binaries,
        Err(response) => return Ok(response),
    };

    // Resolve branch/tag/commit to actual commit hash
    match cache.resolve_commit(&commit_ref).await {
        Ok(resolved_commit) => {
            match cache
                .get_build_status(
                    &resolved_commit,
                    &cpu_target,
                    toolchain.as_deref(),
                    &features,
                    &binaries,
                )
                .await
            {
                Ok(Some(status)) => Ok(json_response(status, StatusCode::OK)),
                Ok(None) => Ok(not_found("Build status not found")),
                Err(e) => {
                    error!("Failed to get build status: {e}");
                    Ok(bad_request(format!("Failed to get build status: {e}")))
                }
            }
        }
        Err(e) => {
            error!("Failed to resolve commit '{commit_ref}': {e}");
            Ok(bad_request(format!("Invalid commit/branch/tag: {e}")))
        }
    }
}

/// Parse query parameters using url crate for proper URL decoding
fn parse_query_params(uri: &http::Uri) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();

    // Extract query string directly from URI
    if let Some(query) = uri.query() {
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            params.insert(key.to_string(), value.to_string());
        }
    }

    params
}

/// Extract commit reference from query parameters
fn get_commit_param(
    query_params: &std::collections::HashMap<String, String>,
) -> Result<String, Response<ResponseBody>> {
    match query_params.get("commit") {
        Some(commit) => Ok(commit.clone()),
        None => Err(bad_request("Missing 'commit' query parameter")),
    }
}

/// Extract CPU target from query parameters
fn get_cpu_target_param(
    query_params: &std::collections::HashMap<String, String>,
) -> Result<String, Response<ResponseBody>> {
    match query_params.get("cpu_target") {
        Some(target) => Ok(target.clone()),
        None => Err(bad_request("Missing 'cpu_target' query parameter")),
    }
}

/// Extract optional rust toolchain from query parameters
/// Returns None if not specified or if "stable" is passed (treated as default)
fn get_toolchain_param(query_params: &std::collections::HashMap<String, String>) -> Option<String> {
    match query_params.get("toolchain") {
        Some(tc) if !tc.is_empty() && tc != "stable" => Some(tc.clone()),
        _ => None,
    }
}

/// Extract optional features list from query parameters (comma-separated)
fn get_features_param(query_params: &std::collections::HashMap<String, String>) -> Vec<String> {
    match query_params.get("features") {
        Some(features_str) if !features_str.is_empty() => {
            let mut features: Vec<String> = features_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            // Sort features for consistent cache keys
            features.sort();
            features
        }
        _ => Vec::new(),
    }
}

/// Extract binaries list from query parameters (required, returns error if
/// missing or empty)
fn get_binaries_param(
    query_params: &std::collections::HashMap<String, String>,
) -> Result<Vec<String>, Response<ResponseBody>> {
    match query_params.get("binaries") {
        Some(binaries_str) if !binaries_str.is_empty() => {
            let binaries: Vec<String> = binaries_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            if binaries.is_empty() || binaries.iter().any(|b| b.is_empty()) {
                Err(bad_request(
                    "'binaries' parameter cannot be empty or contain empty binary names",
                ))
            } else {
                Ok(binaries)
            }
        }
        Some(_) => Err(bad_request("'binaries' parameter cannot be empty")),
        None => Err(bad_request("Missing 'binaries' query parameter")),
    }
}

/// Extract single binary name from query parameters
fn get_binary_param(
    query_params: &std::collections::HashMap<String, String>,
) -> Result<String, Response<ResponseBody>> {
    match query_params.get("binary") {
        Some(name) => Ok(name.clone()),
        None => Err(bad_request("Missing 'binary' query parameter")),
    }
}
