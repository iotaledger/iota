// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use iota_macros::sim_test;
use iota_single_node_benchmark::{
    command::{Component, WorkloadKind},
    run_benchmark,
    workload::Workload,
};
use strum::IntoEnumIterator;

#[sim_test]
async fn benchmark_non_move_transactions_smoke_test() {
    for skip_signing in [true, false] {
        for component in Component::iter() {
            run_benchmark(
                Workload::new(
                    10,
                    WorkloadKind::PTB {
                        num_transfers: 2,
                        use_native_transfer: true,
                        num_dynamic_fields: 0,
                        computation: 0,
                    },
                ),
                component,
                1000,
                false,
                skip_signing,
            )
            .await;
        }
    }
}

#[sim_test]
async fn benchmark_move_transactions_smoke_test() {
    for skip_signing in [true, false] {
        for component in Component::iter() {
            run_benchmark(
                Workload::new(
                    10,
                    WorkloadKind::PTB {
                        num_transfers: 2,
                        use_native_transfer: false,
                        num_dynamic_fields: 1,
                        computation: 1,
                    },
                ),
                component,
                1000,
                false,
                skip_signing,
            )
            .await;
        }
    }
}

#[sim_test]
async fn benchmark_publish_from_source() {
    // This test makes sure that the benchmark runs.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend([
        "tests",
        "data",
        "package_publish_from_source",
        "manifest.json",
    ]);
    for component in Component::iter() {
        run_benchmark(
            Workload::new(
                10,
                WorkloadKind::Publish {
                    manifest_file: path.clone(),
                },
            ),
            component,
            1000,
            false,
            false,
        )
        .await;
    }
}

#[sim_test]
async fn benchmark_publish_from_bytecode() {
    // This test makes sure that the benchmark runs.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend([
        "tests",
        "data",
        "package_publish_from_bytecode",
        "manifest.json",
    ]);
    for component in Component::iter() {
        run_benchmark(
            Workload::new(
                10,
                WorkloadKind::Publish {
                    manifest_file: path.clone(),
                },
            ),
            component,
            1000,
            false,
            false,
        )
        .await;
    }
}
