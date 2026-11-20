// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The gas limit of simulations. Each client has a gas balance that refills by
//! `gas-per-sec` up to `burst-gas`. A simulation reserves the computation cap
//! of one transaction when it starts and gives back what it did not use when
//! it ends, so simulations a client runs at the same time cannot pass the
//! limit together. `governor` cannot do this: a GCRA cell can neither give
//! back what it charged, nor tell whether a client is over its limit without
//! charging it.

use std::{
    net::IpAddr,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use arc_swap::ArcSwapOption;
use iota_common::fatal;
use iota_types::traffic_control::SimulationGasLimitConfig;
use lru::LruCache;
use parking_lot::Mutex;

use crate::{metrics::TrafficControllerMetrics, policies::MAX_TRACKED_CLIENTS};

const NANOS_PER_SEC: u128 = 1_000_000_000;

/// The gas balance of a client that has not got all of its `burst-gas`.
struct Balance {
    gas: u64,
    /// When `gas` was last refilled. The time a refill did not turn into
    /// whole gas units is kept, so that frequent refills do not lose it.
    refilled_at: Instant,
}

impl Balance {
    fn refill(&mut self, now: Instant, settings: &SimulationGasLimitConfig) {
        let elapsed = now.saturating_duration_since(self.refilled_at).as_nanos();
        let gas_per_sec = u128::from(settings.gas_per_sec);
        let added = elapsed.saturating_mul(gas_per_sec) / NANOS_PER_SEC;
        let full = settings.burst_gas;
        if added == 0 {
            self.gas = self.gas.min(full);
            return;
        }
        self.gas = u64::try_from(u128::from(self.gas).saturating_add(added))
            .unwrap_or(u64::MAX)
            .min(full);
        if self.gas == full {
            self.refilled_at = now;
        } else {
            // `used` is at most the elapsed time, so it fits.
            let used = added.saturating_mul(NANOS_PER_SEC) / gas_per_sec;
            self.refilled_at += Duration::from_nanos(u64::try_from(used).unwrap_or(u64::MAX));
        }
    }
}

/// Why a simulation was refused, and when the client may try again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationGasRefusal {
    pub retry_after: Duration,
}

impl std::fmt::Display for SimulationGasRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "simulation gas limit reached, retry in {}s",
            self.retry_after.as_secs()
        )
    }
}

/// The gas balances of the clients. Only a reservation adds an entry, so a
/// client that never simulates takes none. A settle that brings the balance
/// back to `burst-gas` removes the entry.
pub(crate) struct SimulationGasLimiter {
    settings: ArcSwapOption<SimulationGasLimitConfig>,
    max_computation: AtomicU64,
    /// Keyed by the client as `client_prefix` gives it. The clients whose IP
    /// the node could not resolve share the `None` entry.
    balances: Mutex<LruCache<Option<IpAddr>, Balance>>,
    metrics: Arc<TrafficControllerMetrics>,
}

impl SimulationGasLimiter {
    /// # Panics
    ///
    /// If a value of `config` is 0.
    pub(crate) fn new(
        config: Option<&SimulationGasLimitConfig>,
        metrics: Arc<TrafficControllerMetrics>,
    ) -> Self {
        if let Some(config) = config {
            validate(Some(config.gas_per_sec), Some(config.burst_gas), true)
                .unwrap_or_else(|err| fatal!("Invalid traffic control config: {err}"));
        }
        set_metrics(config, &metrics);
        Self {
            settings: ArcSwapOption::from_pointee(config.cloned()),
            max_computation: AtomicU64::new(u64::MAX),
            balances: Mutex::new(LruCache::new(
                NonZeroUsize::new(MAX_TRACKED_CLIENTS).expect("capacity is non-zero"),
            )),
            metrics,
        }
    }

    pub(crate) fn settings(&self) -> Option<SimulationGasLimitConfig> {
        self.settings.load().as_deref().cloned()
    }

