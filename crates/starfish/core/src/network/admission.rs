// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-peer, per-RPC admission control for the inbound consensus gRPC server.
//!
//! Each RPC group has an independent concurrency budget per committee peer,
//! keyed on the peer's authenticated authority index. A misbehaving peer can
//! only exhaust its own budget, never another peer's. Commit fetches carry a
//! second budget shared by all peers, since their responses are held in memory
//! until they have been sent; a peer holding no commit fetch is granted one
//! whatever the others hold, so that budget bounds how much a peer can be
//! crowded out, never whether it is served at all. Caps are local, opt-in
//! parameters; a cap of `0` disables the group, leaving the mechanism inert.

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
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tonic::{Status, body::Body};
use tower::{Layer, Service};

use crate::{
    context::Context,
    network::tonic_network::{
        CONSENSUS_SERVICE_PATH_PREFIX, DEPRECATED_METHOD, DEPRECATED_METHOD_MESSAGE, PeerInfo,
    },
};

/// Inbound consensus RPCs grouped by cost and access pattern. Each group has an
/// independent per-peer concurrency budget.
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

    /// The group an inbound request path belongs to, or `None` for a path with
    /// no budget.
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
    /// The group is disabled (cap 0); proceed without holding a permit.
    Unlimited,
    /// A slot was available; hold the permits for the request's (or stream's)
    /// lifetime and drop them to release the slot.
    Permit(AdmissionPermits),
    /// A cap for this group is reached; the request must be rejected.
    Rejected(AdmissionLimit),
}

/// The cap a rejected request ran into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdmissionLimit {
    /// The cap on requests served at once to the peer that sent it.
    Peer,
    /// The cap on requests of the group served at once to all peers together.
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
pub(crate) struct AdmissionPermits {
    /// The peer's own slot, held until the response stream has ended.
    peer: PeerHold,
    /// The node-wide commit-fetch slot, given back once the response has been
    /// sent or its send deadline has passed; `None` for other RPCs.
    total: Option<TotalSlot>,
}

/// A peer's slot in one RPC group, given back when dropped.
pub(crate) enum PeerHold {
    Semaphore { _permit: OwnedSemaphorePermit },
    CommitFetch { _slot: PeerSlot },
}

/// Commit-fetch slots held per peer and node-wide. A peer's first slot is
/// always granted; its further slots only while the node-wide count is below
/// `total_cap`, so `total_cap` is exceeded by at most one slot per peer. A cap
/// of `0` is off.
struct CommitFetchSlots {
    per_peer_cap: u32,
    total_cap: u32,
    held_by_peer: Box<[AtomicU32]>,
    held_total: AtomicU32,
}

impl CommitFetchSlots {
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

    /// Tries to take one slot for `peer`.
    fn try_take(self: &Arc<Self>, peer: usize) -> Admission {
        // An authenticated committee peer's index is always in range; stay
        // defensive rather than panicking on any unexpected index.
        let Some(held_by_peer) = self.held_by_peer.get(peer) else {
            return Admission::Unlimited;
        };
        // The counters guard nothing but themselves, so relaxed ordering is
        // enough.
        let Ok(held_before) =
            held_by_peer.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
                (self.per_peer_cap == 0 || held < self.per_peer_cap).then_some(held + 1)
            })
        else {
            return Admission::Rejected(AdmissionLimit::Peer);
        };
        let taken = self
            .held_total
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
                (held_before == 0 || self.total_cap == 0 || held < self.total_cap)
                    .then_some(held + 1)
            });
        if taken.is_err() {
            held_by_peer.fetch_sub(1, Ordering::Relaxed);
            return Admission::Rejected(AdmissionLimit::AllPeers);
        }
        Admission::Permit(AdmissionPermits {
            peer: PeerHold::CommitFetch {
                _slot: PeerSlot {
                    slots: self.clone(),
                    peer,
                },
            },
            total: Some(TotalSlot {
                slots: self.clone(),
            }),
        })
    }
}

/// A peer's held commit-fetch slot; dropping it gives the slot back.
pub(crate) struct PeerSlot {
    slots: Arc<CommitFetchSlots>,
    peer: usize,
}

