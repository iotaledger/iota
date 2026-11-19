// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Admission control for the inbound consensus gRPC server.
//!
//! Each RPC group caps the requests served at once per committee peer, keyed
//! on the peer's authenticated authority index, so a misbehaving peer uses up
//! only its own slots. Commit fetches also have a node-wide cap, since their
//! responses hold memory and read the store until they have been sent. A peer
//! holding no commit fetch is granted one whatever the others hold, and that
//! fetch is not counted node-wide: the cap bounds only the further fetches all
//! peers share, so it limits how much a peer can be crowded out, never whether
//! it is served at all. Caps are local parameters; `0` turns a cap off.
//!
//! Whatever the caps, a peer whose request is refused for carrying more entries
//! than its type allows is charged a misbehavior.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    task::{Context as TaskContext, Poll, ready},
    time::Duration,
};

use bytes::Bytes;
use http::{Request, Response};
use http_body::Body as HttpBody;
use iota_network_stack::concurrency::ReclaimableBody;
use pin_project_lite::pin_project;
use prometheus_filtered::{IntCounter, IntGauge};
use starfish_config::AuthorityIndex;
use tonic::{Status, body::Body};
use tower::{Layer, Service};

use crate::{
    context::Context,
    error::ConsensusError,
    misbehavior_store::MisbehaviorStore,
    network::{
        bounded_codec::TOO_MANY_ENTRIES_IN_REQUEST,
        tonic_network::{
            CONSENSUS_SERVICE_PATH_PREFIX, DEPRECATED_METHOD, DEPRECATED_METHOD_MESSAGE, PeerInfo,
        },
    },
};

/// Inbound consensus RPCs grouped by cost and access pattern. Each group has its
/// own caps.
#[derive(Clone, Copy)]
pub(crate) enum RpcGroup {
    Subscribe,
    HeaderFetch,
    TransactionFetch,
    CommitFetch,
}

impl RpcGroup {
    /// Stable label for metrics.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RpcGroup::Subscribe => "subscribe",
            RpcGroup::HeaderFetch => "header_fetch",
            RpcGroup::TransactionFetch => "transaction_fetch",
            RpcGroup::CommitFetch => "commit_fetch",
        }
    }

    /// The group an inbound request path belongs to, or `None` for a path that
    /// is not capped.
    pub(crate) fn from_path(path: &str) -> Option<Self> {
        match path.strip_prefix(CONSENSUS_SERVICE_PATH_PREFIX)? {
            "SubscribeBlockBundles" => Some(RpcGroup::Subscribe),
            "FetchBlockHeaders" | "FetchLatestBlockHeaders" => Some(RpcGroup::HeaderFetch),
            "FetchTransactions" => Some(RpcGroup::TransactionFetch),
            "FetchCommits" | "FetchCommitsAndTransactions" => Some(RpcGroup::CommitFetch),
            _ => None,
        }
    }
}

/// Outcome of an admission attempt.
pub(crate) enum Admission {
    /// Admitted without a slot: both caps of the group are off.
    Unlimited,
    /// Admitted holding these slots; each is given back when dropped.
    Admitted(AdmissionSlots),
    /// A cap for this group is reached; the request must be rejected.
    Rejected(AdmissionLimit),
    /// The peer's index is outside the committee; the request must be refused.
    UnknownPeer,
}

/// The cap a rejected request ran into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdmissionLimit {
    /// The cap on requests served at once to the peer that sent it.
    Peer,
    /// The cap on requests of the group served at once to all peers together,
    /// beyond the one each peer is always granted.
    AllPeers,
}

impl AdmissionLimit {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            AdmissionLimit::Peer => "peer",
            AdmissionLimit::AllPeers => "all_peers",
        }
    }
}

/// Held while this node serves one inbound request.
pub(crate) struct AdmissionSlots {
    /// The peer's own slot, held until the response stream has ended.
    peer_slot: PeerSlot,
    /// The node-wide slot, given back once the response has been sent or its
    /// send deadline has passed; `None` when the peer held no slot of the
    /// group at admission, or when the group has no node-wide cap.
    total_slot: Option<TotalSlot>,
}

/// Slots of one RPC group held per peer and node-wide. A peer holding no slot
/// is always granted one, which is not counted node-wide. Its further slots are
/// counted, and granted only while fewer than `total_cap` are held. A cap of
/// `0` is off.
struct GroupSlots {
    per_peer_cap: u32,
    total_cap: u32,
    held_by_peer: Box<[AtomicU32]>,
    held_total: AtomicU32,
}