    pub(crate) fn set_max_computation(&self, gas: u64) {
        self.max_computation.store(gas, Ordering::Relaxed);
    }

    /// Reserves gas for one simulation of `client`, and returns how much, or
    /// `None` when simulations have no gas limit. The reservation is the
    /// computation cap of one transaction, or `burst-gas` if that is lower,
    /// so that a full balance always passes.
    pub(crate) fn reserve(
        &self,
        client: Option<IpAddr>,
        dry_run: bool,
        now: Instant,
    ) -> Option<Result<u64, SimulationGasRefusal>> {
        let settings = self.settings.load_full()?;
        let reservation = self
            .max_computation
            .load(Ordering::Relaxed)
            .min(settings.burst_gas);
        let mut balances = self.balances.lock();
        let balance = balances.get_or_insert_mut(client, || Balance {
            gas: settings.burst_gas,
            refilled_at: now,
        });
        balance.refill(now, &settings);
        if let Some(gas) = balance.gas.checked_sub(reservation) {
            balance.gas = gas;
            return Some(Ok(reservation));
        }
        if dry_run {
            self.metrics.num_dry_run_simulation_gas_refusals.inc();
            return Some(Ok(0));
        }
        self.metrics.simulation_gas_refusals.inc();
        let missing = reservation - balance.gas;
        let retry_after_secs = missing.div_ceil(settings.gas_per_sec).max(1);
        Some(Err(SimulationGasRefusal {
            retry_after: Duration::from_secs(retry_after_secs),
        }))
    }

    /// Charges `client` for a simulation that reserved `reserved` gas and
    /// used `charged`, and gives back the difference.
    pub(crate) fn settle(&self, client: Option<IpAddr>, reserved: u64, charged: u64, now: Instant) {
        self.metrics.simulation_gas_charged.inc_by(charged);
        let Some(settings) = self.settings.load_full() else {
            return;
        };
        let mut balances = self.balances.lock();
        let Some(balance) = balances.get_mut(&client) else {
            // The client got a full balance back meanwhile, or was evicted and
            // starts again with a full balance. Charge what the reservation
            // did not cover, for example all of it for a client that dry run
            // served over the limit.
            if let Some(gas) = charged
                .checked_sub(reserved)
                .filter(|&excess| excess > 0)
                .map(|excess| settings.burst_gas.saturating_sub(excess))
            {
                balances.put(
                    client,
                    Balance {
                        gas,
                        refilled_at: now,
                    },
                );
            }
            return;
        };
        balance.refill(now, &settings);
        balance.gas = balance
            .gas
            .saturating_add(reserved)
            .saturating_sub(charged)
            .min(settings.burst_gas);
        if balance.gas == settings.burst_gas {
            balances.pop(&client);
        }
    }

    /// Drops the balance of `client`, which starts again with a full one.
    pub(crate) fn forget(&self, client: Option<IpAddr>) {
        self.balances.lock().pop(&client);
    }

    /// The `top` lowest balances as of `now`, lowest first, and how many
    /// balances were left out. A full balance is not listed.
    pub(crate) fn lowest_balances(
        &self,
        top: usize,
        now: Instant,
    ) -> (Vec<(Option<IpAddr>, u64)>, usize) {
        let Some(settings) = self.settings.load_full() else {
            return (vec![], 0);
        };
        // Copied out first, so that simulations wait on the lock only for the
        // copy.
        let copied = self
            .balances
            .lock()
            .iter()
            .map(|(client, balance)| (*client, balance.gas, balance.refilled_at))
            .collect::<Vec<_>>();
        let mut balances = copied
            .into_iter()
            .map(|(client, gas, refilled_at)| {
                let mut balance = Balance { gas, refilled_at };
                balance.refill(now, &settings);
                (client, balance.gas)
            })
            .filter(|(_, gas)| *gas < settings.burst_gas)
            .collect::<Vec<_>>();
        balances.sort_unstable_by_key(|(client, gas)| (*gas, *client));
        let omitted = balances.len().saturating_sub(top);
        balances.truncate(top);
        (balances, omitted)
    }

