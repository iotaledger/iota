// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Limits how many `get_tx_status` queries a validator serves at once, for one
//! client and in total.

use std::{
    collections::{HashMap, hash_map::Entry},
    net::IpAddr,
    num::NonZeroUsize,
    sync::Arc,
};

use iota_common::debug_fatal;
use iota_config::node::AuthorityOverloadConfig;
use parking_lot::Mutex;
use prometheus_filtered::IntGauge;

use crate::authority_server::ValidatorServiceMetrics;

/// The number of queries of one client served at once when
/// [`AuthorityOverloadConfig::max_pending_tx_status_queries_per_client`] is
/// unset.
const DEFAULT_MAX_PENDING_TX_STATUS_QUERIES_PER_CLIENT: usize = 2_000;

/// The number of queries served at once when
/// [`AuthorityOverloadConfig::max_pending_tx_status_queries`] is unset.
const DEFAULT_MAX_PENDING_TX_STATUS_QUERIES: usize = 64_000;

/// The limit that a `get_tx_status` request would exceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExceededTxStatusQueryLimit {
    PerClient,
    Total,
}

impl ExceededTxStatusQueryLimit {
    /// The metric label value that names this limit.
    pub(crate) fn as_label(self) -> &'static str {
        match self {
            Self::PerClient => "per_client",
            Self::Total => "total",
        }
    }
}

/// One client, as [`iota_http::client_prefix`] identifies it.
type ClientKey = Arc<[u8]>;

#[derive(Default)]
struct PendingQueries {
    total: usize,
    /// Holds a client only while it has queries, so it does not grow with the
    /// clients seen.
    per_client: HashMap<ClientKey, usize>,
}

/// The `get_tx_status` queries a validator may serve at once, for one client
/// and in total.
pub(crate) struct TxStatusQueryLimiter {
    max_per_client: usize,
    max_total: usize,
    ipv6_prefix_len: u8,
    pending: Mutex<PendingQueries>,
    /// Set only while `pending` is locked, so it follows the total in order.
    pending_gauge: IntGauge,
}

/// Allows one `get_tx_status` query. Dropping it frees the place for the next
/// one.
pub(crate) struct TxStatusQueryPermit {
    limiter: Arc<TxStatusQueryLimiter>,
    client: Option<ClientKey>,
}

impl Drop for TxStatusQueryPermit {
    fn drop(&mut self) {
        let mut pending = self.limiter.pending.lock();
        match pending.total.checked_sub(1) {
            Some(total) => pending.total = total,
            None => debug_fatal!("released a transaction status query that was not counted"),
        }
        self.limiter.set_pending_gauge(pending.total);

        let Some(client) = self.client.take() else {
            return;
        };
        match pending.per_client.entry(client) {
            Entry::Occupied(mut queries) if *queries.get() > 1 => *queries.get_mut() -= 1,
            Entry::Occupied(queries) => {
                queries.remove();
            }
            Entry::Vacant(_) => {
                debug_fatal!("released a transaction status query of a client that has none")
            }
        }
    }
}

impl TxStatusQueryLimiter {
    /// Returns a limiter sized by
    /// [`AuthorityOverloadConfig::max_pending_tx_status_queries_per_client`]
    /// and [`AuthorityOverloadConfig::max_pending_tx_status_queries`] of
    /// `config`, with the defaults their docs give. IPv6 clients that share
    /// the first `ipv6_prefix_len` bits of their address count as one client.
    /// The limiter reports the queries it counts in
    /// [`ValidatorServiceMetrics::pending_tx_status_queries`].
    pub(crate) fn new(
        config: &AuthorityOverloadConfig,
        ipv6_prefix_len: u8,
        metrics: &ValidatorServiceMetrics,
    ) -> Self {
        Self {
            pending_gauge: metrics.pending_tx_status_queries.clone(),
            max_per_client: config.max_pending_tx_status_queries_per_client.map_or(
                DEFAULT_MAX_PENDING_TX_STATUS_QUERIES_PER_CLIENT,
                NonZeroUsize::get,
            ),
            max_total: config
                .max_pending_tx_status_queries
                .map_or(DEFAULT_MAX_PENDING_TX_STATUS_QUERIES, NonZeroUsize::get),
            ipv6_prefix_len,
            pending: Mutex::default(),
        }
    }

    /// Returns one permit for each of the `count` queries of a request from
    /// `client`, or none at all. A request without a client address counts
    /// toward the total only.
    ///
    /// # Errors
    ///
    /// The limit that `count` more queries would exceed.
    pub(crate) fn try_acquire(
        self: &Arc<Self>,
        client: Option<IpAddr>,
        count: usize,
    ) -> Result<Vec<TxStatusQueryPermit>, ExceededTxStatusQueryLimit> {
        let client: Option<ClientKey> =
            client.map(|ip| iota_http::client_prefix(ip, self.ipv6_prefix_len).into());

        {
            let mut pending = self.pending.lock();
            if let Some(client) = &client {
                let of_client = pending.per_client.get(client).copied().unwrap_or(0);
                if of_client.saturating_add(count) > self.max_per_client {
                    return Err(ExceededTxStatusQueryLimit::PerClient);
                }
            }
            if pending.total.saturating_add(count) > self.max_total {
                return Err(ExceededTxStatusQueryLimit::Total);
            }
            if count == 0 {
                return Ok(Vec::new());
            }
            pending.total += count;
            if let Some(client) = &client {
                *pending.per_client.entry(client.clone()).or_default() += count;
            }
            self.set_pending_gauge(pending.total);
        }

        Ok((0..count)
            .map(|_| TxStatusQueryPermit {
                limiter: self.clone(),
                client: client.clone(),
            })
            .collect())
    }