impl GroupSlots {
    /// `None` when both caps are off.
    fn new(size: usize, per_peer_cap: u32, total_cap: u32) -> Option<Arc<Self>> {
        (per_peer_cap > 0 || total_cap > 0).then(|| {
            Arc::new(Self {
                per_peer_cap,
                total_cap,
                held_by_peer: (0..size).map(|_| AtomicU32::new(0)).collect(),
                held_total: AtomicU32::new(0),
            })
        })
    }

    /// Tries to admit one request from the peer at `peer_index`, taking its
    /// slot and, if the peer already holds one and the group has a node-wide
    /// cap, a node-wide slot.
    fn try_take(self: &Arc<Self>, peer_index: usize) -> Admission {
        let Some(held_by_peer) = self.held_by_peer.get(peer_index) else {
            return Admission::UnknownPeer;
        };
        // The counters guard nothing else, so relaxed ordering is enough.
        let Ok(held_before) =
            held_by_peer.try_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
                (self.per_peer_cap == 0 || held < self.per_peer_cap).then_some(held + 1)
            })
        else {
            return Admission::Rejected(AdmissionLimit::Peer);
        };
        // From here a rejection gives the peer's slot back by dropping it.
        let peer_slot = PeerSlot {
            slots: self.clone(),
            peer_index,
        };
        if self.total_cap == 0 || held_before == 0 {
            return Admission::Admitted(AdmissionSlots {
                peer_slot,
                total_slot: None,
            });
        }
        let taken = self
            .held_total
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
                (held < self.total_cap).then_some(held + 1)
            });
        if taken.is_err() {
            return Admission::Rejected(AdmissionLimit::AllPeers);
        }
        Admission::Admitted(AdmissionSlots {
            peer_slot,
            total_slot: Some(TotalSlot {
                slots: self.clone(),
            }),
        })
    }
}

/// A peer's held slot; dropping it gives the slot back.
pub(crate) struct PeerSlot {
    slots: Arc<GroupSlots>,
    peer_index: usize,
}

impl Drop for PeerSlot {
    fn drop(&mut self) {
        self.slots.held_by_peer[self.peer_index].fetch_sub(1, Ordering::Relaxed);
    }
}

/// A held node-wide slot; dropping it gives the slot back.
pub(crate) struct TotalSlot {
    slots: Arc<GroupSlots>,
}

impl Drop for TotalSlot {
    fn drop(&mut self) {
        self.slots.held_total.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The slots of each RPC group; `None` for a group with both caps off.
pub(crate) struct AdmissionControl {
    subscribe: Option<Arc<GroupSlots>>,
    header: Option<Arc<GroupSlots>>,
    transaction: Option<Arc<GroupSlots>>,
    commit_fetch: Option<Arc<GroupSlots>>,
}

impl AdmissionControl {
    pub(crate) fn new(context: &Context) -> Self {
        let admission = &context.parameters.tonic.admission;
        let size = context.committee.size();
        Self {
            subscribe: GroupSlots::new(size, admission.max_subscriptions_per_peer, 0),
            header: GroupSlots::new(size, admission.max_header_fetches_per_peer, 0),
            transaction: GroupSlots::new(size, admission.max_transaction_fetches_per_peer, 0),
            commit_fetch: GroupSlots::new(
                size,
                admission.max_commit_fetches_per_peer,
                admission.max_commit_fetches_total,
            ),
        }
    }

    /// Tries to admit one request from `peer` in `group`.
    pub(crate) fn try_admit(&self, group: RpcGroup, peer: AuthorityIndex) -> Admission {
        let slots = match group {
            RpcGroup::Subscribe => &self.subscribe,
            RpcGroup::HeaderFetch => &self.header,
            RpcGroup::TransactionFetch => &self.transaction,
            RpcGroup::CommitFetch => &self.commit_fetch,
        };
        match slots {
            Some(slots) => slots.try_take(peer.value()),
            None => Admission::Unlimited,
        }
    }
}

/// RAII guard for an admitted request: holds the peer's slot and keeps the
/// per-group in-use gauge incremented. The response body and the reads its
/// handler runs for it share the guard, so the slot is released and the gauge
/// decremented once the last of them is dropped.
pub(crate) struct AdmissionGuard {
    _peer: PeerSlot,
    in_use: IntGauge,
}

impl AdmissionGuard {
    pub(crate) fn new(peer: PeerSlot, in_use: IntGauge) -> Self {
        in_use.inc();
        Self {
            _peer: peer,
            in_use,
        }
    }
}

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        self.in_use.dec();
    }
}

