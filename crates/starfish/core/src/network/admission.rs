// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-peer, per-RPC admission control for the inbound consensus gRPC server.
//!
//! Each RPC group has an independent concurrency budget per committee peer,
//! keyed on the peer's authenticated authority index. A misbehaving peer can
//! only exhaust its own budget, never another peer's. Commit fetches carry a
//! second budget shared by all peers, since their responses are held in memory
//! until they have been sent. Caps are local, opt-in parameters; a cap of `0`
//! disables the group, leaving the mechanism inert.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll, ready},
};

use bytes::Bytes;
use http::{Request, Response};
use http_body::Body as HttpBody;
use iota_network_stack::concurrency::PermitGuardedBody;
use pin_project_lite::pin_project;
use prometheus_filtered::IntGauge;
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

/// Held while this node serves one inbound request, until its response is sent.
pub(crate) struct AdmissionPermits {
    /// Counts against the limit on requests served at once to the peer that
    /// sent it; `None` when that limit is off.
    _peer: Option<OwnedSemaphorePermit>,
    /// Counts against the limit on commit fetches served at once to all peers
    /// together; `None` for other RPCs or when that limit is off.
    _all_peers: Option<OwnedSemaphorePermit>,
}

/// Per-(peer, RPC group) admission control for the inbound consensus server.
///
/// Each enabled group holds one semaphore per committee peer, sized to that
/// group's per-peer cap. A `None` row means the group is disabled.
pub(crate) struct PerPeerAdmission {
    subscribe: Option<Box<[Arc<Semaphore>]>>,
    header: Option<Box<[Arc<Semaphore>]>>,
    transaction: Option<Box<[Arc<Semaphore>]>>,
    commit: Option<Box<[Arc<Semaphore>]>>,
    /// One budget for commit fetches from every peer together, checked on top
    /// of the peer's own row.
    commit_from_all_peers: Option<Arc<Semaphore>>,
}