    pub(crate) fn max_computation(&self) -> u64 {
        self.max_computation.load(Ordering::Relaxed)
    }

    /// Rejects the values that [`Self::reconfigure`] must not apply.
    pub(crate) fn validate_reconfigure(
        &self,
        gas_per_sec: Option<u64>,
        burst_gas: Option<u64>,
    ) -> Result<(), String> {
        validate(gas_per_sec, burst_gas, self.settings.load().is_none())
    }

    /// Applies the values that are given. Balances are kept, and cut to a
    /// lower `burst-gas` at their next refill. The values must pass
    /// [`Self::validate_reconfigure`].
    ///
    /// # Panics
    ///
    /// If the limit is off and only one of the values is given.
    pub(crate) fn reconfigure(&self, gas_per_sec: Option<u64>, burst_gas: Option<u64>) {
        if gas_per_sec.is_none() && burst_gas.is_none() {
            return;
        }
        // Held so that two changes at once do not overwrite each other, and the
        // gauges show the last one.
        let _balances = self.balances.lock();
        let current = self.settings();
        let settings = SimulationGasLimitConfig {
            gas_per_sec: gas_per_sec
                .or(current.as_ref().map(|current| current.gas_per_sec))
                .expect("validated: the limit is on, or both values are given"),
            burst_gas: burst_gas
                .or(current.as_ref().map(|current| current.burst_gas))
                .expect("validated: the limit is on, or both values are given"),
        };
        set_metrics(Some(&settings), &self.metrics);
        self.settings.store(Some(Arc::new(settings)));
    }
}

/// Rejects a value of 0, and with `turning_on` a change that does not give
/// both values.
fn validate(
    gas_per_sec: Option<u64>,
    burst_gas: Option<u64>,
    turning_on: bool,
) -> Result<(), String> {
    for (value, name) in [(gas_per_sec, "gas-per-sec"), (burst_gas, "burst-gas")] {
        if value == Some(0) {
            return Err(format!("the simulation gas limit {name} must be above 0"));
        }
    }
    let changes = gas_per_sec.is_some() || burst_gas.is_some();
    if turning_on && changes && (gas_per_sec.is_none() || burst_gas.is_none()) {
        return Err(
            "turning the simulation gas limit on takes both gas-per-sec and burst-gas".to_string(),
        );
    }
    Ok(())
}

fn set_metrics(settings: Option<&SimulationGasLimitConfig>, metrics: &TrafficControllerMetrics) {
    let (gas_per_sec, burst_gas) = settings.map_or((0, 0), |settings| {
        (settings.gas_per_sec, settings.burst_gas)
    });
    metrics
        .simulation_gas_per_sec
        .set(i64::try_from(gas_per_sec).unwrap_or(i64::MAX));
    metrics
        .simulation_burst_gas
        .set(i64::try_from(burst_gas).unwrap_or(i64::MAX));
}