/// Tower layer taking an inbound request's slots before tonic reads the request
/// body. The peer's slot is held until the response ends, the node-wide slot
/// until then or until `send_timeout` passes with the peer not reading it. A
/// request refused for carrying more entries than its type allows is charged
/// to the peer in `misbehavior_store`.
#[derive(Clone)]
pub(crate) struct AdmissionLayer {
    context: Arc<Context>,
    admission: Arc<AdmissionControl>,
    send_timeout: Duration,
    misbehavior_store: Arc<MisbehaviorStore>,
}

impl AdmissionLayer {
    pub(crate) fn new(
        context: Arc<Context>,
        send_timeout: Duration,
        misbehavior_store: Arc<MisbehaviorStore>,
    ) -> Self {
        let admission = Arc::new(AdmissionControl::new(&context));
        Self {
            context,
            admission,
            send_timeout,
            misbehavior_store,
        }
    }
}

impl<S> Layer<S> for AdmissionLayer {
    type Service = AdmissionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AdmissionService {
            inner,
            context: self.context.clone(),
            admission: self.admission.clone(),
            send_timeout: self.send_timeout,
            misbehavior_store: self.misbehavior_store.clone(),
        }
    }
}

/// Answers a request over a cap with `ResourceExhausted` and passes every other
/// request on with its slots attached to the response body, which gives up
/// the response and the node-wide slot when the peer stops reading it. The
/// peer's slot also goes into the request extensions as an
/// `Arc<AdmissionGuard>`, for the handler to hold while it runs the reads for
/// the response.
#[derive(Clone)]
pub(crate) struct AdmissionService<S> {
    inner: S,
    context: Arc<Context>,
    admission: Arc<AdmissionControl>,
    send_timeout: Duration,
    misbehavior_store: Arc<MisbehaviorStore>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for AdmissionService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response<ReclaimableBody<Body, TotalSlot, Arc<AdmissionGuard>>>;
    type Error = S::Error;
    type Future = AdmissionFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<ReqBody>) -> Self::Future {
        let path = request.uri().path();
        // The deprecated method decodes its request before its handler answers,
        // so answering here is what keeps its body from being read.
        if path.strip_prefix(CONSENSUS_SERVICE_PATH_PREFIX) == Some(DEPRECATED_METHOD) {
            return AdmissionFuture::rejected(Status::unimplemented(DEPRECATED_METHOD_MESSAGE));
        }
        let Some(group) = RpcGroup::from_path(path) else {
            return AdmissionFuture::passed_through(self.inner.call(request));
        };
        let Some(peer) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|peer| peer.authority_index)
        else {
            return AdmissionFuture::rejected(Status::internal("PeerInfo not found"));
        };
        let metrics = &self.context.metrics.network_metrics;
        // A subscription stream is open for as long as the peer is subscribed.
        let send_deadline = (!matches!(group, RpcGroup::Subscribe)).then(|| {
            (
                self.send_timeout,
                metrics
                    .admission_reclaimed
                    .with_label_values(&[group.as_str()]),
            )
        });
        let sender = Some((peer, self.misbehavior_store.clone()));
        match self.admission.try_admit(group, peer) {
            Admission::UnknownPeer => {
                AdmissionFuture::rejected(Status::internal("peer index outside the committee"))
            }
            Admission::Unlimited => AdmissionFuture::admitted(
                self.inner.call(request),
                None,
                None,
                send_deadline,
                sender,
            ),
            Admission::Admitted(AdmissionSlots {
                peer_slot,
                total_slot,
            }) => {
                let in_use = metrics
                    .admission_in_use
                    .with_label_values(&[group.as_str()]);
                let guard = Arc::new(AdmissionGuard::new(peer_slot, in_use));
                request.extensions_mut().insert(guard.clone());
                AdmissionFuture::admitted(
                    self.inner.call(request),
                    Some(guard),
                    total_slot,
                    send_deadline,
                    sender,
                )
            }
            Admission::Rejected(limit) => {
                self.context
                    .metrics
                    .network_metrics
                    .admission_rejected
                    .with_label_values(&[group.as_str(), limit.as_str()])
                    .inc();
                let scope = match limit {
                    AdmissionLimit::Peer => "per-peer",
                    AdmissionLimit::AllPeers => "all-peers",
                };
                AdmissionFuture::rejected(Status::resource_exhausted(format!(
                    "{scope} {} limit reached",
                    group.as_str()
                )))
            }
        }
    }
}