impl PerPeerAdmission {
    pub(crate) fn new(context: &Context) -> Self {
        let admission = &context.parameters.tonic.admission;
        let size = context.committee.size();
        Self {
            subscribe: Self::row(size, admission.max_subscriptions_per_peer),
            header: Self::row(size, admission.max_header_fetches_per_peer),
            transaction: Self::row(size, admission.max_transaction_fetches_per_peer),
            commit: Self::row(size, admission.max_commit_fetches_per_peer),
            commit_from_all_peers: (admission.max_commit_fetches_total > 0)
                .then(|| Arc::new(Semaphore::new(admission.max_commit_fetches_total as usize))),
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

    fn group(&self, group: RpcGroup) -> &Option<Box<[Arc<Semaphore>]>> {
        match group {
            RpcGroup::Subscribe => &self.subscribe,
            RpcGroup::HeaderFetch => &self.header,
            RpcGroup::TransactionFetch => &self.transaction,
            RpcGroup::CommitFetch => &self.commit,
        }
    }

    /// The budget every peer draws on together in `group`, where the group has
    /// one.
    fn all_peers(&self, group: RpcGroup) -> &Option<Arc<Semaphore>> {
        match group {
            RpcGroup::CommitFetch => &self.commit_from_all_peers,
            _ => &None,
        }
    }

    /// Tries to admit one request from `peer` in `group`.
    pub(crate) fn try_acquire(&self, group: RpcGroup, peer: AuthorityIndex) -> Admission {
        // An authenticated committee peer's index is always in range; stay
        // defensive rather than panicking on any unexpected index.
        let peer_semaphore = self
            .group(group)
            .as_ref()
            .and_then(|row| row.get(peer.value()));
        let all_peers_semaphore = self.all_peers(group).as_ref();
        if peer_semaphore.is_none() && all_peers_semaphore.is_none() {
            return Admission::Unlimited;
        }
        let Ok(peer_permit) = peer_semaphore
            .map(|semaphore| semaphore.clone().try_acquire_owned())
            .transpose()
        else {
            return Admission::Rejected(AdmissionLimit::Peer);
        };
        let Ok(all_peers_permit) = all_peers_semaphore
            .map(|semaphore| semaphore.clone().try_acquire_owned())
            .transpose()
        else {
            return Admission::Rejected(AdmissionLimit::AllPeers);
        };
        Admission::Permit(AdmissionPermits {
            _peer: peer_permit,
            _all_peers: all_peers_permit,
        })
    }
}

/// RAII guard for an admitted request: holds the per-peer permit and keeps the
/// per-group in-use gauge incremented for the request's (or stream's) lifetime.
/// Dropping it releases the slot and decrements the gauge.
pub(crate) struct AdmissionGuard {
    _permits: AdmissionPermits,
    in_use: IntGauge,
}

impl AdmissionGuard {
    pub(crate) fn new(permits: AdmissionPermits, in_use: IntGauge) -> Self {
        in_use.inc();
        Self {
            _permits: permits,
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
/// reads the request body, and holding the permit until the response ends.
#[derive(Clone)]
pub(crate) struct AdmissionLayer {
    context: Arc<Context>,
    admission: Arc<PerPeerAdmission>,
}

impl AdmissionLayer {
    pub(crate) fn new(context: Arc<Context>) -> Self {
        let admission = Arc::new(PerPeerAdmission::new(&context));
        Self { context, admission }
    }
}

impl<S> Layer<S> for AdmissionLayer {
    type Service = AdmissionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AdmissionService {
            inner,
            context: self.context.clone(),
            admission: self.admission.clone(),
        }
    }
}

/// Answers an over-budget peer with `ResourceExhausted` and passes every other
/// request on with its permit attached to the response body.
#[derive(Clone)]
pub(crate) struct AdmissionService<S> {
    inner: S,
    context: Arc<Context>,
    admission: Arc<PerPeerAdmission>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for AdmissionService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response<PermitGuardedBody<Body, AdmissionGuard>>;
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
            return AdmissionFuture::admitted(self.inner.call(request), None);
        };
        let Some(peer) = request
            .extensions()
            .get::<PeerInfo>()
            .map(|peer| peer.authority_index)
        else {
            return AdmissionFuture::rejected(Status::internal("PeerInfo not found"));
        };
        match self.admission.try_acquire(group, peer) {
            Admission::Unlimited => AdmissionFuture::admitted(self.inner.call(request), None),
            Admission::Permit(permit) => {
                let in_use = self
                    .context
                    .metrics
                    .network_metrics
                    .admission_in_use
                    .with_label_values(&[group.as_str()]);
                let guard = AdmissionGuard::new(permit, in_use);
                AdmissionFuture::admitted(self.inner.call(request), Some(guard))
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
        },
        Rejected {
            status: Status,
        },
    }
}

impl<F> AdmissionFuture<F> {
    fn admitted(inner: F, guard: Option<AdmissionGuard>) -> Self {
        Self::Admitted { inner, guard }
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
    type Output = Result<Response<PermitGuardedBody<Body, AdmissionGuard>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match self.project() {
            AdmissionFutureProj::Admitted { inner, guard } => {
                Poll::Ready(ready!(inner.poll(cx)).map(|response| {
                    response.map(|body| PermitGuardedBody::new(Body::new(body), guard.take()))
                }))
            }
            AdmissionFutureProj::Rejected { status } => Poll::Ready(Ok(status
                .clone()
                .into_http()
                .map(|body| PermitGuardedBody::new(body, None)))),
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
            commit: PerPeerAdmission::row(4, 0),
            commit_from_all_peers: None,
        };
        for _ in 0..1000 {
            assert!(matches!(
                admission.try_acquire(RpcGroup::HeaderFetch, peer(0)),
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
            commit: None,
            commit_from_all_peers: None,
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
            commit: None,
            commit_from_all_peers: None,
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
            commit: PerPeerAdmission::row(4, 2),
            commit_from_all_peers: Some(Arc::new(Semaphore::new(3))),
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
            commit: None,
            commit_from_all_peers: Some(Arc::new(Semaphore::new(2))),
        };
        let p0 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(0)));
        assert!(matches!(
            admission.try_acquire(RpcGroup::CommitFetch, peer(1)),
            Admission::Rejected(AdmissionLimit::AllPeers)
        ));
        drop(p0);
        let p2 = expect_permit(admission.try_acquire(RpcGroup::CommitFetch, peer(1)));
        drop((p1, p2));
    }

    #[tokio::test]
    async fn permit_guarded_body_holds_until_dropped() {
        let admission = PerPeerAdmission {
            subscribe: PerPeerAdmission::row(4, 1),
            header: None,
            transaction: None,
            commit: None,
            commit_from_all_peers: None,
        };
        let gauge = IntGauge::new("test_subscribe_in_use", "test").unwrap();
        let permit = expect_permit(admission.try_acquire(RpcGroup::Subscribe, peer(2)));
        let guarded = PermitGuardedBody::new(
            Body::default(),
            Some(AdmissionGuard::new(permit, gauge.clone())),
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
            commit: None,
            commit_from_all_peers: None,
        };
        let gauge = IntGauge::new("test_header_in_use", "test").unwrap();
        let g0 = AdmissionGuard::new(
            expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(0))),
            gauge.clone(),
        );
        let g1 = AdmissionGuard::new(
            expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1))),
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