    fn set_pending_gauge(&self, total: usize) {
        self.pending_gauge
            .set(i64::try_from(total).unwrap_or(i64::MAX));
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    const OTHER_CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    fn limiter(max_per_client: usize, max_total: usize) -> Arc<TxStatusQueryLimiter> {
        let config = AuthorityOverloadConfig {
            max_pending_tx_status_queries_per_client: NonZeroUsize::new(max_per_client),
            max_pending_tx_status_queries: NonZeroUsize::new(max_total),
            ..Default::default()
        };
        Arc::new(TxStatusQueryLimiter::new(
            &config,
            iota_http::DEFAULT_IPV6_PREFIX_LEN,
            &ValidatorServiceMetrics::new_for_tests(),
        ))
    }

    fn v6(last_segment: u16) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(0x2a00, 0x1, 0x2, 0x3, 0, 0, 0, last_segment))
    }

    #[test]
    fn a_client_at_its_limit_is_refused_while_another_is_served() {
        let limiter = limiter(3, 100);

        let _held = limiter.try_acquire(Some(CLIENT), 3).unwrap();
        assert_eq!(
            limiter.try_acquire(Some(CLIENT), 1).err(),
            Some(ExceededTxStatusQueryLimit::PerClient)
        );
        assert_eq!(limiter.try_acquire(Some(OTHER_CLIENT), 3).unwrap().len(), 3);
    }

    #[test]
    fn the_total_limit_holds_across_clients() {
        let limiter = limiter(3, 4);

        let _held = limiter.try_acquire(Some(CLIENT), 3).unwrap();
        assert_eq!(
            limiter.try_acquire(Some(OTHER_CLIENT), 2).err(),
            Some(ExceededTxStatusQueryLimit::Total)
        );
        assert_eq!(limiter.try_acquire(Some(OTHER_CLIENT), 1).unwrap().len(), 1);
    }

    #[test]
    fn a_refused_request_takes_no_place() {
        let limiter = limiter(3, 100);

        let _held = limiter.try_acquire(Some(CLIENT), 2).unwrap();
        assert!(limiter.try_acquire(Some(CLIENT), 2).is_err());
        assert_eq!(limiter.try_acquire(Some(CLIENT), 1).unwrap().len(), 1);
    }

    #[test]
    fn dropping_a_permit_frees_its_place() {
        let limiter = limiter(2, 2);

        let mut held = limiter.try_acquire(Some(CLIENT), 2).unwrap();
        assert!(limiter.try_acquire(Some(CLIENT), 1).is_err());

        held.pop();
        assert_eq!(limiter.try_acquire(Some(CLIENT), 1).unwrap().len(), 1);
    }

    #[test]
    fn the_gauge_follows_the_queries_counted() {
        let limiter = limiter(3, 100);

        let mut held = limiter.try_acquire(Some(CLIENT), 3).unwrap();
        assert_eq!(limiter.pending_gauge.get(), 3);

        assert!(limiter.try_acquire(Some(CLIENT), 1).is_err());
        assert_eq!(limiter.pending_gauge.get(), 3);

        held.pop();
        assert_eq!(limiter.pending_gauge.get(), 2);

        drop(held);
        assert_eq!(limiter.pending_gauge.get(), 0);
    }

    #[test]
    fn a_client_without_queries_is_forgotten() {
        let limiter = limiter(2, 100);

        drop(limiter.try_acquire(Some(CLIENT), 2).unwrap());

        let pending = limiter.pending.lock();
        assert_eq!(pending.total, 0);
        assert!(pending.per_client.is_empty());
    }

    #[test]
    fn ipv6_addresses_of_one_prefix_are_one_client() {
        let limiter = limiter(2, 100);

        let _held = limiter.try_acquire(Some(v6(1)), 2).unwrap();
        assert_eq!(
            limiter.try_acquire(Some(v6(2)), 1).err(),
            Some(ExceededTxStatusQueryLimit::PerClient)
        );
    }

    #[test]
    fn a_request_without_a_client_address_counts_toward_the_total_only() {
        let limiter = limiter(1, 3);

        let _held = limiter.try_acquire(None, 3).unwrap();
        assert_eq!(
            limiter.try_acquire(None, 1).err(),
            Some(ExceededTxStatusQueryLimit::Total)
        );
    }

    #[test]
    fn a_request_without_queries_is_served_at_the_limit() {
        let limiter = limiter(1, 1);

        let _held = limiter.try_acquire(Some(CLIENT), 1).unwrap();
        assert!(limiter.try_acquire(Some(CLIENT), 0).unwrap().is_empty());
    }
}