pin_project! {
    #[project = AdmissionFutureProj]
    pub(crate) enum AdmissionFuture<F> {
        Admitted {
            #[pin]
            inner: F,
            guard: Option<Arc<AdmissionGuard>>,
            total: Option<TotalSlot>,
            // The send deadline and the counter of responses it reclaims;
            // `None` for a response that may stay open.
            send_deadline: Option<(Duration, IntCounter)>,
            // The peer that sent the request and the store that records its
            // misbehavior; `None` for a request outside every budget.
            sender: Option<(AuthorityIndex, Arc<MisbehaviorStore>)>,
        },
        Rejected {
            status: Status,
        },
    }
}

impl<F> AdmissionFuture<F> {
    fn admitted(
        inner: F,
        guard: Option<Arc<AdmissionGuard>>,
        total: Option<TotalSlot>,
        send_deadline: Option<(Duration, IntCounter)>,
        sender: Option<(AuthorityIndex, Arc<MisbehaviorStore>)>,
    ) -> Self {
        Self::Admitted {
            inner,
            guard,
            total,
            send_deadline,
            sender,
        }
    }

    /// A request in no RPC group, passed on untouched.
    fn passed_through(inner: F) -> Self {
        Self::admitted(inner, None, None, None, None)
    }

    fn rejected(status: Status) -> Self {
        Self::Rejected { status }
    }
}

impl<F, E, ResBody> Future for AdmissionFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Output = Result<Response<ReclaimableBody<Body, TotalSlot, Arc<AdmissionGuard>>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match self.project() {
            AdmissionFutureProj::Admitted {
                inner,
                guard,
                total,
                send_deadline,
                sender,
            } => {
                let response = ready!(inner.poll(cx));
                if let (Ok(response), Some((peer, misbehavior_store))) = (&response, sender.take())
                {
                    if refuses_too_many_entries(response) {
                        misbehavior_store.record_fault(
                            &ConsensusError::TooManyEntriesInRequest { peer },
                            None,
                            [peer],
                        );
                    }
                }
                Poll::Ready(response.map(|response| {
                    response.map(|body| match send_deadline.take() {
                        Some((deadline, reclaimed)) => ReclaimableBody::with_deadline(
                            Body::new(body),
                            total.take(),
                            guard.take(),
                            deadline,
                            move || reclaimed.inc(),
                        ),
                        None => ReclaimableBody::new(Body::new(body), total.take(), guard.take()),
                    })
                }))
            }
            AdmissionFutureProj::Rejected { status } => Poll::Ready(Ok(status
                .clone()
                .into_http()
                .map(|body| ReclaimableBody::new(body, None, None)))),
        }
    }
}

