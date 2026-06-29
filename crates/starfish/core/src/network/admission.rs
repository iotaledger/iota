// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-peer, per-RPC admission control for the inbound consensus gRPC server.
//!
//! Each RPC group has an independent concurrency budget per committee peer,
//! keyed on the peer's authenticated authority index. A misbehaving peer can
//! only exhaust its own budget, never another peer's. Caps are local, opt-in
//! parameters; a cap of `0` disables the group, leaving the mechanism inert.

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
};

use futures::Stream;
use prometheus_filtered::IntGauge;
use starfish_config::AuthorityIndex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::context::Context;

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
}

/// Outcome of an admission attempt.
pub(crate) enum Admission {
    /// The group is disabled (cap 0); proceed without holding a permit.
    Unlimited,
    /// A slot was available; hold the permit for the request's (or stream's)
    /// lifetime and drop it to release the slot.
    Permit(OwnedSemaphorePermit),
    /// The peer is at its cap for this group; the request must be rejected.
    Rejected,
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

    /// Tries to admit one request from `peer` in `group`.
    pub(crate) fn try_acquire(&self, group: RpcGroup, peer: AuthorityIndex) -> Admission {
        let Some(row) = self.group(group) else {
            return Admission::Unlimited;
        };
        // An authenticated committee peer's index is always in range; stay
        // defensive rather than panicking on any unexpected index.
        let Some(semaphore) = row.get(peer.value()) else {
            return Admission::Unlimited;
        };
        match semaphore.clone().try_acquire_owned() {
            Ok(permit) => Admission::Permit(permit),
            Err(_) => Admission::Rejected,
        }
    }
}

/// RAII guard for an admitted request: holds the per-peer permit and keeps the
/// per-group in-use gauge incremented for the request's (or stream's) lifetime.
/// Dropping it releases the slot and decrements the gauge.
pub(crate) struct AdmissionGuard {
    _permit: OwnedSemaphorePermit,
    in_use: IntGauge,
}

impl AdmissionGuard {
    pub(crate) fn new(permit: OwnedSemaphorePermit, in_use: IntGauge) -> Self {
        in_use.inc();
        Self {
            _permit: permit,
            in_use,
        }
    }
}

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        self.in_use.dec();
    }
}

/// Wraps a response stream so it owns an admission guard for the stream's
/// entire lifetime; the guard is released when the stream is dropped (client
/// disconnect, server shutdown, or stream end).
pub(crate) struct PermitGuardedStream<St> {
    inner: St,
    _guard: Option<AdmissionGuard>,
}

impl<St> PermitGuardedStream<St> {
    pub(crate) fn new(inner: St, guard: Option<AdmissionGuard>) -> Self {
        Self {
            inner,
            _guard: guard,
        }
    }
}

impl<St: Stream + Unpin> Stream for PermitGuardedStream<St> {
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().inner).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(i: u8) -> AuthorityIndex {
        AuthorityIndex::from(i)
    }

    fn expect_permit(outcome: Admission) -> OwnedSemaphorePermit {
        match outcome {
            Admission::Permit(permit) => permit,
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
        };
        let p0 = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        let p1 = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        // A third concurrent request from the same peer exceeds the cap.
        assert!(matches!(
            admission.try_acquire(RpcGroup::HeaderFetch, peer(1)),
            Admission::Rejected
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
        };
        let held = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(0)));
        // Peer 0 is saturated...
        assert!(matches!(
            admission.try_acquire(RpcGroup::HeaderFetch, peer(0)),
            Admission::Rejected
        ));
        // ...but peer 1 has its own independent budget.
        let _other = expect_permit(admission.try_acquire(RpcGroup::HeaderFetch, peer(1)));
        drop(held);
    }

    #[tokio::test]
    async fn permit_guarded_stream_holds_until_dropped() {
        let admission = PerPeerAdmission {
            subscribe: PerPeerAdmission::row(4, 1),
            header: None,
            transaction: None,
            commit: None,
        };
        let gauge = IntGauge::new("test_subscribe_in_use", "test").unwrap();
        let permit = expect_permit(admission.try_acquire(RpcGroup::Subscribe, peer(2)));
        let guarded = PermitGuardedStream::new(
            futures::stream::empty::<i32>(),
            Some(AdmissionGuard::new(permit, gauge.clone())),
        );
        // While the stream lives, the peer's single subscribe slot is taken and
        // the in-use gauge reflects it.
        assert_eq!(gauge.get(), 1);
        assert!(matches!(
            admission.try_acquire(RpcGroup::Subscribe, peer(2)),
            Admission::Rejected
        ));
        // Dropping the stream releases the permit and decrements the gauge.
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
}
