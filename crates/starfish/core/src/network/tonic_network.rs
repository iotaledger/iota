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
use fastcrypto::{ed25519::Ed25519PublicKey, traits::ToFromBytes as _};
use futures::{Stream, StreamExt as _, TryStreamExt as _, stream};
use iota_http::{PeerConnectionEvent, ServerHandle};
use iota_network_stack::{
    Multiaddr,
    callback::{CallbackLayer, MakeCallbackHandler, ResponseHandler},
    multiaddr::Protocol,
};
use iota_tls::AllowPublicKeys;
use parking_lot::RwLock;
use starfish_config::{
    AuthorityIndex, MAX_HEADERS_PER_HEADER_SYNC_FETCH, NetworkKeyPair, NetworkPublicKey,
};
use tokio::sync::Mutex;
use tokio_stream::iter;
use tonic::{Request, Response, Streaming, codec::CompressionEncoding};
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, TraceLayer};
use tracing::{debug, error, info, trace, warn};

use super::{
    BlockBundleStream, NetworkClient, NetworkService, SerializedBlockBundle, TransactionFetchMode,
    admission::{Admission, AdmissionGuard, PerPeerAdmission, PermitGuardedStream, RpcGroup},
    metrics_layer::{MetricsCallbackMaker, MetricsResponseCallback},
    tonic_gen::{
        consensus_service_client::ConsensusServiceClient,
        consensus_service_server::ConsensusService,
    },
};
use crate::{
    CommitIndex, Round,
    block_header::{BlockRef, max_signed_block_header_bytes},
    block_verifier::{MAX_BCS_LENGTH_PREFIX_BYTES, serialized_transactions_size_limit},
    commit::{CommitRange, max_commit_bytes},
    commit_syncer::{CommitSyncType, MAX_COMMIT_VOTE_HEADERS_PER_AUTHORITY},
    context::Context,
    error::{ConsensusError, ConsensusResult},
    network::{
        tonic_gen::consensus_service_server::ConsensusServiceServer,
        tonic_tls::certificate_server_name,
    },
    transaction_ref::{SERIALIZED_TRANSACTION_REF_BYTES, TransactionRef},
};

// Maximum bytes size in a single fetch_blocks()response.
// TODO: put max RPC response size in protocol config.
const MAX_FETCH_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

// Upper bound on the bytes a fetch response may occupy once collected,
// terminating the stream once exceeded. Each bound mirrors the matching
// server-side per-fetch cap, so an honest peer's response always fits while a
// flooding peer is cut off. Sizing tracks the cap, not the requested count.

/// Bytes a buffer of `count` entries of at most `entry_bytes` each occupies:
/// the payload plus the `Bytes` descriptor every entry carries.
fn buffer_bytes(count: usize, entry_bytes: usize) -> usize {
    count.saturating_mul(entry_bytes.saturating_add(size_of::<Bytes>()))
}

/// Upper bound on the headers a fetch response may carry; a response with
/// more is the peer's fault. Commit sync: our own cap, since the server
/// returns only the requested headers. Header sync: the ceiling every
/// configuration respects, since the peer fills gaps up to its own cap.
fn max_fetched_headers(context: &Context, commit_sync: bool) -> usize {
    if commit_sync {
        context.parameters.max_headers_per_commit_sync_fetch
    } else {
        MAX_HEADERS_PER_HEADER_SYNC_FETCH
    }
}

/// Header-fetch budget: the most headers a response may carry, each entry at
/// the maximum serialized header size for this committee.
fn max_fetch_block_headers_response_bytes(context: &Context, commit_sync: bool) -> usize {
    buffer_bytes(
        max_fetched_headers(context, commit_sync),
        max_signed_block_header_bytes(context.committee.size()),
    )
}

/// Upper bound on one fetched transaction entry: a `SerializedTransactionsV2`
/// carrying the maximum serialized per-block transaction payload, framed with
/// its `TransactionRef` and length prefix.
fn max_serialized_transactions_entry_bytes(context: &Context) -> usize {
    serialized_transactions_size_limit(context)
        .saturating_add(SERIALIZED_TRANSACTION_REF_BYTES)
        .saturating_add(MAX_BCS_LENGTH_PREFIX_BYTES)
}