/// Whether `response` refuses its request for carrying more entries than the
/// request type allows. The code alone does not tell, since tonic refuses a
/// request over the message size limit with the same code.
fn refuses_too_many_entries<B>(response: &Response<B>) -> bool {
    Status::from_header_map(response.headers()).is_some_and(|status| {
        status.code() == tonic::Code::OutOfRange && status.message() == TOO_MANY_ENTRIES_IN_REQUEST
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(i: u8) -> AuthorityIndex {
        AuthorityIndex::from(i)
    }

    fn expect_admitted(outcome: Admission) -> AdmissionSlots {
        match outcome {
            Admission::Admitted(slots) => slots,
            _ => panic!("expected the request to be admitted"),
        }
    }

    #[tokio::test]
    async fn disabled_group_is_unlimited() {
        let admission = AdmissionControl {
            subscribe: GroupSlots::new(4, 0, 0),
            header: GroupSlots::new(4, 0, 0),
            transaction: GroupSlots::new(4, 0, 0),
            commit_fetch: GroupSlots::new(4, 0, 0),
        };
        for _ in 0..1000 {
            assert!(matches!(
                admission.try_admit(RpcGroup::HeaderFetch, peer(0)),
                Admission::Unlimited
            ));
            assert!(matches!(
                admission.try_admit(RpcGroup::CommitFetch, peer(0)),
                Admission::Unlimited
            ));
        }
    }

    #[tokio::test]
    async fn enforces_cap_and_releases_on_drop() {
        let admission = AdmissionControl {
            subscribe: None,
            header: GroupSlots::new(4, 2, 0),
            transaction: None,
            commit_fetch: None,
        };
        let p0 = expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(1)));
        let p1 = expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(1)));
        // A third concurrent request from the same peer exceeds the cap.
        assert!(matches!(
            admission.try_admit(RpcGroup::HeaderFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::Peer)
        ));
        // Dropping one request's slots frees exactly one slot.
        drop(p0);
        let p2 = expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(1)));
        drop((p1, p2));
    }

    #[tokio::test]
    async fn peers_are_isolated() {
        let admission = AdmissionControl {
            subscribe: None,
            header: GroupSlots::new(4, 1, 0),
            transaction: None,
            commit_fetch: None,
        };
        let held = expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(0)));
        // Peer 0 is saturated...
        assert!(matches!(
            admission.try_admit(RpcGroup::HeaderFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::Peer)
        ));
        // ...but peer 1 has its own cap.
        let _other = expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(1)));
        drop(held);
    }

    #[tokio::test]
    async fn only_commit_fetches_share_a_node_wide_cap() {
        let (mut context, _keys) = Context::new_for_test(4);
        let limits = &mut context.parameters.tonic.admission;
        limits.max_subscriptions_per_peer = 2;
        limits.max_header_fetches_per_peer = 2;
        limits.max_transaction_fetches_per_peer = 2;
        limits.max_commit_fetches_per_peer = 2;
        limits.max_commit_fetches_total = 1;
        let admission = AdmissionControl::new(&context);
        for group in [
            RpcGroup::Subscribe,
            RpcGroup::HeaderFetch,
            RpcGroup::TransactionFetch,
        ] {
            // Two peers fill their own caps; nothing is shared between them.
            let mut held: Vec<_> = [peer(0), peer(0), peer(1), peer(1)]
                .into_iter()
                .map(|p| expect_admitted(admission.try_admit(group, p)))
                .collect();
            assert!(held.iter().all(|slots| slots.total_slot.is_none()));
            assert!(matches!(
                admission.try_admit(group, peer(0)),
                Admission::Rejected(AdmissionLimit::Peer)
            ));
            // A released slot admits exactly one more request from its peer.
            held.remove(0);
            held.push(expect_admitted(admission.try_admit(group, peer(0))));
            assert!(matches!(
                admission.try_admit(group, peer(0)),
                Admission::Rejected(AdmissionLimit::Peer)
            ));
        }
        // Commit fetches share one slot beyond each peer's first.
        let _first = [peer(0), peer(1)]
            .map(|p| expect_admitted(admission.try_admit(RpcGroup::CommitFetch, p)));
        let _shared = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
    }

    #[tokio::test]
    async fn commit_fetches_have_a_node_wide_cap() {
        let admission = AdmissionControl {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: GroupSlots::new(4, 2, 1),
        };
        let p0 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p2 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(1)));
        // Peer 1 is within its own cap, but peer 0's second fetch holds the
        // only node-wide slot.
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // A rejection by the node-wide cap leaves the peer's own slot free.
        drop(p1);
        let p3 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(1)));
        drop((p0, p2, p3));
    }

    #[tokio::test]
    async fn all_peers_commit_limit_applies_without_a_per_peer_limit() {
        let admission = AdmissionControl {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: GroupSlots::new(4, 0, 2),
        };
        // The first fetch is not counted, the next two reach the cap.
        let p0 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p2 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop(p1);
        let p3 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        drop((p0, p2, p3));
    }

    #[tokio::test]
    async fn a_peers_first_commit_fetch_is_outside_the_node_wide_cap() {
        let admission = AdmissionControl {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: GroupSlots::new(4, 8, 2),
        };
        // Peer 0 reaches the node-wide cap alone, with its second and third
        // fetch.
        let p0 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p2 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // Every other peer still gets its first slot, and only that one.
        let p3 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(1)));
        let p4 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(2)));
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // The first slots take nothing from the node-wide count.
        let held_total = &admission.commit_fetch.as_ref().unwrap().held_total;
        assert_eq!(held_total.load(Ordering::Relaxed), 2);
        // A released first slot is granted again.
        drop(p3);
        let p5 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(1)));
        drop((p0, p1, p2, p4, p5));
        assert_eq!(held_total.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn ending_a_peers_first_fetch_frees_no_node_wide_slot() {
        let admission = AdmissionControl {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: GroupSlots::new(4, 8, 1),
        };
        let p0 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        // Peer 0 still holds a counted fetch, so another one is a further fetch
        // and the node-wide cap is still reached.
        drop(p0);
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop(p1);
        let p2 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        drop(p2);
    }

    #[tokio::test]
    async fn giving_back_the_node_wide_slot_alone_admits_another_fetch() {
        let admission = AdmissionControl {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: GroupSlots::new(4, 8, 1),
        };
        let p0 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let mut p1 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // The send deadline gives back the node-wide slot and keeps the peer's
        // own, so the peer may fetch again but stays charged for all of them.
        drop(p1.total_slot.take());
        let p2 = expect_admitted(admission.try_admit(RpcGroup::CommitFetch, peer(0)));
        let held_by_peer = &admission.commit_fetch.as_ref().unwrap().held_by_peer;
        assert_eq!(held_by_peer[0].load(Ordering::Relaxed), 3);
        assert!(matches!(
            admission.try_admit(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop((p0, p1, p2));
        assert_eq!(held_by_peer[0].load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn response_body_holds_the_peer_slot_until_dropped() {
        let admission = AdmissionControl {
            subscribe: GroupSlots::new(4, 1, 0),
            header: None,
            transaction: None,
            commit_fetch: None,
        };
        let gauge = IntGauge::new("test_subscribe_in_use", "test").unwrap();
        let slots = expect_admitted(admission.try_admit(RpcGroup::Subscribe, peer(2)));
        let guarded = ReclaimableBody::new(
            Body::default(),
            None::<TotalSlot>,
            Some(AdmissionGuard::new(slots.peer_slot, gauge.clone())),
        );
        // While the response body lives, the peer's single subscribe slot is
        // taken and the in-use gauge reflects it.
        assert_eq!(gauge.get(), 1);
        assert!(matches!(
            admission.try_admit(RpcGroup::Subscribe, peer(2)),
            Admission::Rejected(AdmissionLimit::Peer)
        ));
        // Dropping the body releases the slot and decrements the gauge.
        drop(guarded);
        assert_eq!(gauge.get(), 0);
        assert!(matches!(
            admission.try_admit(RpcGroup::Subscribe, peer(2)),
            Admission::Admitted(_)
        ));
    }

    #[tokio::test]
    async fn in_use_gauge_tracks_held_guards() {
        let admission = AdmissionControl {
            subscribe: None,
            header: GroupSlots::new(4, 2, 0),
            transaction: None,
            commit_fetch: None,
        };
        let gauge = IntGauge::new("test_header_in_use", "test").unwrap();
        let g0 = AdmissionGuard::new(
            expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(0))).peer_slot,
            gauge.clone(),
        );
        let g1 = AdmissionGuard::new(
            expect_admitted(admission.try_admit(RpcGroup::HeaderFetch, peer(1))).peer_slot,
            gauge.clone(),
        );
        assert_eq!(gauge.get(), 2);
        drop(g0);
        assert_eq!(gauge.get(), 1);
        drop(g1);
        assert_eq!(gauge.get(), 0);
    }

    #[test]
    fn request_paths_map_to_their_group() {
        let group =
            |method: &str| RpcGroup::from_path(&format!("{CONSENSUS_SERVICE_PATH_PREFIX}{method}"));

        assert!(matches!(
            group("SubscribeBlockBundles"),
            Some(RpcGroup::Subscribe)
        ));
        assert!(matches!(
            group("FetchBlockHeaders"),
            Some(RpcGroup::HeaderFetch)
        ));
        assert!(matches!(
            group("FetchLatestBlockHeaders"),
            Some(RpcGroup::HeaderFetch)
        ));
        assert!(matches!(
            group("FetchTransactions"),
            Some(RpcGroup::TransactionFetch)
        ));
        assert!(matches!(group("FetchCommits"), Some(RpcGroup::CommitFetch)));
        assert!(matches!(
            group("FetchCommitsAndTransactions"),
            Some(RpcGroup::CommitFetch)
        ));

        assert!(group("GetLatestRounds").is_none());
        assert!(group("Unknown").is_none());
        assert!(RpcGroup::from_path("/other.Service/FetchCommits").is_none());
        assert!(RpcGroup::from_path("FetchCommits").is_none());
    }
}
