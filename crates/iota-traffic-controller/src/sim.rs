// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Load generator that drives a [`TrafficController`] with synthetic clients.
//! Used by tests to measure how a policy behaves under a given request rate.

use std::{
    net::{IpAddr, Ipv4Addr},
    ops::Add,
    time::{Duration, Instant},
};

use iota_types::traffic_control::{PolicyConfig, Weight};
use rand::RngExt;
use tokio::time;
use tracing::error;

use crate::{TrafficController, policies::TrafficTally};

#[derive(Debug, Clone, Default)]
pub struct TrafficSimMetrics {
    pub num_requests: u64,
    pub num_blocked: u64,
    pub abs_time_to_first_block: Option<Duration>,
    pub num_blocklist_adds: u64,
}

impl Add for TrafficSimMetrics {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            num_requests: self.num_requests + other.num_requests,
            num_blocked: self.num_blocked + other.num_blocked,
            abs_time_to_first_block: match (
                self.abs_time_to_first_block,
                other.abs_time_to_first_block,
            ) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            num_blocklist_adds: self.num_blocklist_adds + other.num_blocklist_adds,
        }
    }
}

pub struct TrafficSim;

impl TrafficSim {
    pub async fn run(
        policy: PolicyConfig,
        num_clients: u8,
        per_client_tps: usize,
        duration: Duration,
    ) -> TrafficSimMetrics {
        assert!(
            per_client_tps <= 10_000,
            "per_client_tps must be less than 10,000. For higher values, increase num_clients"
        );
        assert!(num_clients < 20, "num_clients must be less than 20");
        assert!(num_clients > 0);
        assert!(per_client_tps > 0);
        assert!(duration.as_secs() > 0);

        let controller = TrafficController::init_for_test(policy, None);
        let tasks = (0..num_clients).map(|task_num| {
            tokio::spawn(Self::run_single_client(
                controller.clone(),
                duration,
                task_num,
                per_client_tps,
            ))
        });

        futures::future::join_all(tasks).await.into_iter().fold(
            TrafficSimMetrics::default(),
            |acc, run_client_ret| match run_client_ret {
                Ok(metrics) => acc + metrics,
                Err(err) => {
                    error!("Error running traffic sim client: {:?}", err);
                    acc
                }
            },
        )
    }

    async fn run_single_client(
        controller: TrafficController,
        duration: Duration,
        task_num: u8,
        per_client_tps: usize,
    ) -> TrafficSimMetrics {
        // Do an initial sleep for a random amount of time to smooth
        // out the traffic. This shouldn't be strictly necessary and
        // we can remove if we want more determinism
        let sleep_time = Duration::from_micros(rand::rng().random_range(0..100));
        tokio::time::sleep(sleep_time).await;

        // collectors
        let mut num_requests = 0;
        let mut num_blocked = 0;
        let mut time_to_first_block = None;
        let mut num_blocklist_adds = 0;
        // state variables
        let mut currently_blocked = false;
        let start = Instant::now();

        // we use ticker instead of sleep to be as close to the target TPS as possible,
        let sleep_time = Duration::from_micros(1_000_000 / per_client_tps as u64);
        let mut interval_ticker = time::interval(sleep_time);

        while start.elapsed() < duration {
            let client = Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, task_num)));
            let allowed = controller.check(&client, &None);
            if allowed {
                currently_blocked = false;
                controller.tally(TrafficTally::new(
                    client,
                    // TODO add proxy IP for testing
                    None,
                    // TODO add weight adjustments
                    None,
                    Weight::one(),
                ));
            } else {
                if !currently_blocked {
                    currently_blocked = true;
                    num_blocklist_adds += 1;
                    if time_to_first_block.is_none() {
                        time_to_first_block = Some(start.elapsed());
                    }
                }
                num_blocked += 1;
            }
            num_requests += 1;

            interval_ticker.tick().await;
        }
        TrafficSimMetrics {
            num_requests,
            num_blocked,
            abs_time_to_first_block: time_to_first_block,
            num_blocklist_adds,
        }
    }
}