/// The gas units a simulation that ran with `gas_price` is charged for its
/// `computation_cost`: rounded up, and at most `max_computation`. Out of gas,
/// `computation_cost` is the whole budget, which can be well above the
/// computation the simulation did. A price of 0 charges `max_computation`.
pub(crate) fn charged_gas(computation_cost: u64, gas_price: u64, max_computation: u64) -> u64 {
    if gas_price == 0 {
        return max_computation;
    }
    computation_cost.div_ceil(gas_price).min(max_computation)
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const CLIENT: Option<IpAddr> = Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
    const MAX_COMPUTATION: u64 = 100;

    fn limiter(gas_per_sec: u64, burst_gas: u64) -> SimulationGasLimiter {
        let limiter = SimulationGasLimiter::new(
            Some(&SimulationGasLimitConfig {
                gas_per_sec,
                burst_gas,
            }),
            Arc::new(TrafficControllerMetrics::new_for_tests()),
        );
        limiter.set_max_computation(MAX_COMPUTATION);
        limiter
    }

    #[test]
    fn a_client_is_refused_once_its_burst_is_used() {
        let limiter = limiter(10, 250);
        let now = Instant::now();
        assert_eq!(limiter.reserve(CLIENT, false, now), Some(Ok(100)));
        assert_eq!(limiter.reserve(CLIENT, false, now), Some(Ok(100)));
        // 50 left, 50 missing at 10 per second.
        assert_eq!(
            limiter.reserve(CLIENT, false, now),
            Some(Err(SimulationGasRefusal {
                retry_after: Duration::from_secs(5)
            }))
        );
        assert_eq!(limiter.metrics.simulation_gas_refusals.get(), 1);
        // Another client has a balance of its own.
        assert_eq!(
            limiter.reserve(Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))), false, now),
            Some(Ok(100))
        );
    }

    #[test]
    fn the_balance_refills_over_time() {
        let limiter = limiter(10, 100);
        let now = Instant::now();
        assert_eq!(limiter.reserve(CLIENT, false, now), Some(Ok(100)));
        assert!(matches!(
            limiter.reserve(CLIENT, false, now + Duration::from_secs(9)),
            Some(Err(_))
        ));
        assert_eq!(
            limiter.reserve(CLIENT, false, now + Duration::from_secs(10)),
            Some(Ok(100))
        );
    }

    #[test]
    fn frequent_refills_lose_no_gas() {
        let limiter = limiter(1, 100);
        let now = Instant::now();
        assert_eq!(limiter.reserve(CLIENT, false, now), Some(Ok(100)));
        // Each check comes before a whole gas unit refilled.
        for half_secs in 1..200 {
            let _ = limiter.reserve(CLIENT, false, now + Duration::from_millis(500 * half_secs));
        }
        assert_eq!(
            limiter.reserve(CLIENT, false, now + Duration::from_secs(100)),
            Some(Ok(100))
        );
    }

    #[test]
    fn a_cheap_simulation_gets_the_rest_back() {
        let limiter = limiter(1, 100);
        let now = Instant::now();
        let reserved = limiter.reserve(CLIENT, false, now).unwrap().unwrap();
        limiter.settle(CLIENT, reserved, 10, now);
        assert_eq!(limiter.metrics.simulation_gas_charged.get(), 10);
        assert!(matches!(limiter.reserve(CLIENT, false, now), Some(Err(_))));
        assert_eq!(
            limiter.reserve(CLIENT, false, now + Duration::from_secs(10)),
            Some(Ok(100))
        );
    }

    #[test]
    fn reservations_held_at_once_stop_at_the_balance() {
        let limiter = limiter(1, 300);
        let now = Instant::now();
        let passed = (0..10)
            .filter(|_| matches!(limiter.reserve(CLIENT, false, now), Some(Ok(_))))
            .count();
        assert_eq!(passed, 3);
    }

    #[test]
    fn a_full_balance_has_no_entry() {
        let limiter = limiter(1, 300);
        let now = Instant::now();
        let reserved = limiter.reserve(CLIENT, false, now).unwrap().unwrap();
        assert_eq!(limiter.balances.lock().len(), 1);
        limiter.settle(CLIENT, reserved, 0, now);
        assert_eq!(limiter.balances.lock().len(), 0);
    }

    #[test]
    fn unresolved_clients_share_one_balance() {
        let limiter = limiter(1, 100);
        let now = Instant::now();
        assert_eq!(limiter.reserve(None, false, now), Some(Ok(100)));
        assert!(matches!(limiter.reserve(None, false, now), Some(Err(_))));
    }

    #[test]
    fn dry_run_serves_a_client_over_the_limit_and_counts_it() {
        let limiter = limiter(1, 100);
        let now = Instant::now();
        assert_eq!(limiter.reserve(CLIENT, true, now), Some(Ok(100)));
        assert_eq!(limiter.reserve(CLIENT, true, now), Some(Ok(0)));
        assert_eq!(limiter.metrics.num_dry_run_simulation_gas_refusals.get(), 1);
        assert_eq!(limiter.metrics.simulation_gas_refusals.get(), 0);
    }

    /// In dry run a client over the limit reserves nothing, so its whole
    /// charge is taken when it settles, also after the balance was full again.
    #[test]
    fn a_charge_the_reservation_did_not_cover_is_taken() {
        let limiter = limiter(1, 100);
        let now = Instant::now();
        assert_eq!(limiter.reserve(CLIENT, true, now), Some(Ok(100)));
        assert_eq!(limiter.reserve(CLIENT, true, now), Some(Ok(0)));
        limiter.settle(CLIENT, 100, 0, now);
        assert_eq!(limiter.balances.lock().len(), 0);

        limiter.settle(CLIENT, 0, 60, now);
        assert_eq!(
            limiter.balances.lock().peek(&CLIENT).map(|b| b.gas),
            Some(40)
        );
    }

    #[test]
    fn a_burst_below_the_computation_cap_still_serves_a_full_balance() {
        let limiter = limiter(1, 40);
        assert_eq!(limiter.reserve(CLIENT, false, Instant::now()), Some(Ok(40)));
    }

    #[test]
    fn no_limit_reserves_nothing() {
        let limiter =
            SimulationGasLimiter::new(None, Arc::new(TrafficControllerMetrics::new_for_tests()));
        assert_eq!(limiter.reserve(CLIENT, false, Instant::now()), None);
    }

    #[test]
    fn a_lower_burst_cuts_the_balances() {
        let limiter = limiter(1, 1000);
        let now = Instant::now();
        assert_eq!(limiter.reserve(CLIENT, false, now), Some(Ok(100)));
        limiter.reconfigure(None, Some(150));
        assert_eq!(limiter.reserve(CLIENT, false, now), Some(Ok(100)));
        assert!(matches!(limiter.reserve(CLIENT, false, now), Some(Err(_))));
    }

    #[test]
    fn the_limit_turns_on_only_with_both_values() {
        let limiter =
            SimulationGasLimiter::new(None, Arc::new(TrafficControllerMetrics::new_for_tests()));
        assert!(limiter.validate_reconfigure(Some(10), None).is_err());
        assert!(limiter.validate_reconfigure(Some(10), Some(0)).is_err());
        assert!(limiter.validate_reconfigure(None, None).is_ok());
        limiter.validate_reconfigure(Some(10), Some(20)).unwrap();
        limiter.reconfigure(Some(10), Some(20));
        assert_eq!(
            limiter.settings(),
            Some(SimulationGasLimitConfig {
                gas_per_sec: 10,
                burst_gas: 20
            })
        );
        assert_eq!(limiter.metrics.simulation_burst_gas.get(), 20);
        // Once on, one value can change alone.
        assert!(limiter.validate_reconfigure(None, Some(30)).is_ok());
    }

    #[test]
    #[should_panic(expected = "gas-per-sec must be above 0")]
    fn a_zero_rate_stops_the_node() {
        limiter(0, 100);
    }

    #[test]
    fn an_out_of_gas_simulation_is_charged_at_most_the_computation_cap() {
        // Out of gas, the computation cost is the whole budget.
        assert_eq!(charged_gas(50_000_000_000, 1000, 5_000_000), 5_000_000);
        assert_eq!(charged_gas(1001, 1000, 5_000_000), 2);
        assert_eq!(charged_gas(0, 1000, 5_000_000), 0);
    }

    #[test]
    fn a_zero_price_charges_the_computation_cap() {
        assert_eq!(charged_gas(1000, 0, 5_000_000), 5_000_000);
    }
}