impl Drop for PeerSlot {
    fn drop(&mut self) {
        self.slots.held_by_peer[self.peer].fetch_sub(1, Ordering::Relaxed);
    }
}

/// A held node-wide commit-fetch slot; dropping it gives the slot back.
pub(crate) struct TotalSlot {
    slots: Arc<CommitFetchSlots>,
}

impl Drop for TotalSlot {
    fn drop(&mut self) {
        self.slots.held_total.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Per-(peer, RPC group) admission control for the inbound consensus server.
///
/// Each enabled group holds one semaphore per committee peer, sized to that
/// group's per-peer cap; commit fetches hold counters instead, since their
/// per-peer and node-wide caps interact. `None` means the group is disabled.
pub(crate) struct PerPeerAdmission {
    subscribe: Option<Box<[Arc<Semaphore>]>>,
    header: Option<Box<[Arc<Semaphore>]>>,
    transaction: Option<Box<[Arc<Semaphore>]>>,
    commit_fetch: Option<Arc<CommitFetchSlots>>,
}

impl PerPeerAdmission {
    pub(crate) fn new(context: &Context) -> Self {
        let admission = &context.parameters.tonic.admission;
        let size = context.committee.size();
        Self {
            subscribe: Self::row(size, admission.max_subscriptions_per_peer),
            header: Self::row(size, admission.max_header_fetches_per_peer),
            transaction: Self::row(size, admission.max_transaction_fetches_per_peer),
            commit_fetch: CommitFetchSlots::new(
                size,
                admission.max_commit_fetches_per_peer,
                admission.max_commit_fetches_total,
            ),
        }
    }

    /// One semaphore per peer for an enabled group, or `None` when `cap == 0`.
    fn row(size: usize, cap: u32) -> Option<Box<[Arc<Semaphore>]>> {
        (cap > 0).then(|| {
            (0..size)
                .map(|_| Arc::new(Semaphore::new(cap as usize)))
                .collect()
        })
    }

    /// Tries to admit one request from `peer` in `group`.
    pub(crate) fn try_acquire(&self, group: RpcGroup, peer: AuthorityIndex) -> Admission {
        let row = match group {
            RpcGroup::Subscribe => &self.subscribe,
            RpcGroup::HeaderFetch => &self.header,
            RpcGroup::TransactionFetch => &self.transaction,
            RpcGroup::CommitFetch => {
                return match &self.commit_fetch {
                    Some(slots) => slots.try_take(peer.value()),
                    None => Admission::Unlimited,
                };
            }
        };
        // An authenticated committee peer's index is always in range; stay
        // defensive rather than panicking on any unexpected index.
        let Some(semaphore) = row.as_ref().and_then(|row| row.get(peer.value())) else {
            return Admission::Unlimited;
        };
        match semaphore.clone().try_acquire_owned() {
            Ok(permit) => Admission::Permit(AdmissionPermits {
                peer: PeerHold::Semaphore { _permit: permit },
                total: None,
            }),
            Err(_) => Admission::Rejected(AdmissionLimit::Peer),
        }
    }
}

/// RAII guard for an admitted request: holds the peer's slot and keeps the
/// per-group in-use gauge incremented for the request's (or stream's) lifetime.
/// Dropping it releases the slot and decrements the gauge.
pub(crate) struct AdmissionGuard {
    _peer: PeerHold,
    in_use: IntGauge,
}

impl AdmissionGuard {
    pub(crate) fn new(peer: PeerHold, in_use: IntGauge) -> Self {
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

/// Tower layer charging an inbound request to its peer's budget before tonic
/// reads the request body, and holding the permit until the response ends or
/// `send_timeout` passes with the peer not reading it.
#[derive(Clone)]
pub(crate) struct AdmissionLayer {
    context: Arc<Context>,
    admission: Arc<PerPeerAdmission>,
    send_timeout: Duration,
}

impl AdmissionLayer {
    pub(crate) fn new(context: Arc<Context>, send_timeout: Duration) -> Self {
        let admission = Arc::new(PerPeerAdmission::new(&context));
        Self {
            context,
            admission,
            send_timeout,
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
        }
    }
}

/// Answers an over-budget peer with `ResourceExhausted` and passes every other
/// request on with its permits attached to the response body, which gives up
/// the response and the node-wide slot when the peer stops reading it.
#[derive(Clone)]
pub(crate) struct AdmissionService<S> {
    inner: S,
    context: Arc<Context>,
    admission: Arc<PerPeerAdmission>,
    send_timeout: Duration,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for AdmissionService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response<ReclaimableBody<Body, TotalSlot, AdmissionGuard>>;
    type Error = S::Error;
    type Future = AdmissionFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let path = request.uri().path();
        // The deprecated method decodes its request before its handler answers,
        // so answering here is what keeps its body from being read.
        if path.strip_prefix(CONSENSUS_SERVICE_PATH_PREFIX) == Some(DEPRECATED_METHOD) {
            return AdmissionFuture::rejected(Status::unimplemented(DEPRECATED_METHOD_MESSAGE));
        }
        let Some(group) = RpcGroup::from_path(path) else {
            return AdmissionFuture::admitted(self.inner.call(request), None, None, None);
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
        match self.admission.try_acquire(group, peer) {
            Admission::Unlimited => {
                AdmissionFuture::admitted(self.inner.call(request), None, None, send_deadline)
            }
            Admission::Permit(AdmissionPermits { peer, total }) => {
                let in_use = metrics
                    .admission_in_use
                    .with_label_values(&[group.as_str()]);
                let guard = AdmissionGuard::new(peer, in_use);
                AdmissionFuture::admitted(
                    self.inner.call(request),
                    Some(guard),
                    total,
                    send_deadline,
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
            guard: Option<AdmissionGuard>,
            total: Option<TotalSlot>,
            // The send deadline and the counter of responses it reclaims;
            // `None` for a response that may stay open.
            send_deadline: Option<(Duration, IntCounter)>,
        },
        Rejected {
            status: Status,
        },
    }
}

impl<F> AdmissionFuture<F> {
    fn admitted(
        inner: F,
        guard: Option<AdmissionGuard>,
        total: Option<TotalSlot>,
        send_deadline: Option<(Duration, IntCounter)>,
    ) -> Self {
        Self::Admitted {
            inner,
            guard,
            total,
            send_deadline,
        }
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
    type Output = Result<Response<ReclaimableBody<Body, TotalSlot, AdmissionGuard>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match self.project() {
            AdmissionFutureProj::Admitted {
                inner,
                guard,
                total,
                send_deadline,
            } => Poll::Ready(ready!(inner.poll(cx)).map(|response| {
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
            })),
            AdmissionFutureProj::Rejected { status } => Poll::Ready(Ok(status
                .clone()
                .into_http()
                .map(|body| ReclaimableBody::new(body, None, None)))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(i: u8) -> AuthorityIndex {
        AuthorityIndex::from(i)
    }

    fn expect_permit(outcome: Admission) -> AdmissionPermits {
        match outcome {
            Admission::Permit(permits) => permits,
            _ => panic!("expected a permit"),
        }
    }

    #[tokio::test]
    async fn disabled_group_is_unlimited() {
        let admission = PerPeerAdmission {
            subscribe: PerPeerAdmission::row(4, 0),
            header: PerPeerAdmission::row(4, 0),
            transaction: PerPeerAdmission::row(4, 0),
            commit_fetch: CommitFetchSlots::new(4, 0, 0),
        };
        for _ in 0..1000 {
            assert!(matches!(
                admission.try_acquire(RpcGroup::HeaderFetch, peer(0)),
                Admission::Unlimited
            ));
            assert!(matches!(
                admission.try_acquire(RpcGroup::CommitFetch, peer(0)),
                Admission::Unlimited
            ));
        }
    }

    #[tokio::test]
    async fn enforces_cap_and_releases_on_drop() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: PerPeerAdmission::row(4, 2),
            transaction: None,
            commit_fetch: None,
        };
        let p0 = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        // A third concurrent request from the same peer exceeds the cap.
        assert!(matches!(
            admission.try_acquire(RpcGroup::HeaderFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::Peer)
        ));
        // Releasing one permit frees exactly one slot.
        drop(p0);
        let p2 = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        drop((p1, p2));
    }

    #[tokio::test]
    async fn peers_are_isolated() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: PerPeerAdmission::row(4, 1),
            transaction: None,
            commit_fetch: None,
        };
        let held = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(0)));
        // Peer 0 is saturated...
        assert!(matches!(
            admission.try_acquire(RpcGroup::HeaderFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::Peer)
        ));
        // ...but peer 1 has its own independent budget.
        let _other = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        drop(held);
    }

    #[tokio::test]
    async fn commit_fetches_share_a_budget_across_peers() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: CommitFetchSlots::new(4, 2, 3),
        };
        let p0 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p2 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(1)));
        // Peer 1 is within its own cap, but the shared budget is spent.
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // A rejection on the shared budget leaves the peer's own slot free.
        drop(p0);
        let p3 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(1)));
        drop((p1, p2, p3));
    }

    #[tokio::test]
    async fn all_peers_commit_limit_applies_without_a_per_peer_limit() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: CommitFetchSlots::new(4, 0, 2),
        };
        let p0 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop(p0);
        let p2 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        drop((p1, p2));
    }

    #[tokio::test]
    async fn a_peer_holding_no_commit_fetch_is_granted_one_past_the_shared_cap() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: CommitFetchSlots::new(4, 8, 2),
        };
        // Peer 0 spends the whole shared budget.
        let p0 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // Every other peer still gets its first slot, and only that one.
        let p2 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(1)));
        let p3 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(2)));
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // A released first slot is granted again.
        drop(p2);
        let p4 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(1)));
        drop((p0, p1, p3, p4));
    }

    #[tokio::test]
    async fn an_extra_commit_fetch_is_not_regranted_while_the_total_is_at_the_cap() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: CommitFetchSlots::new(4, 8, 2),
        };
        let p0 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p2 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(1)));
        // Peer 0 gives one of its two back, but peer 1's first slot keeps the
        // total at the cap, so peer 0 cannot take a second one again.
        drop(p1);
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop(p2);
        let p3 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        drop((p0, p3));
    }

    #[tokio::test]
    async fn giving_back_the_node_wide_slot_alone_frees_the_shared_budget() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: None,
            transaction: None,
            commit_fetch: CommitFetchSlots::new(4, 8, 1),
        };
        let mut p0 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        // The send deadline gives back the node-wide slot and keeps the peer's
        // own, so the peer may fetch again but stays charged for both.
        drop(p0.total.take());
        let p1 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let held_by_peer = &admission.commit_fetch.as_ref().unwrap().held_by_peer;
        assert_eq!(held_by_peer[0].load(Ordering::Relaxed), 2);
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(0)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop((p0, p1));
        assert_eq!(held_by_peer[0].load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn permit_guarded_body_holds_until_dropped() {
        let admission = PerPeerAdmission {
            subscribe: PerPeerAdmission::row(4, 1),
            header: None,
            transaction: None,
            commit_fetch: None,
        };
        let gauge = IntGauge::new("test_subscribe_in_use", "test").unwrap();
        let permit = expect_permit(admission.try_acquire(RpcGroup::Subscribe, peer(2)));
        let guarded = ReclaimableBody::new(
            Body::default(),
            None::<TotalSlot>,
            Some(AdmissionGuard::new(permit.peer, gauge.clone())),
        );
        // While the response body lives, the peer's single subscribe slot is
        // taken and the in-use gauge reflects it.
        assert_eq!(gauge.get(), 1);
        assert!(matches!(
            admission.try_acquire(RpcGroup::Subscribe, peer(2)),
            Admission::Rejected(AdmissionLimit::Peer)
        ));
        // Dropping the body releases the permit and decrements the gauge.
        drop(guarded);
        assert_eq!(gauge.get(), 0);
        assert!(matches!(
            admission.try_acquire(RpcGroup::Subscribe, peer(2)),
            Admission::Permit(_)
        ));
    }

    #[tokio::test]
    async fn in_use_gauge_tracks_held_guards() {
        let admission = PerPeerAdmission {
            subscribe: None,
            header: PerPeerAdmission::row(4, 2),
            transaction: None,
            commit_fetch: None,
        };
        let gauge = IntGauge::new("test_header_in_use", "test").unwrap();
        let g0 = AdmissionGuard::new(
            expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(0))).peer,
            gauge.clone(),
        );
        let g1 = AdmissionGuard::new(
            expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1))).peer,
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
