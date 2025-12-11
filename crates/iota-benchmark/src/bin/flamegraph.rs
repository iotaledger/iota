// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_test_transaction_builder::make_transfer_iota_transaction;
use test_cluster::TestClusterBuilder;
use tracing::Instrument as _;

#[cfg(all(feature = "flamegraph-alloc", nightly))]
#[global_allocator]
static GLOBAL: telemetry_subscribers::flamegraph::CounterAlloc<std::alloc::System> =
    telemetry_subscribers::flamegraph::CounterAlloc::new(std::alloc::System);

/// This is a binary to generate test flamegraph data in a simple benchmark with
/// a local test cluster. To run it, use:
/// ```cargo run --release --package iota-benchmark --bin flamegraph```
/// This will output a JSON blob that can be imported into Grafana to visualize the flamegraph.
#[tokio::main]
async fn main() {
    let sub = telemetry_subscribers::FlameSub::new();
    tracing::subscriber::set_global_default(sub.clone()).unwrap();

    async {
        let test_cluster = TestClusterBuilder::new().build().await;
        let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
        test_cluster.execute_transaction(tx).await;
    }
    .instrument(tracing::trace_span!("iota_benchmark::flamegraph"))
    .await;

    // follow instructions in telemetry-subscribers README how to setup grafana to
    // visualize the flamegraphs
    let nested_sets = sub.get_nested_sets("iota-benchmark::flamegraph", true, true);
    println!("{}", serde_json::to_string_pretty(&nested_sets).unwrap());

    use std::io::Write as _;

    #[allow(unused_mut)]
    let mut config = Default::default();

    std::fs::File::create("flamegraph.svg")
        .unwrap()
        .write_all(
            sub.get_combined_svg("iota-benchmark::flamegraph", true, true, &config)
                .unwrap()
                .into_string()
                .as_bytes(),
        )
        .unwrap();
    #[cfg(all(feature = "flamegraph-alloc", nightly))]
    {
        config.measure_mem = true;
        std::fs::File::create("flamegraph-mem.svg")
            .unwrap()
            .write_all(
                sub.get_combined_svg("iota-benchmark::flamegraph", true, true, &config)
                    .unwrap()
                    .into_string()
                    .as_bytes(),
            )
            .unwrap();
    }
}
