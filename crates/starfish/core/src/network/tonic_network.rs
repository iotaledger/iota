// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt as _, stream};
use iota_http::ServerHandle;
use iota_network_stack::{
    Multiaddr,
    callback::{CallbackLayer, MakeCallbackHandler, ResponseHandler},
    multiaddr::Protocol,
};
use iota_tls::AllowPublicKeys;
use parking_lot::RwLock;
use starfish_config::{AuthorityIndex, NetworkKeyPair, NetworkPublicKey};
use tokio_stream::{Iter, iter};
use tonic::{Request, Response, Streaming, codec::CompressionEncoding};
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, TraceLayer};
use tracing::{debug, error, info, trace, warn};

use super::{
    BlockBundleStream, NetworkClient, NetworkService, SerializedBlockBundle, TransactionFetchMode,
    metrics_layer::{MetricsCallbackMaker, MetricsResponseCallback, SizedRequest, SizedResponse},
    tonic_gen::{
        consensus_service_client::ConsensusServiceClient,
        consensus_service_server::ConsensusService,
    },
};
use crate::{
    CommitIndex, Round,
    block_header::BlockRef,
    commit::CommitRange,
    commit_syncer::CommitSyncType,
    context::Context,
    error::{ConsensusError, ConsensusResult},
    network::{
        tonic_gen::consensus_service_server::ConsensusServiceServer,
        tonic_tls::certificate_server_name,
    },
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

// Maximum bytes size in a single fetch_blocks()response.
// TODO: put max RPC response size in protocol config.
const MAX_FETCH_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

// Maximum total bytes fetched in a single fetch_blocks() call, after combining
// the responses.
const MAX_TOTAL_FETCHED_BYTES: usize = 128 * 1024 * 1024;

// Implements Tonic RPC client for Consensus.
pub(crate) struct TonicClient {
    context: Arc<Context>,
    network_keypair: NetworkKeyPair,
    channel_pool: Arc<ChannelPool>,
}

impl TonicClient {
    pub(crate) fn new(context: Arc<Context>, network_keypair: NetworkKeyPair) -> Self {
        Self {
            context: context.clone(),
            network_keypair,
            channel_pool: Arc::new(ChannelPool::new(context)),
        }
    }

    async fn get_client(
        &self,
        peer: AuthorityIndex,
        timeout: Duration,
    ) -> ConsensusResult<ConsensusServiceClient<Channel>> {
        let config = &self.context.parameters.tonic;
        let channel = self
            .channel_pool
            .get_channel(self.network_keypair.clone(), peer, timeout)
            .await?;
        let mut client = ConsensusServiceClient::new(channel)
            .max_encoding_message_size(config.message_size_limit)
            .max_decoding_message_size(config.message_size_limit);

        if self.context.protocol_config.consensus_zstd_compression() {
            client = client
                .send_compressed(CompressionEncoding::Zstd)
                .accept_compressed(CompressionEncoding::Zstd);
        }
        Ok(client)
    }
}

// TODO: make sure callsites do not send request to own index, and return error
// otherwise.
#[async_trait]
impl NetworkClient for TonicClient {
    async fn subscribe_block_bundles(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
        timeout: Duration,
    ) -> ConsensusResult<BlockBundleStream> {
        let mut client = self.get_client(peer, timeout).await?;
        // TODO: add sampled block acknowledgments for latency measurements.
        let request = Request::new(stream::once(async move {
            SubscribeBlockBundlesRequest {
                last_received_round: last_received,
            }
        }));
        let response = client.subscribe_block_bundles(request).await.map_err(|e| {
            ConsensusError::NetworkRequest(format!("subscribe_block_bundles failed: {e:?}"))
        })?;
        let stream = response
            .into_inner()
            .take_while(|b| futures::future::ready(b.is_ok()))
            .filter_map(move |b| async move {
                match b {
                    Ok(response) => Some(SerializedBlockBundle {
                        serialized_block_bundle: response.serialized_block_bundle,
                    }),
                    Err(e) => {
                        debug!("Network error received from {}: {e:?}", peer);
                        None
                    }
                }
            });
        let rate_limited_stream =
            tokio_stream::StreamExt::throttle(stream, self.context.parameters.min_block_delay / 2)
                .boxed();
        Ok(rate_limited_stream)
    }

    // Returns a vector of serialized block headers
    async fn fetch_block_headers(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>> {
        let mut client = self.get_client(peer, timeout).await?;
        let mut request = Request::new(FetchBlockHeadersRequest {
            block_refs: block_refs
                .iter()
                .filter_map(|r| match bcs::to_bytes(r) {
                    Ok(serialized) => Some(serialized),
                    Err(e) => {
                        debug!("Failed to serialize block ref {:?}: {e:?}", r);
                        None
                    }
                })
                .collect(),
            highest_accepted_rounds,
        });
        request.set_timeout(timeout);
        let mut stream = client
            .fetch_block_headers(request)
            .await
            .map_err(|e| {
                if e.code() == tonic::Code::DeadlineExceeded {
                    ConsensusError::NetworkRequestTimeout(format!("fetch_blocks failed: {e:?}"))
                } else {
                    ConsensusError::NetworkRequest(format!("fetch_blocks failed: {e:?}"))
                }
            })?
            .into_inner();
        let mut vec_serialized_block_header = vec![];
        let mut total_fetched_bytes = 0;
        loop {
            match stream.message().await {
                Ok(Some(response)) => {
                    for b in &response.vec_serialized_block_header {
                        total_fetched_bytes += b.len();
                    }
                    vec_serialized_block_header.extend(response.vec_serialized_block_header);
                    if total_fetched_bytes > MAX_TOTAL_FETCHED_BYTES {
                        info!(
                            "fetch_blocks() fetched bytes exceeded limit: {} > {}, terminating stream.",
                            total_fetched_bytes, MAX_TOTAL_FETCHED_BYTES,
                        );
                        break;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    if vec_serialized_block_header.is_empty() {
                        if e.code() == tonic::Code::DeadlineExceeded {
                            return Err(ConsensusError::NetworkRequestTimeout(format!(
                                "fetch_block_headers failed mid-stream: {e:?}"
                            )));
                        }
                        return Err(ConsensusError::NetworkRequest(format!(
                            "fetch_block_headers failed mid-stream: {e:?}"
                        )));
                    } else {
                        warn!("fetch_block_headers failed mid-stream: {e:?}");
                        break;
                    }
                }
            }
        }
        Ok(vec_serialized_block_header)
    }

    async fn fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
        let mut client = self.get_client(peer, timeout).await?;
        let mut request = Request::new(FetchCommitsRequest {
            start: commit_range.start(),
            end: commit_range.end(),
        });
        request.set_timeout(timeout);
        let response = client
            .fetch_commits(request)
            .await
            .map_err(|e| ConsensusError::NetworkRequest(format!("fetch_commits failed: {e:?}")))?;
        let response = response.into_inner();
        Ok((response.commits, response.certifier_block_headers))
    }

    async fn fetch_latest_block_headers(
        &self,
        peer: AuthorityIndex,
        authorities: Vec<AuthorityIndex>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>> {
        let mut client = self.get_client(peer, timeout).await?;
        let mut request = Request::new(FetchLatestBlockHeadersRequest {
            authorities: authorities
                .iter()
                .map(|authority| authority.value() as u32)
                .collect(),
        });
        request.set_timeout(timeout);
        let mut stream = client
            .fetch_latest_block_headers(request)
            .await
            .map_err(|e| {
                if e.code() == tonic::Code::DeadlineExceeded {
                    ConsensusError::NetworkRequestTimeout(format!(
                        "fetch_latest_block_headers failed: {e:?}"
                    ))
                } else {
                    ConsensusError::NetworkRequest(format!(
                        "fetch_latest_block_headers failed: {e:?}"
                    ))
                }
            })?
            .into_inner();
        let mut blocks = vec![];
        let mut total_fetched_bytes = 0;
        loop {
            match stream.message().await {
                Ok(Some(response)) => {
                    let vec_serialized_block_headers = response.vec_serialized_block_header;
                    for b in &vec_serialized_block_headers {
                        total_fetched_bytes += b.len();
                    }
                    blocks.extend(vec_serialized_block_headers);
                    if total_fetched_bytes > MAX_TOTAL_FETCHED_BYTES {
                        info!(
                            "fetch_blocks() fetched bytes exceeded limit: {} > {}, terminating stream.",
                            total_fetched_bytes, MAX_TOTAL_FETCHED_BYTES,
                        );
                        break;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    if blocks.is_empty() {
                        if e.code() == tonic::Code::DeadlineExceeded {
                            return Err(ConsensusError::NetworkRequestTimeout(format!(
                                "fetch_blocks failed mid-stream: {e:?}"
                            )));
                        }
                        return Err(ConsensusError::NetworkRequest(format!(
                            "fetch_blocks failed mid-stream: {e:?}"
                        )));
                    } else {
                        warn!("fetch_latest_blocks failed mid-stream: {e:?}");
                        break;
                    }
                }
            }
        }
        Ok(blocks)
    }

    async fn fetch_transactions(
        &self,
        peer: AuthorityIndex,
        transactions_refs: Vec<GenericTransactionRef>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>> {
        let mut client = self.get_client(peer, timeout).await?;
        let mut request = Request::new(FetchTransactionsRequest {
            block_refs: transactions_refs
                .iter()
                .filter_map(|r| match r {
                    GenericTransactionRef::BlockRef(block_ref) => match bcs::to_bytes(block_ref) {
                        Ok(serialized) => Some(serialized),
                        Err(e) => {
                            debug!("Failed to serialize BlockRef {:?}: {e:?}", block_ref);
                            None
                        }
                    },
                    GenericTransactionRef::TransactionRef(tx_ref) => match bcs::to_bytes(tx_ref) {
                        Ok(serialized) => Some(serialized),
                        Err(e) => {
                            debug!("Failed to serialize TransactionRef {:?}: {e:?}", tx_ref);
                            None
                        }
                    },
                })
                .collect(),
        });

        request.set_timeout(timeout);
        let mut stream = client
            .fetch_transactions(request)
            .await
            .map_err(|e| {
                if e.code() == tonic::Code::DeadlineExceeded {
                    ConsensusError::NetworkRequestTimeout(format!(
                        "fetch_transactions failed: {e:?}"
                    ))
                } else {
                    ConsensusError::NetworkRequest(format!("fetch_transactions failed: {e:?}"))
                }
            })?
            .into_inner();

        let mut total_fetched_bytes = 0;
        let mut vec_serialized_transactions = vec![];
        loop {
            match stream.message().await {
                Ok(Some(response)) => {
                    for b in &response.vec_serialized_transactions {
                        total_fetched_bytes += b.len();
                    }
                    if total_fetched_bytes > MAX_TOTAL_FETCHED_BYTES {
                        info!(
                            "fetch_transactions() fetched bytes exceeded limit: {} > {}, terminating stream.",
                            total_fetched_bytes, MAX_TOTAL_FETCHED_BYTES,
                        );
                        break;
                    }
                    vec_serialized_transactions.extend(response.vec_serialized_transactions);
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    if vec_serialized_transactions.is_empty() {
                        if e.code() == tonic::Code::DeadlineExceeded {
                            return Err(ConsensusError::NetworkRequestTimeout(format!(
                                "fetch_transactions failed mid-stream: {e:?}"
                            )));
                        }
                        return Err(ConsensusError::NetworkRequest(format!(
                            "fetch_transactions failed mid-stream: {e:?}"
                        )));
                    } else {
                        warn!("fetch_transactions failed mid-stream: {e:?}");
                        break;
                    }
                }
            }
        }
        Ok(vec_serialized_transactions)
    }

    async fn fetch_commits_and_transactions(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)> {
        let mut client = self.get_client(peer, timeout).await?;
        let mut request = Request::new(FetchCommitsAndTransactionsRequest {
            start: commit_range.start(),
            end: commit_range.end(),
        });
        request.set_timeout(timeout);
        let mut stream = client
            .fetch_commits_and_transactions(request)
            .await
            .map_err(|e| {
                if e.code() == tonic::Code::DeadlineExceeded {
                    ConsensusError::NetworkRequestTimeout(format!(
                        "fetch_commits_and_transactions failed: {e:?}"
                    ))
                } else {
                    ConsensusError::NetworkRequest(format!(
                        "fetch_commits_and_transactions failed: {e:?}"
                    ))
                }
            })?
            .into_inner();

        // First chunk contains commits and certifier headers
        let mut commits = Vec::new();
        let mut certifier_block_headers = Vec::new();
        let mut transactions = Vec::new();
        let mut total_fetched_bytes = 0;

        loop {
            match stream.message().await {
                Ok(Some(response)) => {
                    // Accumulate commits and headers (typically in first chunk)
                    for c in &response.commits {
                        total_fetched_bytes += c.len();
                    }
                    commits.extend(response.commits);

                    for h in &response.certifier_block_headers {
                        total_fetched_bytes += h.len();
                    }
                    certifier_block_headers.extend(response.certifier_block_headers);

                    // Accumulate transactions (streamed in subsequent chunks)
                    for t in &response.transactions {
                        total_fetched_bytes += t.len();
                    }
                    transactions.extend(response.transactions);

                    if total_fetched_bytes > MAX_TOTAL_FETCHED_BYTES {
                        info!(
                            "fetch_commits_and_transactions() fetched bytes exceeded limit: {} > {}, terminating stream.",
                            total_fetched_bytes, MAX_TOTAL_FETCHED_BYTES,
                        );
                        break;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    if commits.is_empty() {
                        if e.code() == tonic::Code::DeadlineExceeded {
                            return Err(ConsensusError::NetworkRequestTimeout(format!(
                                "fetch_commits_and_transactions failed mid-stream: {e:?}"
                            )));
                        }
                        return Err(ConsensusError::NetworkRequest(format!(
                            "fetch_commits_and_transactions failed mid-stream: {e:?}"
                        )));
                    } else {
                        warn!("fetch_commits_and_transactions failed mid-stream: {e:?}");
                        break;
                    }
                }
            }
        }

        Ok((commits, certifier_block_headers, transactions))
    }
}

// Tonic channel wrapped with layers.
type Channel = iota_network_stack::callback::Callback<
    tower_http::trace::Trace<
        tonic_rustls::Channel,
        tower_http::classify::SharedClassifier<tower_http::classify::GrpcErrorsAsFailures>,
    >,
    MetricsCallbackMaker,
>;

/// Manages a pool of connections to peers to avoid constantly reconnecting,
/// which can be expensive.
struct ChannelPool {
    context: Arc<Context>,
    // Size is limited by known authorities in the committee.
    channels: RwLock<BTreeMap<AuthorityIndex, Channel>>,
}

impl ChannelPool {
    fn new(context: Arc<Context>) -> Self {
        Self {
            context,
            channels: RwLock::new(BTreeMap::new()),
        }
    }

    async fn get_channel(
        &self,
        network_keypair: NetworkKeyPair,
        peer: AuthorityIndex,
        timeout: Duration,
    ) -> ConsensusResult<Channel> {
        {
            let channels = self.channels.read();
            if let Some(channel) = channels.get(&peer) {
                return Ok(channel.clone());
            }
        }

        let authority = self.context.committee.authority(peer);
        let address = to_host_port_str(&authority.address).map_err(|e| {
            ConsensusError::NetworkConfig(format!("Cannot convert address to host:port: {e:?}"))
        })?;
        let address = format!("https://{address}");
        let config = &self.context.parameters.tonic;
        let buffer_size = config.connection_buffer_size;
        let client_tls_config = iota_tls::create_rustls_client_config(
            self.context
                .committee
                .authority(peer)
                .network_key
                .clone()
                .into_inner(),
            certificate_server_name(&self.context),
            Some(network_keypair.private_key().into_inner()),
        );
        let endpoint = tonic_rustls::Channel::from_shared(address.clone())
            .unwrap()
            .connect_timeout(timeout)
            .initial_connection_window_size(Some(buffer_size as u32))
            .initial_stream_window_size(Some(buffer_size as u32 / 2))
            .keep_alive_while_idle(true)
            .keep_alive_timeout(config.keepalive_interval)
            .http2_keep_alive_interval(config.keepalive_interval)
            // tcp keepalive is probably unnecessary and is unsupported by msim.
            .user_agent("mysticeti")
            .unwrap()
            .tls_config(client_tls_config)
            .unwrap();

        let deadline = tokio::time::Instant::now() + timeout;
        let channel = loop {
            trace!("Connecting to endpoint at {address}");
            match endpoint.connect().await {
                Ok(channel) => break channel,
                Err(e) => {
                    debug!("Failed to connect to endpoint at {address}: {e:?}");
                    if tokio::time::Instant::now() >= deadline {
                        return Err(ConsensusError::NetworkClientConnection(format!(
                            "Timed out connecting to endpoint at {address}: {e:?}"
                        )));
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        };
        trace!("Connected to {address}");

        let channel = tower::ServiceBuilder::new()
            .layer(CallbackLayer::new(MetricsCallbackMaker::new(
                self.context.metrics.network_metrics.outbound.clone(),
                self.context.parameters.tonic.excessive_message_size,
            )))
            .layer(
                TraceLayer::new_for_grpc()
                    .make_span_with(DefaultMakeSpan::new().level(tracing::Level::TRACE))
                    .on_failure(DefaultOnFailure::new().level(tracing::Level::DEBUG)),
            )
            .service(channel);

        let mut channels = self.channels.write();
        // There should not be many concurrent attempts at connecting to the same peer.
        let channel = channels.entry(peer).or_insert(channel);
        Ok(channel.clone())
    }
}

/// Proxies Tonic requests to NetworkService with actual handler implementation.
struct TonicServiceProxy<S: NetworkService> {
    context: Arc<Context>,
    service: Arc<S>,
}

impl<S: NetworkService> TonicServiceProxy<S> {
    fn new(context: Arc<Context>, service: Arc<S>) -> Self {
        Self { context, service }
    }
}

#[async_trait]
impl<S: NetworkService> ConsensusService for TonicServiceProxy<S> {
    type SubscribeBlockBundlesStream =
        Pin<Box<dyn Stream<Item = Result<SubscribeBlockBundlesResponse, tonic::Status>> + Send>>;

    async fn subscribe_block_bundles(
        &self,
        request: Request<Streaming<SubscribeBlockBundlesRequest>>,
    ) -> Result<Response<Self::SubscribeBlockBundlesStream>, tonic::Status> {
        let Some(peer_index) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|p| p.authority_index)
        else {
            return Err(tonic::Status::internal("PeerInfo not found"));
        };
        let mut request_stream = request.into_inner();
        let first_request = match request_stream.next().await {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                debug!(
                    "subscribe_block_bundles() request from {} failed: {e:?}",
                    peer_index
                );
                return Err(tonic::Status::invalid_argument("Request error"));
            }
            None => {
                return Err(tonic::Status::invalid_argument("Missing request"));
            }
        };
        let stream = self
            .service
            .handle_subscribe_block_bundles_request(peer_index, first_request.last_received_round)
            .await
            .map_err(|e| tonic::Status::internal(format!("{e:?}")))?
            .map(|serialized_block_bundle| {
                Ok(SubscribeBlockBundlesResponse {
                    serialized_block_bundle: serialized_block_bundle.serialized_block_bundle,
                })
            });
        let rate_limited_stream =
            tokio_stream::StreamExt::throttle(stream, self.context.parameters.min_block_delay / 2)
                .boxed();
        Ok(Response::new(rate_limited_stream))
    }

    type FetchBlockHeadersStream =
        Iter<std::vec::IntoIter<Result<FetchBlockHeadersResponse, tonic::Status>>>;

    async fn fetch_block_headers(
        &self,
        request: Request<FetchBlockHeadersRequest>,
    ) -> Result<Response<Self::FetchBlockHeadersStream>, tonic::Status> {
        let Some(peer_index) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|p| p.authority_index)
        else {
            return Err(tonic::Status::internal("PeerInfo not found"));
        };
        let inner = request.into_inner();
        let block_refs = inner
            .block_refs
            .into_iter()
            .filter_map(|serialized| match bcs::from_bytes(&serialized) {
                Ok(r) => Some(r),
                Err(e) => {
                    debug!("Failed to deserialize block ref {:?}: {e:?}", serialized);
                    None
                }
            })
            .collect();
        let highest_accepted_rounds = inner.highest_accepted_rounds;
        let blocks = self
            .service
            .handle_fetch_headers(peer_index, block_refs, highest_accepted_rounds)
            .await
            .map_err(|e| tonic::Status::internal(format!("{e:?}")))?;
        let responses: std::vec::IntoIter<Result<FetchBlockHeadersResponse, tonic::Status>> =
            chunk_data(blocks, MAX_FETCH_RESPONSE_BYTES)
                .into_iter()
                .map(|block_headers| {
                    Ok(FetchBlockHeadersResponse {
                        vec_serialized_block_header: block_headers,
                    })
                })
                .collect::<Vec<_>>()
                .into_iter();
        let stream = iter(responses);
        Ok(Response::new(stream))
    }

    async fn fetch_commits(
        &self,
        request: Request<FetchCommitsRequest>,
    ) -> Result<Response<FetchCommitsResponse>, tonic::Status> {
        let Some(peer_index) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|p| p.authority_index)
        else {
            return Err(tonic::Status::internal("PeerInfo not found"));
        };
        let request = request.into_inner();
        let (commits, certifier_block_headers) = self
            .service
            .handle_fetch_commits(
                peer_index,
                (request.start..=request.end).into(),
                CommitSyncType::Regular,
            )
            .await
            .map_err(|e| tonic::Status::internal(format!("{e:?}")))?;
        let commits = commits
            .into_iter()
            .map(|c| c.serialized().clone())
            .collect();
        let certifier_block_headers = certifier_block_headers
            .into_iter()
            .map(|bh| bh.serialized().clone())
            .collect();
        Ok(Response::new(FetchCommitsResponse {
            commits,
            certifier_block_headers,
        }))
    }

    type FetchCommitsAndTransactionsStream =
        Iter<std::vec::IntoIter<Result<FetchCommitsAndTransactionsResponse, tonic::Status>>>;

    async fn fetch_commits_and_transactions(
        &self,
        request: Request<FetchCommitsAndTransactionsRequest>,
    ) -> Result<Response<Self::FetchCommitsAndTransactionsStream>, tonic::Status> {
        let Some(peer_index) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|p| p.authority_index)
        else {
            return Err(tonic::Status::internal("PeerInfo not found"));
        };
        let request = request.into_inner();
        let (serialized_commits, serialized_headers, serialized_transactions) = self
            .service
            .handle_fetch_commits_and_transactions(peer_index, (request.start..=request.end).into())
            .await
            .map_err(|e| tonic::Status::internal(format!("{e:?}")))?;

        // Build response as a stream of chunks to stay under gRPC message size limit.
        // Commits and transactions are chunked by size. Certifier headers are small
        // enough to fit in a single chunk and are sent with the first commit chunk.
        let mut responses = Vec::new();

        let commit_chunks = chunk_data(serialized_commits, MAX_FETCH_RESPONSE_BYTES);
        for (i, commit_chunk) in commit_chunks.into_iter().enumerate() {
            responses.push(Ok(FetchCommitsAndTransactionsResponse {
                commits: commit_chunk,
                certifier_block_headers: if i == 0 {
                    serialized_headers.clone()
                } else {
                    vec![]
                },
                transactions: vec![],
            }));
        }

        if responses.is_empty() {
            responses.push(Ok(FetchCommitsAndTransactionsResponse {
                commits: vec![],
                certifier_block_headers: serialized_headers,
                transactions: vec![],
            }));
        }

        let tx_chunks = chunk_data(serialized_transactions, MAX_FETCH_RESPONSE_BYTES);
        for txs_chunk in tx_chunks {
            responses.push(Ok(FetchCommitsAndTransactionsResponse {
                commits: vec![],
                certifier_block_headers: vec![],
                transactions: txs_chunk,
            }));
        }

        let stream = iter(responses);
        Ok(Response::new(stream))
    }

    type FetchLatestBlockHeadersStream =
        Iter<std::vec::IntoIter<Result<FetchLatestBlockHeadersResponse, tonic::Status>>>;

    async fn fetch_latest_block_headers(
        &self,
        request: Request<FetchLatestBlockHeadersRequest>,
    ) -> Result<Response<Self::FetchLatestBlockHeadersStream>, tonic::Status> {
        let Some(peer_index) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|p| p.authority_index)
        else {
            return Err(tonic::Status::internal("PeerInfo not found"));
        };
        let inner = request.into_inner();

        // Convert the authority indexes and validate them
        let mut authorities = vec![];
        for authority in inner.authorities.into_iter() {
            let Some(authority) = self
                .context
                .committee
                .to_authority_index(authority as usize)
            else {
                return Err(tonic::Status::internal(format!(
                    "Invalid authority index provided {authority}"
                )));
            };
            authorities.push(authority);
        }

        let blocks = self
            .service
            .handle_fetch_latest_block_headers(peer_index, authorities)
            .await
            .map_err(|e| tonic::Status::internal(format!("{e:?}")))?;
        let responses: std::vec::IntoIter<Result<FetchLatestBlockHeadersResponse, tonic::Status>> =
            chunk_data(blocks, MAX_FETCH_RESPONSE_BYTES)
                .into_iter()
                .map(|block_headers| {
                    Ok(FetchLatestBlockHeadersResponse {
                        vec_serialized_block_header: block_headers,
                    })
                })
                .collect::<Vec<_>>()
                .into_iter();
        let stream = iter(responses);
        Ok(Response::new(stream))
    }

    async fn get_latest_rounds(
        &self,
        _request: Request<GetLatestRoundsRequest>,
    ) -> Result<Response<GetLatestRoundsResponse>, tonic::Status> {
        error!("get_latest_rounds() is deprecated in starfish and should not be called");
        unimplemented!();
    }

    type FetchTransactionsStream =
        Iter<std::vec::IntoIter<Result<FetchTransactionsResponse, tonic::Status>>>;

    async fn fetch_transactions(
        &self,
        request: Request<FetchTransactionsRequest>,
    ) -> Result<Response<Self::FetchTransactionsStream>, tonic::Status> {
        let Some(peer_index) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|p| p.authority_index)
        else {
            return Err(tonic::Status::internal("PeerInfo not found"));
        };

        let request = request.into_inner();
        let committed_transactions_refs: Vec<GenericTransactionRef> = request
            .block_refs
            .iter()
            .filter_map(|r| {
                if self.context.protocol_config.consensus_fast_commit_sync() {
                    match bcs::from_bytes::<TransactionRef>(r) {
                        Ok(transaction_ref) => {
                            Some(GenericTransactionRef::TransactionRef(transaction_ref))
                        }
                        Err(e) => {
                            debug!("Failed to deserialize block ref: {e:?}");
                            None
                        }
                    }
                } else {
                    match bcs::from_bytes::<BlockRef>(r) {
                        Ok(block_ref) => Some(GenericTransactionRef::BlockRef(block_ref)),
                        Err(e) => {
                            debug!("Failed to deserialize block ref: {e:?}");
                            None
                        }
                    }
                }
            })
            .collect();

        let vec_serialized_transactions = self
            .service
            .handle_fetch_transactions(
                peer_index,
                committed_transactions_refs,
                TransactionFetchMode::TransactionSync,
            )
            .await
            .map_err(|e| tonic::Status::internal(format!("fetch_transactions failed: {e:?}")))?;

        let responses: std::vec::IntoIter<Result<FetchTransactionsResponse, tonic::Status>> =
            chunk_data(vec_serialized_transactions, MAX_FETCH_RESPONSE_BYTES)
                .into_iter()
                .map(|transactions| {
                    Ok(FetchTransactionsResponse {
                        vec_serialized_transactions: transactions,
                    })
                })
                .collect::<Vec<_>>()
                .into_iter();
        let stream = iter(responses);
        Ok(Response::new(stream))
    }
}

/// Manages the lifecycle of Tonic network client and service. Typical usage
/// during initialization:
/// 1. Create a new `TonicManager`.
/// 2. Take `TonicClient` from `TonicManager::client()`.
/// 3. Create consensus components.
/// 4. Create `TonicService` for consensus service handler.
/// 5. Install `TonicService` to `TonicManager` with
///    `TonicManager::install_service()`.
pub(crate) struct TonicManager<S>
where
    S: NetworkService,
{
    context: Arc<Context>,
    network_keypair: NetworkKeyPair,
    client: Arc<TonicClient>,
    server: Option<ServerHandle>,
    _marker: std::marker::PhantomData<S>,
}

impl<S: NetworkService> TonicManager<S> {
    pub(crate) fn new(context: Arc<Context>, network_keypair: NetworkKeyPair) -> Self {
        Self {
            context: context.clone(),
            network_keypair: network_keypair.clone(),
            client: Arc::new(TonicClient::new(context, network_keypair)),
            server: None,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn client(&self) -> Arc<TonicClient> {
        self.client.clone()
    }

    pub async fn install_service(&mut self, service: Arc<S>) {
        self.context
            .metrics
            .network_metrics
            .network_type
            .with_label_values(&["tonic"])
            .set(1);

        info!("Starting tonic service");

        let authority = self.context.committee.authority(self.context.own_index);
        // By default, bind to the unspecified address to allow the actual address to be
        // assigned. But bind to localhost if it is requested.
        let own_address = if authority.address.is_localhost_ip() {
            authority.address.clone()
        } else {
            authority.address.with_zero_ip()
        };
        let own_address = to_socket_addr(&own_address).unwrap();
        let service = TonicServiceProxy::new(self.context.clone(), service);
        let config = &self.context.parameters.tonic;

        let connections_info = Arc::new(ConnectionsInfo::new(self.context.clone()));
        let layers = tower::ServiceBuilder::new()
            // Add a layer to extract a peer's PeerInfo from their TLS certs
            .map_request(move |mut request: http::Request<_>| {
                if let Some(peer_certificates) =
                    request.extensions().get::<iota_http::PeerCertificates>()
                {
                    if let Some(peer_info) =
                        peer_info_from_certs(&connections_info, peer_certificates)
                    {
                        request.extensions_mut().insert(peer_info);
                    }
                }
                request
            })
            .layer(CallbackLayer::new(MetricsCallbackMaker::new(
                self.context.metrics.network_metrics.inbound.clone(),
                self.context.parameters.tonic.excessive_message_size,
            )))
            .layer(
                TraceLayer::new_for_grpc()
                    .make_span_with(DefaultMakeSpan::new().level(tracing::Level::TRACE))
                    .on_failure(DefaultOnFailure::new().level(tracing::Level::DEBUG)),
            )
            .layer_fn(|service| iota_network_stack::grpc_timeout::GrpcTimeout::new(service, None));

        let mut consensus_service_server = ConsensusServiceServer::new(service)
            .max_encoding_message_size(config.message_size_limit)
            .max_decoding_message_size(config.message_size_limit);

        if self.context.protocol_config.consensus_zstd_compression() {
            consensus_service_server = consensus_service_server
                .send_compressed(CompressionEncoding::Zstd)
                .accept_compressed(CompressionEncoding::Zstd);
        }

        let consensus_service = tonic::service::Routes::new(consensus_service_server)
            .into_axum_router()
            .route_layer(layers);

        let tls_server_config = iota_tls::create_rustls_server_config_with_client_verifier(
            self.network_keypair.clone().private_key().into_inner(),
            certificate_server_name(&self.context),
            AllowPublicKeys::new(
                self.context
                    .committee
                    .authorities()
                    .map(|(_i, a)| a.network_key.clone().into_inner())
                    .collect(),
            ),
        );

        // Calculate some metrics around send/recv buffer sizes for the current
        // machine/OS
        #[cfg(not(msim))]
        {
            let tcp_connection_metrics =
                &self.context.metrics.network_metrics.tcp_connection_metrics;

            // Try creating an ephemeral port to test the highest allowed send and recv
            // buffer sizes. Buffer sizes are not set explicitly on the socket
            // used for real traffic, to allow the OS to set appropriate values.
            {
                let ephemeral_addr = SocketAddr::new(own_address.ip(), 0);
                let ephemeral_socket = create_socket(&ephemeral_addr);
                tcp_connection_metrics
                    .socket_send_buffer_size
                    .set(ephemeral_socket.send_buffer_size().unwrap_or(0) as i64);
                tcp_connection_metrics
                    .socket_recv_buffer_size
                    .set(ephemeral_socket.recv_buffer_size().unwrap_or(0) as i64);

                if let Err(e) = ephemeral_socket.set_send_buffer_size(32 << 20) {
                    info!("Failed to set send buffer size: {e:?}");
                }
                if let Err(e) = ephemeral_socket.set_recv_buffer_size(32 << 20) {
                    info!("Failed to set recv buffer size: {e:?}");
                }
                if ephemeral_socket.bind(ephemeral_addr).is_ok() {
                    tcp_connection_metrics
                        .socket_send_buffer_max_size
                        .set(ephemeral_socket.send_buffer_size().unwrap_or(0) as i64);
                    tcp_connection_metrics
                        .socket_recv_buffer_max_size
                        .set(ephemeral_socket.recv_buffer_size().unwrap_or(0) as i64);
                };
            }
        }

        let http_config = iota_http::Config::default()
            .tcp_nodelay(true)
            .initial_connection_window_size(64 << 20)
            .initial_stream_window_size(32 << 20)
            .http2_keepalive_interval(Some(config.keepalive_interval))
            .http2_keepalive_timeout(Some(config.keepalive_interval))
            .accept_http1(false);

        // Create server
        //
        // During simtest crash/restart tests there may be an older instance of
        // consensus running that is bound to the TCP port of `own_address` that
        // hasn't finished relinquishing control of the port yet. So instead of
        // crashing when the address is inuse, we will retry for a short/
        // reasonable period of time before giving up.
        let deadline = Instant::now() + Duration::from_secs(20);
        let server = loop {
            match iota_http::Builder::new()
                .config(http_config.clone())
                .tls_config(tls_server_config.clone())
                .serve(own_address, consensus_service.clone())
            {
                Ok(server) => break server,
                Err(err) => {
                    warn!("Error starting consensus server: {err:?}");
                    if Instant::now() > deadline {
                        panic!("Failed to start consensus server within required deadline");
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        };

        info!("Server started at: {own_address}");
        self.server = Some(server);
    }

    pub async fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.shutdown().await;
        }

        self.context
            .metrics
            .network_metrics
            .network_type
            .with_label_values(&["tonic"])
            .set(0);
    }
}

// Ensure that if there is an active network running that it is shutdown when
// the TonicManager is dropped.
impl<S: NetworkService> Drop for TonicManager<S> {
    fn drop(&mut self) {
        if let Some(server) = self.server.as_ref() {
            server.trigger_shutdown();
        }
    }
}

// TODO: improve iota-http to allow for providing a MakeService so that this can
// be done once per connection
fn peer_info_from_certs(
    connections_info: &ConnectionsInfo,
    peer_certificates: &iota_http::PeerCertificates,
) -> Option<PeerInfo> {
    let certs = peer_certificates.peer_certs();

    if certs.len() != 1 {
        trace!(
            "Unexpected number of certificates from TLS stream: {}",
            certs.len()
        );
        return None;
    }
    trace!("Received {} certificates", certs.len());
    let public_key = iota_tls::public_key_from_certificate(&certs[0])
        .map_err(|e| {
            trace!("Failed to extract public key from certificate: {e:?}");
            e
        })
        .ok()?;
    let client_public_key = NetworkPublicKey::new(public_key);
    let Some(authority_index) = connections_info.authority_index(&client_public_key) else {
        error!("Failed to find the authority with public key {client_public_key:?}");
        return None;
    };
    Some(PeerInfo { authority_index })
}

/// Attempts to convert a multiaddr of the form `/[ip4,ip6,dns]/{}/udp/{port}`
/// into a host:port string.
fn to_host_port_str(addr: &Multiaddr) -> Result<String, String> {
    let mut iter = addr.iter();

    match (iter.next(), iter.next()) {
        (Some(Protocol::Ip4(ipaddr)), Some(Protocol::Udp(port))) => Ok(format!("{ipaddr}:{port}")),
        (Some(Protocol::Ip6(ipaddr)), Some(Protocol::Udp(port))) => Ok(format!("{ipaddr}:{port}")),
        (Some(Protocol::Dns(hostname)), Some(Protocol::Udp(port))) => {
            Ok(format!("{hostname}:{port}"))
        }

        _ => Err(format!("unsupported multiaddr: {addr}")),
    }
}

/// Attempts to convert a multiaddr of the form `/[ip4,ip6]/{}/[udp,tcp]/{port}`
/// into a SocketAddr value.
pub fn to_socket_addr(addr: &Multiaddr) -> Result<SocketAddr, String> {
    let mut iter = addr.iter();

    match (iter.next(), iter.next()) {
        (Some(Protocol::Ip4(ipaddr)), Some(Protocol::Udp(port)))
        | (Some(Protocol::Ip4(ipaddr)), Some(Protocol::Tcp(port))) => {
            Ok(SocketAddr::V4(SocketAddrV4::new(ipaddr, port)))
        }

        (Some(Protocol::Ip6(ipaddr)), Some(Protocol::Udp(port)))
        | (Some(Protocol::Ip6(ipaddr)), Some(Protocol::Tcp(port))) => {
            Ok(SocketAddr::V6(SocketAddrV6::new(ipaddr, port, 0, 0)))
        }

        _ => Err(format!("unsupported multiaddr: {addr}")),
    }
}

#[cfg(not(msim))]
fn create_socket(address: &SocketAddr) -> tokio::net::TcpSocket {
    let socket = if address.is_ipv4() {
        tokio::net::TcpSocket::new_v4()
    } else if address.is_ipv6() {
        tokio::net::TcpSocket::new_v6()
    } else {
        panic!("Invalid own address: {address:?}");
    }
    .unwrap_or_else(|e| panic!("Cannot create TCP socket: {e:?}"));
    if let Err(e) = socket.set_nodelay(true) {
        info!("Failed to set TCP_NODELAY: {e:?}");
    }
    if let Err(e) = socket.set_reuseaddr(true) {
        info!("Failed to set SO_REUSEADDR: {e:?}");
    }
    socket
}

/// Looks up authority index by authority public key.
///
/// TODO: Add connection monitoring, and keep track of connected peers.
/// TODO: Maybe merge with connection_monitor.rs
struct ConnectionsInfo {
    authority_key_to_index: BTreeMap<NetworkPublicKey, AuthorityIndex>,
}

impl ConnectionsInfo {
    fn new(context: Arc<Context>) -> Self {
        let authority_key_to_index = context
            .committee
            .authorities()
            .map(|(index, authority)| (authority.network_key.clone(), index))
            .collect();
        Self {
            authority_key_to_index,
        }
    }

    fn authority_index(&self, key: &NetworkPublicKey) -> Option<AuthorityIndex> {
        self.authority_key_to_index.get(key).copied()
    }
}

/// Information about the client peer, set per connection.
#[derive(Clone, Debug)]
struct PeerInfo {
    authority_index: AuthorityIndex,
}

// Adapt MetricsCallbackMaker and MetricsResponseCallback to http.

/// Calculate approximate size of HTTP headers.
/// Note: This is an approximation of uncompressed size. Actual wire size will
/// be smaller due to HTTP/2 HPACK compression.
fn calculate_header_size(headers: &http::HeaderMap) -> usize {
    headers
        .iter()
        .map(|(name, value)| {
            // +4 bytes for ": " and "\r\n" separator in HTTP/1.1 format
            name.as_str().len() + value.len() + 4
        })
        .sum()
}

impl SizedRequest for http::request::Parts {
    fn size(&self) -> usize {
        let header_size = calculate_header_size(&self.headers);
        let body_size = self
            .headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        header_size + body_size
    }

    fn route(&self) -> String {
        let path = self.uri.path();
        path.rsplit_once('/')
            .map(|(_, route)| route)
            .unwrap_or("unknown")
            .to_string()
    }
}

impl SizedResponse for http::response::Parts {
    fn size(&self) -> usize {
        // Return header size only. Body size is tracked separately via
        // ResponseHandler::on_body_chunk callback to support streaming responses.
        calculate_header_size(&self.headers)
    }

    fn error_type(&self) -> Option<String> {
        if self.status.is_success() {
            None
        } else {
            Some(self.status.to_string())
        }
    }
}

impl MakeCallbackHandler for MetricsCallbackMaker {
    type Handler = MetricsResponseCallback;

    fn make_handler(&self, request: &http::request::Parts) -> Self::Handler {
        self.handle_request(request)
    }
}

impl ResponseHandler for MetricsResponseCallback {
    fn on_response(&mut self, response: &http::response::Parts) {
        MetricsResponseCallback::on_response(self, response, &response.headers)
    }

    fn on_error<E>(&mut self, err: &E) {
        MetricsResponseCallback::on_error(self, err)
    }

    fn on_body_chunk<B>(&mut self, chunk: &B)
    where
        B: bytes::Buf,
    {
        let chunk_size = chunk.chunk().len();
        self.on_chunk(chunk_size);
    }
}

/// Network message types.
#[derive(Clone, prost::Message)]
pub(crate) struct SubscribeBlockBundlesRequest {
    #[prost(uint32, tag = "1")]
    last_received_round: Round,
}

#[derive(Clone, prost::Message)]
pub(crate) struct SubscribeBlockBundlesResponse {
    #[prost(bytes = "bytes", tag = "1")]
    serialized_block_bundle: Bytes,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchBlockHeadersRequest {
    #[prost(bytes = "vec", repeated, tag = "1")]
    block_refs: Vec<Vec<u8>>,
    // The highest accepted round per authority. The vector represents the round for each authority
    // and its length should be the same as the committee size.
    #[prost(uint32, repeated, tag = "2")]
    highest_accepted_rounds: Vec<Round>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchBlockHeadersResponse {
    #[prost(bytes = "bytes", repeated, tag = "1")]
    vec_serialized_block_header: Vec<Bytes>,
}

#[allow(unused)]
#[derive(Clone, prost::Message)]
pub(crate) struct FetchBlocksRequest {
    #[prost(bytes = "vec", repeated, tag = "1")]
    block_refs: Vec<Vec<u8>>,
    // The highest accepted round per authority. The vector represents the round for each authority
    // and its length should be the same as the committee size.
    #[prost(uint32, repeated, tag = "2")]
    highest_accepted_rounds: Vec<Round>,
}

#[allow(unused)]
#[derive(Clone, prost::Message)]
pub(crate) struct FetchBlocksResponse {
    #[prost(bytes = "bytes", repeated, tag = "1")]
    vec_serialized_blocks: Vec<Bytes>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchCommitsRequest {
    #[prost(uint32, tag = "1")]
    start: CommitIndex,
    #[prost(uint32, tag = "2")]
    end: CommitIndex,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchCommitsResponse {
    // Serialized consecutive Commit.
    #[prost(bytes = "bytes", repeated, tag = "1")]
    commits: Vec<Bytes>,
    // Serialized SignedBlockHeader that certify the last commit from above.
    #[prost(bytes = "bytes", repeated, tag = "2")]
    certifier_block_headers: Vec<Bytes>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchCommitsAndTransactionsRequest {
    #[prost(uint32, tag = "1")]
    start: CommitIndex,
    #[prost(uint32, tag = "2")]
    end: CommitIndex,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchCommitsAndTransactionsResponse {
    // Serialized consecutive Commit (sent in first chunk).
    #[prost(bytes = "bytes", repeated, tag = "1")]
    commits: Vec<Bytes>,
    // Serialized SignedBlockHeader that certify the last commit (sent in first chunk).
    #[prost(bytes = "bytes", repeated, tag = "2")]
    certifier_block_headers: Vec<Bytes>,
    // Serialized transactions as SerializedTransactionsV2 (sent in transaction chunks).
    // Each entry contains both the TransactionRef and the actual transaction data.
    #[prost(bytes = "bytes", repeated, tag = "3")]
    transactions: Vec<Bytes>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchLatestBlockHeadersRequest {
    #[prost(uint32, repeated, tag = "1")]
    authorities: Vec<u32>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchLatestBlockHeadersResponse {
    #[prost(bytes = "bytes", repeated, tag = "1")]
    vec_serialized_block_header: Vec<Bytes>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct GetLatestRoundsRequest {}

#[derive(Clone, prost::Message)]
pub(crate) struct GetLatestRoundsResponse {
    // Highest received round per authority.
    #[prost(uint32, repeated, tag = "1")]
    highest_received: Vec<u32>,
    // Highest accepted round per authority.
    #[prost(uint32, repeated, tag = "2")]
    highest_accepted: Vec<u32>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchTransactionsRequest {
    #[prost(bytes = "vec", repeated, tag = "1")]
    block_refs: Vec<Vec<u8>>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct FetchTransactionsResponse {
    #[prost(bytes = "bytes", repeated, tag = "1")]
    vec_serialized_transactions: Vec<Bytes>,
}

// Splits a list of byte sequences into chunks where each chunk's total size
// does not exceed the specified `chunk_limit`.
// Returns a vector of chunks, each being a vector of `Bytes`.
fn chunk_data(data: Vec<Bytes>, chunk_limit: usize) -> Vec<Vec<Bytes>> {
    let mut chunks = vec![];
    let mut chunk = vec![];
    let mut chunk_size = 0;
    for piece in data.into_iter() {
        let piece_size = piece.len();
        if !chunk.is_empty() && chunk_size + piece_size > chunk_limit {
            chunks.push(chunk);
            chunk = vec![];
            chunk_size = 0;
        }
        chunk.push(piece);
        chunk_size += piece_size;
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}