/// Transaction-fetch budget: one maximum-size entry per requested reference,
/// since the server serves at most one entry per reference it was asked for.
fn max_fetch_transactions_response_bytes(
    context: &Context,
    requested_transactions: usize,
) -> usize {
    buffer_bytes(
        requested_transactions,
        max_serialized_transactions_entry_bytes(context),
    )
}

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
        let client = ConsensusServiceClient::new(channel)
            .max_encoding_message_size(config.request_message_size_limit())
            .max_decoding_message_size(config.message_size_limit)
            .send_compressed(CompressionEncoding::Zstd)
            .accept_compressed(CompressionEncoding::Zstd);
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
        let commit_sync = highest_accepted_rounds.is_empty();
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
        let stream = client
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

        collect_block_headers(&self.context, peer, stream, commit_sync).await
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
        let mut headers = vec![];
        let mut total_fetched_bytes = 0;
        let max_headers = authorities.len();
        let max_allowed_bytes = max_headers
            .saturating_mul(max_signed_block_header_bytes(self.context.committee.size()));
        loop {
            match stream.message().await {
                Ok(Some(response)) => {
                    let vec_serialized_block_headers = response.vec_serialized_block_header;
                    let received_headers = headers
                        .len()
                        .saturating_add(vec_serialized_block_headers.len());
                    if received_headers > max_headers {
                        return Err(ConsensusError::TooManyFetchedHeadersReturned {
                            peer,
                            requested: max_headers,
                            received: received_headers,
                        });
                    }
                    for b in &vec_serialized_block_headers {
                        total_fetched_bytes += b.len();
                    }
                    headers.extend(vec_serialized_block_headers);
                    if total_fetched_bytes > max_allowed_bytes {
                        info!(
                            "fetch_latest_block_headers() fetched bytes exceeded limit: {} > {}, terminating stream.",
                            total_fetched_bytes, max_allowed_bytes,
                        );
                        break;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    if headers.is_empty() {
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
        Ok(headers)
    }

    async fn fetch_transactions(
        &self,
        peer: AuthorityIndex,
        transactions_refs: Vec<TransactionRef>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>> {
        let mut client = self.get_client(peer, timeout).await?;
        let transaction_refs: Vec<Vec<u8>> = transactions_refs
            .iter()
            .filter_map(|tx_ref| match bcs::to_bytes(tx_ref) {
                Ok(serialized) => Some(serialized),
                Err(e) => {
                    debug!("Failed to serialize TransactionRef {:?}: {e:?}", tx_ref);
                    None
                }
            })
            .collect();
        let requested_transactions = transaction_refs.len();
        let mut request = Request::new(FetchTransactionsRequest { transaction_refs });

        request.set_timeout(timeout);
        let stream = client
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

        collect_transactions(&self.context, peer, stream, requested_transactions).await
    }

    async fn fetch_commits_and_transactions(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>, Option<ConsensusError>)> {
        let mut client = self.get_client(peer, timeout).await?;
        let mut request = Request::new(FetchCommitsAndTransactionsRequest {
            start: commit_range.start(),
            end: commit_range.end(),
        });
        request.set_timeout(timeout);
        let stream = client
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

        collect_commits_and_transactions(&self.context, peer, &commit_range, stream).await
    }
}

/// Collects the chunks of a `fetch_commits_and_transactions` response stream
/// into the commit, certifier-header and transaction buffers. A stream cut by
/// an error after commits arrived yields the delivered chunks plus the error.
async fn collect_commits_and_transactions<S>(
    context: &Context,
    peer: AuthorityIndex,
    commit_range: &CommitRange,
    mut stream: S,
) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>, Option<ConsensusError>)>
where
    S: Stream<Item = Result<FetchCommitsAndTransactionsResponse, tonic::Status>> + Unpin,
{
    // First chunk contains commits and certifier headers.
    //
    // Bound the response per element and per category while streaming, since
    // `verify_commits` only runs on the fully-received buffers and so cannot
    // protect them from a malicious server. Commits and certifier headers
    // carry the same count caps `verify_commits` applies (twice the requested
    // range and two headers per authority).
    // Transactions carry no configured count cap on the fast path, since the
    // server returns every transaction the committed range references. The
    // commits already received bound them instead: a commit holds at most one
    // `TransactionRef` per 37 bytes it occupies, and the server serves one
    // entry per reference.
    let committee_size = context.committee.size();
    let gc_depth = context.protocol_config.gc_depth() as usize;
    let max_commits = CommitSyncType::Fast.max_commits_per_response(commit_range);
    let max_certifier_headers =
        committee_size.saturating_mul(MAX_COMMIT_VOTE_HEADERS_PER_AUTHORITY);
    let max_commit_size = max_commit_bytes(committee_size, gc_depth);
    let max_header_size = max_signed_block_header_bytes(committee_size);
    let max_transaction_size = max_serialized_transactions_entry_bytes(context);
    // Coarse total backstop for the buffer. The commit and certifier-header
    // terms reuse the per-category caps above so the total never trips
    // before them; the transaction term uses the commit-sync fetch cap as a
    // coarse allowance. An empty entry still costs its `Bytes` descriptor,
    // so it is charged to the total as well.
    let max_allowed_bytes = buffer_bytes(max_commits, max_commit_size)
        .saturating_add(buffer_bytes(max_certifier_headers, max_header_size))
        .saturating_add(buffer_bytes(
            context.parameters.max_transactions_per_commit_sync_fetch,
            max_transaction_size,
        ));

    let mut commits = Vec::new();
    let mut certifier_block_headers = Vec::new();
    let mut transactions = Vec::new();
    let mut max_transactions = 0usize;
    let mut total_fetched_bytes = 0;
    let mut stream_error = None;

    loop {
        match stream.try_next().await {
            Ok(Some(response)) => {
                // Commits (typically in the first chunk): count cap + per-element size.
                if commits.len() + response.commits.len() > max_commits {
                    return Err(ConsensusError::TooManyCommitsFromPeer {
                        peer,
                        count: (commits.len() + response.commits.len()) as CommitIndex,
                        limit: max_commits as CommitIndex,
                    });
                }
                for c in &response.commits {
                    if c.len() > max_commit_size {
                        return Err(ConsensusError::SerializedCommitTooLarge {
                            peer,
                            size: c.len(),
                            limit: max_commit_size,
                        });
                    }
                    total_fetched_bytes += c.len() + size_of::<Bytes>();
                    max_transactions =
                        max_transactions.saturating_add(c.len() / SERIALIZED_TRANSACTION_REF_BYTES);
                }
                commits.extend(response.commits);

                // Certifier headers: count cap + per-element size.
                if certifier_block_headers.len() + response.certifier_block_headers.len()
                    > max_certifier_headers
                {
                    return Err(ConsensusError::TooManyCommitVoteHeaders {
                        peer,
                        count: certifier_block_headers.len()
                            + response.certifier_block_headers.len(),
                        limit: max_certifier_headers,
                    });
                }
                for h in &response.certifier_block_headers {
                    if h.len() > max_header_size {
                        return Err(ConsensusError::SerializedBlockHeaderTooLarge {
                            peer,
                            size: h.len(),
                            limit: max_header_size,
                        });
                    }
                    total_fetched_bytes += h.len() + size_of::<Bytes>();
                }
                certifier_block_headers.extend(response.certifier_block_headers);

                // Transactions (streamed in subsequent chunks): count cap from
                // the commits received + per-element size.
                if transactions
                    .len()
                    .saturating_add(response.transactions.len())
                    > max_transactions
                {
                    return Err(ConsensusError::TooManyFetchedTransactionsReturned(peer));
                }
                for t in &response.transactions {
                    if t.len() > max_transaction_size {
                        return Err(ConsensusError::SerializedTransactionsTooLarge {
                            size: t.len(),
                            limit: max_transaction_size,
                        });
                    }
                    total_fetched_bytes += t.len() + size_of::<Bytes>();
                }
                transactions.extend(response.transactions);

                // Coarse total backstop bounding the transaction buffer, which
                // has no precise count cap on the fast path.
                if total_fetched_bytes > max_allowed_bytes {
                    info!(
                        "fetch_commits_and_transactions() fetched bytes exceeded limit: {} > {}, terminating stream.",
                        total_fetched_bytes, max_allowed_bytes,
                    );
                    break;
                }
            }
            Ok(None) => {
                break;
            }
            Err(e) => {
                let error = if e.code() == tonic::Code::DeadlineExceeded {
                    ConsensusError::NetworkRequestTimeout(format!(
                        "fetch_commits_and_transactions failed mid-stream: {e:?}"
                    ))
                } else {
                    ConsensusError::NetworkRequest(format!(
                        "fetch_commits_and_transactions failed mid-stream: {e:?}"
                    ))
                };
                if commits.is_empty() {
                    return Err(error);
                }
                // Keep what arrived and surface the cut alongside it: only
                // the caller can tell whether the delivered chunks cover
                // anything usable.
                warn!("fetch_commits_and_transactions from {peer} failed mid-stream: {e:?}");
                stream_error = Some(error);
                break;
            }
        }
    }

    Ok((commits, certifier_block_headers, transactions, stream_error))
}

/// Collects the chunks of a `fetch_block_headers` response stream into the
/// header buffer. A stream cut by an error after headers arrived yields the
/// delivered chunks.
///
/// Commit sync rejects more headers than the server's cap allows, since the
/// server returns only what was requested. Header sync accepts the peer's
/// gap-fill up to the ceiling every configuration respects, and stops reading
/// once our own cap is held; the caller trims to it.
async fn collect_block_headers<S>(
    context: &Context,
    peer: AuthorityIndex,
    mut stream: S,
    commit_sync: bool,
) -> ConsensusResult<Vec<Bytes>>
where
    S: Stream<Item = Result<FetchBlockHeadersResponse, tonic::Status>> + Unpin,
{
    let max_headers = max_fetched_headers(context, commit_sync);
    let max_allowed_bytes = max_fetch_block_headers_response_bytes(context, commit_sync);
    let wanted_headers = context.parameters.max_headers_per_fetch(commit_sync);
    let mut vec_serialized_block_header = vec![];
    let mut total_fetched_bytes = 0;
    loop {
        match stream.try_next().await {
            Ok(Some(response)) => {
                let headers = response.vec_serialized_block_header;
                let received = vec_serialized_block_header
                    .len()
                    .saturating_add(headers.len());
                if received > max_headers {
                    return Err(ConsensusError::TooManyFetchedHeadersReturned {
                        peer,
                        requested: max_headers,
                        received,
                    });
                }
                for b in &headers {
                    // An empty entry still costs its `Bytes` descriptor.
                    total_fetched_bytes += b.len() + size_of::<Bytes>();
                }
                if total_fetched_bytes > max_allowed_bytes {
                    info!(
                        "fetch_block_headers() fetched bytes exceeded limit: {} > {}, terminating stream.",
                        total_fetched_bytes, max_allowed_bytes,
                    );
                    break;
                }
                vec_serialized_block_header.extend(headers);
                if !commit_sync && vec_serialized_block_header.len() >= wanted_headers {
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

/// Collects the chunks of a `fetch_transactions` response stream into the
/// transaction buffer. A stream cut by an error after entries arrived yields
/// the delivered chunks.
///
/// The entry count is bounded to the request, since the server serves one
/// entry per reference it was asked for.
async fn collect_transactions<S>(
    context: &Context,
    peer: AuthorityIndex,
    mut stream: S,
    requested_transactions: usize,
) -> ConsensusResult<Vec<Bytes>>
where
    S: Stream<Item = Result<FetchTransactionsResponse, tonic::Status>> + Unpin,
{
    let max_entry_bytes = max_serialized_transactions_entry_bytes(context);
    let max_allowed_bytes = max_fetch_transactions_response_bytes(context, requested_transactions);
    let mut total_fetched_bytes = 0;
    let mut vec_serialized_transactions = vec![];
    loop {
        match stream.try_next().await {
            Ok(Some(response)) => {
                let transactions = response.vec_serialized_transactions;
                if vec_serialized_transactions
                    .len()
                    .saturating_add(transactions.len())
                    > requested_transactions
                {
                    return Err(ConsensusError::TooManyFetchedTransactionsReturned(peer));
                }
                for b in &transactions {
                    if b.len() > max_entry_bytes {
                        return Err(ConsensusError::SerializedTransactionsTooLarge {
                            size: b.len(),
                            limit: max_entry_bytes,
                        });
                    }
                    // An empty entry still costs its `Bytes` descriptor.
                    total_fetched_bytes += b.len() + size_of::<Bytes>();
                }
                if total_fetched_bytes > max_allowed_bytes {
                    info!(
                        "fetch_transactions() fetched bytes exceeded limit: {} > {}, terminating stream.",
                        total_fetched_bytes, max_allowed_bytes,
                    );
                    break;
                }
                vec_serialized_transactions.extend(transactions);
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
    /// Held while connecting to the peer at that index, so callers that miss
    /// the pool at the same time share one connection instead of each
    /// opening their own.
    connecting: Vec<Mutex<()>>,
}

impl ChannelPool {
    fn new(context: Arc<Context>) -> Self {
        let connecting = (0..context.committee.size())
            .map(|_| Mutex::new(()))
            .collect();
        Self {
            context,
            channels: RwLock::new(BTreeMap::new()),
            connecting,
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

        let _connecting = self.connecting[peer.value()].lock().await;
        // Another caller may have connected while this one waited for the lock.
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
            .map_err(|e| ConsensusError::NetworkConfig(format!("invalid URI '{address}': {e}")))?
            .connect_timeout(timeout)
            .initial_connection_window_size(Some(buffer_size as u32))
            .initial_stream_window_size(Some(buffer_size as u32 / 2))
            .keep_alive_while_idle(true)
            .keep_alive_timeout(config.keepalive_interval)
            .http2_keep_alive_interval(config.keepalive_interval)
            // tcp keepalive is probably unnecessary and is unsupported by msim.
            .user_agent("starfish")
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

        self.channels.write().insert(peer, channel.clone());
        Ok(channel)
    }
}

/// Proxies Tonic requests to NetworkService with actual handler implementation.
struct TonicServiceProxy<S: NetworkService> {
    context: Arc<Context>,
    service: Arc<S>,
    admission: PerPeerAdmission,
}

impl<S: NetworkService> TonicServiceProxy<S> {
    fn new(context: Arc<Context>, service: Arc<S>) -> Self {
        let admission = PerPeerAdmission::new(&context);
        Self {
            context,
            service,
            admission,
        }
    }

    /// Admits one request from `peer` in `group`, returning a permit to hold
    /// for the request's (or stream's) lifetime, or `None` when the group's
    /// limit is disabled. A rejected request increments the admission
    /// metric and returns `ResourceExhausted` so the peer backs off.
    fn admit(
        &self,
        group: RpcGroup,
        peer: AuthorityIndex,
    ) -> Result<Option<AdmissionGuard>, tonic::Status> {
        match self.admission.try_acquire(group, peer) {
            Admission::Unlimited => Ok(None),
            Admission::Permit(permit) => {
                let in_use = self
                    .context
                    .metrics
                    .network_metrics
                    .admission_in_use
                    .with_label_values(&[group.as_str()]);
                Ok(Some(AdmissionGuard::new(permit, in_use)))
            }
            Admission::Rejected => {
                self.context
                    .metrics
                    .network_metrics
                    .admission_rejected
                    .with_label_values(&[group.as_str()])
                    .inc();
                Err(tonic::Status::resource_exhausted(format!(
                    "per-peer {} limit reached",
                    group.as_str()
                )))
            }
        }
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
        // Acquire before reading the request stream so a peer cannot stack
        // half-open subscriptions; the permit is held for the stream's lifetime.
        let permit = self.admit(RpcGroup::Subscribe, peer_index)?;
        let mut request_stream = request.into_inner();
        let subscribe_request_timeout = self.context.parameters.tonic.subscribe_request_timeout;
        let first_message = if subscribe_request_timeout.is_zero() {
            request_stream.next().await
        } else {
            match tokio::time::timeout(subscribe_request_timeout, request_stream.next()).await {
                Ok(message) => message,
                Err(_) => {
                    debug!(
                        "subscribe_block_bundles() request from {} not received within {:?}",
                        peer_index, subscribe_request_timeout
                    );
                    return Err(tonic::Status::deadline_exceeded(
                        "Subscription request not received in time",
                    ));
                }
            }
        };
        let first_request = match first_message {
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
        Ok(Response::new(
            PermitGuardedStream::new(rate_limited_stream, permit).boxed(),
        ))
    }

    type FetchBlockHeadersStream =
        Pin<Box<dyn Stream<Item = Result<FetchBlockHeadersResponse, tonic::Status>> + Send>>;

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
        let permit = self.admit(RpcGroup::HeaderFetch, peer_index)?;
        let inner = request.into_inner();
        let highest_accepted_rounds = inner.highest_accepted_rounds;
        let max_fetch_size = self
            .context
            .parameters
            .max_headers_per_fetch(highest_accepted_rounds.is_empty());
        if inner.block_refs.len() > max_fetch_size {
            warn!(
                "Truncated fetch headers request from {} to {} blocks for peer {}",
                inner.block_refs.len(),
                max_fetch_size,
                peer_index
            );
        }
        let block_refs = inner
            .block_refs
            .into_iter()
            .take(max_fetch_size)
            .filter_map(|serialized| match bcs::from_bytes(&serialized) {
                Ok(r) => Some(r),
                Err(e) => {
                    debug!("Failed to deserialize block ref {:?}: {e:?}", serialized);
                    None
                }
            })
            .collect();
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
        let stream = PermitGuardedStream::new(iter(responses), permit).boxed();
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
        let _permit = self.admit(RpcGroup::CommitFetch, peer_index)?;
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

    type FetchCommitsAndTransactionsStream = Pin<
        Box<dyn Stream<Item = Result<FetchCommitsAndTransactionsResponse, tonic::Status>> + Send>,
    >;

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
        let permit = self.admit(RpcGroup::CommitFetch, peer_index)?;
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

        let stream = PermitGuardedStream::new(iter(responses), permit).boxed();
        Ok(Response::new(stream))
    }

    type FetchLatestBlockHeadersStream =
        Pin<Box<dyn Stream<Item = Result<FetchLatestBlockHeadersResponse, tonic::Status>> + Send>>;

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
        let permit = self.admit(RpcGroup::HeaderFetch, peer_index)?;
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
        let stream = PermitGuardedStream::new(iter(responses), permit).boxed();
        Ok(Response::new(stream))
    }

    async fn get_latest_rounds(
        &self,
        _request: Request<GetLatestRoundsRequest>,
    ) -> Result<Response<GetLatestRoundsResponse>, tonic::Status> {
        // This RPC is kept in the service definition for backward compatibility,
        // but is not supported by Starfish.
        error!("get_latest_rounds() is deprecated in starfish and should not be called");
        Err(tonic::Status::unimplemented(
            "get_latest_rounds is deprecated and not supported",
        ))
    }

    type FetchTransactionsStream =
        Pin<Box<dyn Stream<Item = Result<FetchTransactionsResponse, tonic::Status>> + Send>>;

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
        let permit = self.admit(RpcGroup::TransactionFetch, peer_index)?;

        let request = request.into_inner();
        let committed_transactions_refs: Vec<TransactionRef> = request
            .transaction_refs
            .iter()
            .filter_map(|r| match bcs::from_bytes::<TransactionRef>(r) {
                Ok(transaction_ref) => Some(transaction_ref),
                Err(e) => {
                    debug!("Failed to deserialize transaction ref: {e:?}");
                    None
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
        let stream = PermitGuardedStream::new(iter(responses), permit).boxed();
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

/// Long-lived server-streaming RPCs exempt from the server-side fallback
/// request timeout: they carry no client `grpc-timeout`, so a deadline would
/// abort an otherwise healthy subscription. Bounded RPCs are not listed.
const TIMEOUT_EXEMPT_PATHS: &[&str] = &["/consensus.ConsensusService/SubscribeBlockBundles"];

/// Connections a single committee peer may hold on the consensus listener at
/// once. One is enough to serve a peer: the channel pool keeps a single
/// connection per authority and multiplexes every RPC over it. The rest is
/// headroom for a reconnect whose predecessor has not been reaped yet.
const MAX_CONNECTIONS_PER_PEER: usize = 4;

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
            .map_request({
                let connections_info = connections_info.clone();
                move |mut request: http::Request<_>| {
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
                }
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
            .layer_fn({
                // A zero `request_timeout` disables the server-side fallback deadline.
                let server_timeout =
                    (!config.request_timeout.is_zero()).then_some(config.request_timeout);
                move |service| {
                    iota_network_stack::grpc_timeout::GrpcTimeout::new_with_exempt_paths(
                        service,
                        server_timeout,
                        TIMEOUT_EXEMPT_PATHS,
                    )
                }
            });

        let consensus_service_server = ConsensusServiceServer::new(service)
            .max_encoding_message_size(config.message_size_limit)
            .max_decoding_message_size(config.request_message_size_limit())
            .send_compressed(CompressionEncoding::Zstd)
            .accept_compressed(CompressionEncoding::Zstd);

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
            .max_concurrent_streams(
                (config.max_concurrent_streams > 0).then_some(config.max_concurrent_streams),
            )
            .http2_keepalive_interval(Some(config.keepalive_interval))
            .http2_keepalive_timeout(Some(config.keepalive_interval))
            .accept_http1(false)
            .max_connections_per_peer(Some(MAX_CONNECTIONS_PER_PEER))
            .on_peer_connection_event({
                let context = self.context.clone();
                let connections_info = connections_info.clone();
                move |peer_public_key, event| {
                    let Some(authority_index) =
                        authority_index_from_key(&connections_info, peer_public_key)
                    else {
                        return;
                    };
                    let hostname = &context.committee.authority(authority_index).hostname;
                    let network_metrics = &context.metrics.network_metrics;
                    match event {
                        PeerConnectionEvent::Established { held }
                        | PeerConnectionEvent::Closed { held } => network_metrics
                            .inbound_connections
                            .with_label_values(&[hostname])
                            .set(held as i64),
                        PeerConnectionEvent::RefusedAtLimit { .. } => network_metrics
                            .inbound_connections_refused
                            .with_label_values(&[hostname])
                            .inc(),
                    }
                }
            });

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

/// Resolves a peer's raw network public key to its index in the committee.
fn authority_index_from_key(
    connections_info: &ConnectionsInfo,
    public_key: &[u8],
) -> Option<AuthorityIndex> {
    let public_key = Ed25519PublicKey::from_bytes(public_key).ok()?;
    connections_info.authority_index(&NetworkPublicKey::new(public_key))
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
        (Some(Protocol::Ip6(ipaddr)), Some(Protocol::Udp(port))) => {
            Ok(format!("{}", SocketAddrV6::new(ipaddr, port, 0, 0)))
        }
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

/// Path prefix the consensus service is served under.
const CONSENSUS_SERVICE_PATH_PREFIX: &str = "/consensus.ConsensusService/";

/// Methods served by the consensus service, each recorded under its own metric
/// label.
const CONSENSUS_SERVICE_METHODS: &[&str] = &[
    "SubscribeBlockBundles",
    "FetchBlockHeaders",
    "FetchCommits",
    "FetchCommitsAndTransactions",
    "FetchLatestBlockHeaders",
    "GetLatestRounds",
    "FetchTransactions",
];

/// Label recorded for every path that is not a served method.
const UNKNOWN_ROUTE: &str = "unknown";

/// Metric label for a request path: the name of the served method, or
/// `unknown`. Callers choose the path, so the label never derives from it.
fn route_label(path: &str) -> &'static str {
    path.strip_prefix(CONSENSUS_SERVICE_PATH_PREFIX)
        .and_then(|method| {
            CONSENSUS_SERVICE_METHODS
                .iter()
                .find(|served| **served == method)
                .copied()
        })
        .unwrap_or(UNKNOWN_ROUTE)
}

/// Error label for a failed HTTP status, `None` for a successful one.
fn response_error_type(response: &http::response::Parts) -> Option<String> {
    (!response.status.is_success()).then(|| response.status.to_string())
}

impl MakeCallbackHandler for MetricsCallbackMaker {
    type Handler = MetricsResponseCallback;

    fn make_handler(&self, request: &http::request::Parts) -> Self::Handler {
        self.handle_request(route_label(request.uri.path()))
    }
}

impl ResponseHandler for MetricsResponseCallback {
    fn on_response(&mut self, response: &http::response::Parts) {
        MetricsResponseCallback::on_response(self, response_error_type(response).as_deref())
    }

    fn on_error<E>(&mut self, err: &E) {
        MetricsResponseCallback::on_error(self, err)
    }

    fn on_body_chunk<B>(&mut self, chunk: &B)
    where
        B: bytes::Buf,
    {
        // Body data is `Bytes`, so the first chunk is the whole buffer.
        debug_assert_eq!(chunk.chunk().len(), chunk.remaining());
        self.on_chunk(chunk.chunk());
    }

    fn on_end_of_stream(&mut self, _trailers: Option<&http::HeaderMap>) {
        MetricsResponseCallback::on_end_of_stream(self);
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
    // BCS-serialized `TransactionRef`s.
    #[prost(bytes = "vec", repeated, tag = "1")]
    transaction_refs: Vec<Vec<u8>>,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use futures::stream;
    use starfish_config::{AuthorityIndex, MAX_HEADERS_PER_HEADER_SYNC_FETCH};

    use super::{
        CONSENSUS_SERVICE_METHODS, CONSENSUS_SERVICE_PATH_PREFIX, FetchBlockHeadersResponse,
        FetchCommitsAndTransactionsResponse, FetchTransactionsResponse, UNKNOWN_ROUTE,
        collect_block_headers, collect_commits_and_transactions, collect_transactions,
        max_fetch_block_headers_response_bytes, max_fetch_transactions_response_bytes,
        max_serialized_transactions_entry_bytes, route_label,
    };
    use crate::{
        block_header::max_signed_block_header_bytes,
        block_verifier::{MAX_BCS_LENGTH_PREFIX_BYTES, serialized_transactions_size_limit},
        commit::CommitRange,
        context::Context,
        error::ConsensusError,
        transaction_ref::SERIALIZED_TRANSACTION_REF_BYTES,
    };

    fn chunk(commits: usize, transactions: usize) -> FetchCommitsAndTransactionsResponse {
        FetchCommitsAndTransactionsResponse {
            // Sized for four transaction references, since the transaction
            // count is derived from the commit bytes.
            commits: vec![Bytes::from(vec![0u8; 4 * SERIALIZED_TRANSACTION_REF_BYTES]); commits],
            certifier_block_headers: vec![],
            transactions: vec![Bytes::from_static(b"transaction"); transactions],
        }
    }

    /// Wide enough for the tests below to trip the cap each one exercises.
    fn requested_range() -> CommitRange {
        (1..=10).into()
    }

    fn commit_chunk(commit_bytes: usize) -> FetchCommitsAndTransactionsResponse {
        FetchCommitsAndTransactionsResponse {
            commits: vec![Bytes::from(vec![0u8; commit_bytes])],
            certifier_block_headers: vec![],
            transactions: vec![],
        }
    }

    /// A commit can reference one transaction per 37 bytes it occupies, so a
    /// peer sending more entries than its commits account for is rejected.
    #[tokio::test]
    async fn transactions_past_what_the_commits_reference_are_rejected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let flood = stream::iter([
            Ok(commit_chunk(SERIALIZED_TRANSACTION_REF_BYTES)),
            Ok(chunk(0, 2)),
        ]);

        let result =
            collect_commits_and_transactions(&context, peer, &requested_range(), flood).await;

        assert!(matches!(
            result,
            Err(ConsensusError::TooManyFetchedTransactionsReturned(_))
        ));
    }

    /// Entries the received commits account for are collected in full.
    #[tokio::test]
    async fn transactions_within_what_the_commits_reference_are_collected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let full = stream::iter([
            Ok(commit_chunk(2 * SERIALIZED_TRANSACTION_REF_BYTES)),
            Ok(chunk(0, 2)),
        ]);

        let (commits, _headers, transactions, _error) =
            collect_commits_and_transactions(&context, peer, &requested_range(), full)
                .await
                .expect("a response the commits account for is kept");

        assert_eq!(commits.len(), 1);
        assert_eq!(transactions.len(), 2);
    }

    fn header_chunk(headers: usize, bytes_each: usize) -> FetchBlockHeadersResponse {
        FetchBlockHeadersResponse {
            vec_serialized_block_header: vec![Bytes::from(vec![0u8; bytes_each]); headers],
        }
    }

    /// A peer returning more headers than the server-side cap is rejected
    /// while the response is still streaming, before its chunks are kept.
    #[tokio::test]
    async fn headers_past_the_count_cap_are_rejected() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.parameters.max_headers_per_commit_sync_fetch = 3;
        let peer = AuthorityIndex::new_for_test(1);
        let flood = stream::iter([Ok(header_chunk(2, 6)), Ok(header_chunk(2, 6))]);

        let result = collect_block_headers(&context, peer, flood, true).await;

        assert!(matches!(
            result,
            Err(ConsensusError::TooManyFetchedHeadersReturned {
                requested: 3,
                received: 4,
                ..
            })
        ));
    }

    /// A response of maximum-size headers filling the cap fits the budget.
    #[tokio::test]
    async fn maximal_headers_at_the_count_cap_are_collected() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.parameters.max_headers_per_commit_sync_fetch = 3;
        let peer = AuthorityIndex::new_for_test(1);
        let header_bytes = max_signed_block_header_bytes(context.committee.size());
        let maximal = stream::iter([
            Ok(header_chunk(2, header_bytes)),
            Ok(header_chunk(1, header_bytes)),
        ]);

        let headers = collect_block_headers(&context, peer, maximal, true)
            .await
            .expect("a maximal honest response is kept");

        assert_eq!(headers.len(), 3);
    }

    /// A response filling the cap exactly is still collected in full.
    #[tokio::test]
    async fn headers_at_the_count_cap_are_collected() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.parameters.max_headers_per_commit_sync_fetch = 3;
        let peer = AuthorityIndex::new_for_test(1);
        let full = stream::iter([Ok(header_chunk(2, 6)), Ok(header_chunk(1, 6))]);

        let headers = collect_block_headers(&context, peer, full, true)
            .await
            .expect("a response at the cap is kept");

        assert_eq!(headers.len(), 3);
    }

    fn transaction_chunk(transactions: usize, bytes_each: usize) -> FetchTransactionsResponse {
        FetchTransactionsResponse {
            vec_serialized_transactions: vec![Bytes::from(vec![0u8; bytes_each]); transactions],
        }
    }

    /// A peer returning more entries than the fetch asked for is rejected
    /// while the response is still streaming.
    #[tokio::test]
    async fn transactions_past_the_requested_count_are_rejected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let flood = stream::iter([Ok(transaction_chunk(2, 6)), Ok(transaction_chunk(2, 6))]);

        let result = collect_transactions(&context, peer, flood, 3).await;

        assert!(matches!(
            result,
            Err(ConsensusError::TooManyFetchedTransactionsReturned(_))
        ));
    }

    /// A response of maximum-size entries, one per requested reference, fits
    /// the budget.
    #[tokio::test]
    async fn maximal_transactions_at_the_requested_count_are_collected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let entry_bytes = max_serialized_transactions_entry_bytes(&context);
        let maximal = stream::iter([
            Ok(transaction_chunk(1, entry_bytes)),
            Ok(transaction_chunk(1, entry_bytes)),
        ]);

        let transactions = collect_transactions(&context, peer, maximal, 2)
            .await
            .expect("a maximal honest response is kept");

        assert_eq!(transactions.len(), 2);
    }

    /// One entry per requested reference is still collected in full.
    #[tokio::test]
    async fn transactions_at_the_requested_count_are_collected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let full = stream::iter([Ok(transaction_chunk(2, 6)), Ok(transaction_chunk(1, 6))]);

        let transactions = collect_transactions(&context, peer, full, 3)
            .await
            .expect("a response at the requested count is kept");

        assert_eq!(transactions.len(), 3);
    }

    /// An entry larger than a maximally full block's payload is rejected on
    /// arrival, naming the offending size.
    #[tokio::test]
    async fn oversized_transaction_entries_are_rejected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let limit = max_serialized_transactions_entry_bytes(&context);
        let oversized = stream::iter([Ok(transaction_chunk(1, limit + 1))]);

        let result = collect_transactions(&context, peer, oversized, 2).await;

        assert!(matches!(
            result,
            Err(ConsensusError::SerializedTransactionsTooLarge { .. })
        ));
    }

    /// A chunk that overruns the byte budget is dropped instead of landing in
    /// the buffer first.
    #[tokio::test]
    async fn headers_past_the_byte_budget_are_not_kept() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.parameters.max_headers_per_commit_sync_fetch = 2;
        let peer = AuthorityIndex::new_for_test(1);
        let budget = max_fetch_block_headers_response_bytes(&context, true);
        let overrunning =
            stream::iter([Ok(header_chunk(1, budget / 2)), Ok(header_chunk(1, budget))]);

        let headers = collect_block_headers(&context, peer, overrunning, true)
            .await
            .expect("the chunks within the budget are kept");

        assert_eq!(headers.len(), 1);
    }

    /// A header-sync peer configured above our cap is honest: its gap-fill is
    /// kept up to our cap and the rest of the stream is left unread.
    #[tokio::test]
    async fn header_sync_keeps_a_larger_peer_cap_up_to_our_own() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.parameters.max_headers_per_header_sync_fetch = 3;
        let peer = AuthorityIndex::new_for_test(1);
        let generous = stream::iter([
            Ok(header_chunk(2, 6)),
            Ok(header_chunk(2, 6)),
            Ok(header_chunk(2, 6)),
        ]);

        let headers = collect_block_headers(&context, peer, generous, false)
            .await
            .expect("a peer above our cap is not at fault");

        // The chunk that reaches our cap is kept whole; the one after it is
        // never read.
        assert_eq!(headers.len(), 4);
    }

    /// No configuration allows a header-sync response past the ceiling, so a
    /// peer sending one is rejected.
    #[tokio::test]
    async fn header_sync_rejects_headers_past_the_ceiling() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let flood = stream::iter([Ok(header_chunk(MAX_HEADERS_PER_HEADER_SYNC_FETCH + 1, 1))]);

        let result = collect_block_headers(&context, peer, flood, false).await;

        assert!(matches!(
            result,
            Err(ConsensusError::TooManyFetchedHeadersReturned {
                requested: MAX_HEADERS_PER_HEADER_SYNC_FETCH,
                received,
                ..
            }) if received == MAX_HEADERS_PER_HEADER_SYNC_FETCH + 1
        ));
    }

    /// A stream cut before anything arrived delivers nothing to keep, so the
    /// fetch fails outright.
    #[tokio::test]
    async fn cut_before_any_commit_fails_the_fetch() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let cut = stream::iter([Err(tonic::Status::unknown("h2 protocol error"))]);

        let result =
            collect_commits_and_transactions(&context, peer, &requested_range(), cut).await;

        assert!(matches!(result, Err(ConsensusError::NetworkRequest(_))));
    }

    /// A cut after commits arrived keeps the delivered chunks and returns the
    /// error alongside them, so the caller can attribute missing transactions
    /// to the connection instead of the peer's data.
    #[tokio::test]
    async fn cut_after_commits_keeps_the_delivered_chunks_and_the_error() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let cut = stream::iter([
            Ok(chunk(2, 0)),
            Ok(chunk(0, 3)),
            Err(tonic::Status::unknown("h2 protocol error")),
        ]);

        let (commits, _headers, transactions, stream_error) =
            collect_commits_and_transactions(&context, peer, &requested_range(), cut)
                .await
                .expect("a cut after commits arrived keeps them");

        assert_eq!(commits.len(), 2);
        assert_eq!(transactions.len(), 3);
        assert!(matches!(
            stream_error,
            Some(ConsensusError::NetworkRequest(_))
        ));
    }

    /// A stream that ends cleanly carries no error, so missing transactions
    /// stay attributable to the peer.
    #[tokio::test]
    async fn clean_end_carries_no_error() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let clean = stream::iter([Ok(chunk(2, 3))]);

        let (commits, _headers, transactions, stream_error) =
            collect_commits_and_transactions(&context, peer, &requested_range(), clean)
                .await
                .expect("a clean stream is kept in full");

        assert_eq!(commits.len(), 2);
        assert_eq!(transactions.len(), 3);
        assert!(stream_error.is_none());
    }

    /// A deadline reached mid-stream is reported as a timeout, so the caller
    /// can tell an expired request from a broken connection.
    #[tokio::test]
    async fn cut_by_the_deadline_reports_a_timeout() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let cut = stream::iter([
            Ok(chunk(2, 0)),
            Err(tonic::Status::deadline_exceeded("deadline")),
        ]);

        let (_commits, _headers, _transactions, stream_error) =
            collect_commits_and_transactions(&context, peer, &requested_range(), cut)
                .await
                .expect("a cut after commits arrived keeps them");

        assert!(matches!(
            stream_error,
            Some(ConsensusError::NetworkRequestTimeout(_))
        ));
    }

    /// The commit cap comes from the requested range, so a response filling the
    /// whole extension a server with a larger batch size may add is accepted.
    #[tokio::test]
    async fn commits_up_to_twice_the_requested_range_are_kept() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let requested: CommitRange = (1..=4).into();
        let maximal = stream::iter([Ok(chunk(8, 0))]);

        let (commits, _headers, _transactions, _error) =
            collect_commits_and_transactions(&context, peer, &requested, maximal)
                .await
                .expect("twice the requested range is within the cap");

        assert_eq!(commits.len(), 8);
    }

    /// One commit past twice the requested range is more than the extension
    /// can reach, so the response is rejected.
    #[tokio::test]
    async fn commits_past_twice_the_requested_range_are_rejected() {
        let (context, _keys) = Context::new_for_test(4);
        let peer = AuthorityIndex::new_for_test(1);
        let requested: CommitRange = (1..=4).into();
        let flood = stream::iter([Ok(chunk(9, 0))]);

        let result = collect_commits_and_transactions(&context, peer, &requested, flood).await;

        assert!(matches!(
            result,
            Err(ConsensusError::TooManyCommitsFromPeer {
                count: 9,
                limit: 8,
                ..
            })
        ));
    }

    /// The per-fetch response budgets track the matching server-side count cap:
    /// the value is the cap times the maximum per-item size, depends only on
    /// the cap and committee, and never on how many items the caller
    /// requested.
    #[tokio::test]
    async fn fetch_response_budgets_track_server_caps() {
        let (mut context, _keys) = Context::new_for_test(4);
        context.parameters.max_headers_per_commit_sync_fetch = 7;
        context.parameters.max_headers_per_header_sync_fetch = 11;
        context.parameters.max_transactions_per_commit_sync_fetch = 5;
        context
            .parameters
            .max_transactions_per_transaction_sync_fetch = 9;
        let committee_size = context.committee.size();
        let context = Arc::new(context);

        // Every entry is budgeted with the descriptor it occupies once collected.
        let header_entry = max_signed_block_header_bytes(committee_size) + size_of::<Bytes>();
        // Commit sync selects the commit-sync header cap.
        assert_eq!(
            max_fetch_block_headers_response_bytes(&context, true),
            7 * header_entry
        );
        // Header sync is budgeted at the ceiling every configuration respects,
        // since the peer's gap-fill follows its own cap.
        assert_eq!(
            max_fetch_block_headers_response_bytes(&context, false),
            MAX_HEADERS_PER_HEADER_SYNC_FETCH * header_entry
        );
        // The transaction budget is one maximum-size entry per requested
        // reference, independent of the transaction caps above.
        assert_eq!(
            max_fetch_transactions_response_bytes(&context, 3),
            3 * (serialized_transactions_size_limit(&context)
                + SERIALIZED_TRANSACTION_REF_BYTES
                + MAX_BCS_LENGTH_PREFIX_BYTES
                + size_of::<Bytes>())
        );
    }

    /// A peer that opens a block subscription stream but never sends the
    /// request starting it is disconnected once the deadline expires, instead
    /// of holding a subscription slot and its connection state indefinitely.
    #[cfg(not(msim))]
    #[tokio::test]
    async fn subscribe_without_request_hits_deadline() {
        use std::time::Duration;

        use futures::stream;
        use parking_lot::Mutex;
        use tonic::Request;

        use super::{SubscribeBlockBundlesRequest, TonicManager};
        use crate::network::test_network::TestService;

        const SUBSCRIBE_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

        let (context, keys) = Context::new_for_test(4);
        let server_index = context.committee.to_authority_index(0).unwrap();
        let mut server_context = context.clone().with_authority_index(server_index);
        server_context.parameters.tonic.subscribe_request_timeout = SUBSCRIBE_REQUEST_TIMEOUT;
        let mut server = TonicManager::new(Arc::new(server_context), keys[0].0.clone());
        server
            .install_service(Arc::new(Mutex::new(TestService::new())))
            .await;

        let client_context = Arc::new(
            context
                .clone()
                .with_authority_index(context.committee.to_authority_index(1).unwrap()),
        );
        let client =
            TonicManager::<Mutex<TestService>>::new(client_context, keys[1].0.clone()).client();

        let mut raw_client = client
            .get_client(server_index, Duration::from_secs(5))
            .await
            .unwrap();
        // A request stream that stays open without ever yielding the
        // subscription request.
        let request = Request::new(stream::pending::<SubscribeBlockBundlesRequest>());
        let status = tokio::time::timeout(
            SUBSCRIBE_REQUEST_TIMEOUT * 10,
            raw_client.subscribe_block_bundles(request),
        )
        .await
        .expect("server must not wait for the request past the deadline")
        .expect_err("subscription without a request must be rejected");

        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    }

    /// The stripped prefix has to match the generated service name, otherwise
    /// every request lands on the `unknown` label.
    #[test]
    fn path_prefix_matches_the_generated_service_name() {
        use crate::network::tonic_gen::consensus_service_server;

        assert_eq!(
            CONSENSUS_SERVICE_PATH_PREFIX,
            format!("/{}/", consensus_service_server::SERVICE_NAME)
        );
    }

    #[test]
    fn served_methods_keep_their_own_label() {
        for method in CONSENSUS_SERVICE_METHODS {
            let path = format!("{CONSENSUS_SERVICE_PATH_PREFIX}{method}");
            assert_eq!(route_label(&path), *method);
        }
    }

    #[test]
    fn other_paths_share_the_unknown_label() {
        for path in [
            "/consensus.ConsensusService/NotAMethod",
            "/consensus.ConsensusService/fetchcommits",
            "/consensus.ConsensusService/FetchCommits/extra",
            "/consensus.ConsensusService/",
            "/consensus.ConsensusService",
            "/other.Service/FetchCommits",
            "/FetchCommits",
            "/",
            "",
        ] {
            assert_eq!(route_label(path), UNKNOWN_ROUTE, "path: {path}");
        }
    }

    /// Unknown method names under the service path still reach the metrics
    /// layer, so they all have to land on one label.
    #[tokio::test]
    async fn unknown_methods_share_one_metric_label() {
        use std::time::Duration;

        use http::uri::PathAndQuery;
        use parking_lot::Mutex;
        use prometheus_filtered::core::Collector;
        use tonic::{Request, client::Grpc};
        use tonic_prost::ProstCodec;

        use super::{Channel, FetchCommitsRequest, FetchCommitsResponse, TonicManager};
        use crate::network::test_network::TestService;

        const UNKNOWN_METHODS: usize = 128;

        /// Route label values a metric retains, sorted.
        fn routes(metric: &impl Collector) -> Vec<String> {
            let mut routes: Vec<String> = metric
                .collect()
                .iter()
                .flat_map(|family| family.get_metric())
                .map(|metric| metric.get_label()[0].value().to_string())
                .collect();
            routes.sort();
            routes
        }

        async fn call(grpc: &mut Grpc<Channel>, path: &str) -> Result<(), tonic::Status> {
            grpc.ready().await.expect("the channel stays connected");
            grpc.unary::<_, FetchCommitsResponse, _>(
                Request::new(FetchCommitsRequest { start: 1, end: 2 }),
                PathAndQuery::try_from(path).unwrap(),
                ProstCodec::default(),
            )
            .await
            .map(|_| ())
        }

        let (context, keys) = Context::new_for_test(4);
        let server_index = context.committee.to_authority_index(0).unwrap();
        let server_context = Arc::new(context.clone().with_authority_index(server_index));
        let mut server = TonicManager::new(server_context.clone(), keys[0].0.clone());
        server
            .install_service(Arc::new(Mutex::new(TestService::new())))
            .await;

        let client_context = Arc::new(
            context
                .clone()
                .with_authority_index(context.committee.to_authority_index(1).unwrap()),
        );
        let client =
            TonicManager::<Mutex<TestService>>::new(client_context, keys[1].0.clone()).client();
        let channel = client
            .channel_pool
            .get_channel(
                client.network_keypair.clone(),
                server_index,
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        let mut grpc = Grpc::new(channel);

        for i in 0..UNKNOWN_METHODS {
            let status = call(&mut grpc, &format!("/consensus.ConsensusService/Method{i}"))
                .await
                .expect_err("an unknown method is not implemented");
            assert_eq!(status.code(), tonic::Code::Unimplemented);
        }

        let inbound = &server_context.metrics.network_metrics.inbound;
        assert_eq!(routes(&inbound.requests), ["unknown"]);
        assert_eq!(routes(&inbound.inflight_requests), ["unknown"]);
        assert_eq!(routes(&inbound.request_latency), ["unknown"]);
        assert_eq!(
            inbound.requests.with_label_values(&["unknown"]).get(),
            UNKNOWN_METHODS as u64
        );

        // A served method keeps its own label, whether it answers or rejects.
        call(&mut grpc, "/consensus.ConsensusService/FetchCommits")
            .await
            .expect("the served method answers");
        let status = call(&mut grpc, "/consensus.ConsensusService/GetLatestRounds")
            .await
            .expect_err("the deprecated method rejects");
        assert_eq!(status.code(), tonic::Code::Unimplemented);

        // A path outside the service matches no route, so it never reaches the
        // layer.
        call(&mut grpc, "/other.Service/FetchCommits")
            .await
            .expect_err("a path outside the service is not served");

        assert_eq!(
            routes(&inbound.requests),
            ["FetchCommits", "GetLatestRounds", "unknown"]
        );
    }
}
