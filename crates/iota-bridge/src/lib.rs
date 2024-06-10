// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod abi;
pub mod action_executor;
pub mod client;
pub mod config;
pub mod crypto;
pub mod error;
pub mod eth_client;
pub mod eth_syncer;
pub mod events;
pub mod iota_client;
pub mod iota_syncer;
pub mod iota_transaction_builder;
pub mod node;
pub mod orchestrator;
pub mod server;
pub mod storage;
pub mod types;

#[cfg(test)]
pub(crate) mod eth_mock_provider;

#[cfg(test)]
pub(crate) mod iota_mock_client;

#[cfg(test)]
pub(crate) mod test_utils;

#[macro_export]
macro_rules! retry_with_max_elapsed_time {
    ($func:expr, $max_elapsed_time:expr) => {{
        // The following delay sequence (in secs) will be used, applied with jitter
        // 0.1, 0.2, 0.4, 0.8, 1.6, 3.2, 6.4, 12.8, 25.6, 30, 30, 30 ...
        let backoff = backoff::ExponentialBackoff {
            initial_interval: Duration::from_millis(100),
            randomization_factor: 0.1,
            multiplier: 2.0,
            max_interval: Duration::from_secs(30),
            max_elapsed_time: Some($max_elapsed_time),
            ..Default::default()
        };
        backoff::future::retry(backoff, || {
            let fut = async {
                let result = $func.await;
                match result {
                    Ok(_) => {
                        return Ok(result);
                    }
                    Err(e) => {
                        // For simplicity we treat every error as transient so we can retry until
                        // max_elapsed_time
                        tracing::debug!("Retrying due to error: {:?}", e);
                        return Err(backoff::Error::transient(e));
                    }
                }
            };
            std::boxed::Box::pin(fut)
        })
        .await
    }};
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    async fn example_func_ok() -> anyhow::Result<()> {
        Ok(())
    }

    async fn example_func_err() -> anyhow::Result<()> {
        Err(anyhow::anyhow!(""))
    }

    #[tokio::test]
    async fn test_retry_with_max_elapsed_time() {
        // no retry is needed, should return immediately. We give it a very small
        // max_elapsed_time and it should still finish in time.
        let max_elapsed_time = Duration::from_millis(20);
        retry_with_max_elapsed_time!(example_func_ok(), max_elapsed_time)
            .unwrap()
            .unwrap();

        // now call a function that always errors and expect it to return before
        // max_elapsed_time runs out
        let max_elapsed_time = Duration::from_secs(4);
        let instant = std::time::Instant::now();
        retry_with_max_elapsed_time!(example_func_err(), max_elapsed_time).unwrap_err();
        assert!(instant.elapsed() < max_elapsed_time);
    }
}
