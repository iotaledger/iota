// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pins the two measurement-window invariants the calibration data depends
//! on. Both are easy to break silently when the shared benchmark substrate
//! evolves:
//!
//! 1. the measured window opens **before** input-object loading — input reads
//!    run synchronously on the worker thread, so they are lane work; a window
//!    that opens after them fits the input-read coefficient to zero (the first
//!    implementation had exactly this bug);
//! 2. setup transactions (account funding, package publishing, fixture
//!    creation) never reach `--profile-output` — they would contaminate the
//!    regression dataset.

use std::{path::PathBuf, sync::OnceLock, time::Duration};

use crate::{
    benchmark_context::BenchmarkContext,
    command::{BenchmarkConfig, Component, PtbParams, WorkloadKind},
    profile_capture::{self, ProfileCapture},
    run_benchmark,
    workload::Workload,
};

/// Install the real `ProfileCapture` layer — the same gating and
/// serialization the calibration scripts consume — as the global subscriber,
/// writing to one temp file for the whole process. Tests serialize on
/// [`TEST_LOCK`] and parse only the lines their own section appended.
fn capture_file() -> &'static PathBuf {
    static FILE: OnceLock<PathBuf> = OnceLock::new();
    FILE.get_or_init(|| {
        use tracing_subscriber::{
            Layer, filter::Targets, layer::SubscriberExt, util::SubscriberInitExt,
        };
        let path =
            std::env::temp_dir().join(format!("calibrate-pinning-{}.jsonl", std::process::id()));
        let capture = ProfileCapture::new(&path, serde_json::json!({"test": true})).unwrap();
        tracing_subscriber::registry()
            .with(
                capture.with_filter(
                    Targets::new().with_target("resource_profile", tracing::Level::TRACE),
                ),
            )
            .try_init()
            .expect("these tests must own the global subscriber");
        path
    })
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn file_len() -> u64 {
    std::fs::metadata(capture_file()).unwrap().len()
}

/// The `(measured_ns, profile)` rows appended after byte offset `from`.
fn rows_after(from: u64) -> Vec<(u64, serde_json::Value)> {
    std::fs::read_to_string(capture_file()).unwrap()[from as usize..]
        .lines()
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            (v["measured_ns"].as_u64().unwrap(), v["profile"].clone())
        })
        .collect()
}

fn calibration_config() -> BenchmarkConfig {
    BenchmarkConfig {
        sequential: true,
        skip_signing: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn measured_window_opens_before_input_object_loading() {
    let _serial = TEST_LOCK.lock().await;
    let workload = Workload::new(
        2,
        WorkloadKind::PTB(PtbParams {
            num_transfers: 1,
            ..Default::default()
        }),
    );
    let mut ctx = BenchmarkContext::new(
        workload.clone(),
        Component::ExecutionOnly,
        &calibration_config(),
    )
    .await;
    let tx_generator = workload.create_tx_generator(&mut ctx).await;
    let transactions = ctx.generate_transactions(tx_generator).await;
    let transactions = ctx.certify_transactions(transactions, true).await;
    profile_capture::set_capture_enabled(true);

    // Control: without the injected delay, pure execution time sits far below
    // it, so the assertion below cannot pass by accident on a slow machine.
    const DELAY: Duration = Duration::from_millis(200);
    let store = ctx.validator().create_in_memory_store();
    let offset = file_len();
    ctx.validator()
        .execute_transaction_in_memory(store, transactions[0].clone())
        .await;
    let control = rows_after(offset);
    assert_eq!(control.len(), 1);
    assert!(
        (control[0].0 as u128) < DELAY.as_nanos(),
        "control transaction took {} ns; the delay no longer dominates and \
         this test cannot distinguish window placements",
        control[0].0
    );

    // With input-object loading slowed by DELAY, a window that opens before
    // loading must measure at least DELAY; a window opened after loading
    // would measure roughly the control value and fail here.
    let store = ctx.validator().create_in_memory_store();
    store.set_read_delay(DELAY);
    let offset = file_len();
    ctx.validator()
        .execute_transaction_in_memory(store, transactions[1].clone())
        .await;
    let delayed = rows_after(offset);
    assert_eq!(delayed.len(), 1);
    assert!(
        (delayed[0].0 as u128) >= DELAY.as_nanos(),
        "measured_ns = {} ns with a {} ns input-loading delay: input-object \
         loading is no longer inside the measured window",
        delayed[0].0,
        DELAY.as_nanos()
    );
}

#[tokio::test]
async fn setup_transactions_never_reach_profile_output() {
    let _serial = TEST_LOCK.lock().await;
    // Dynamic fields force setup transactions (account funding plus one
    // root-object creation per account); only the 3 measured transactions may
    // appear in the capture.
    let tx_count = 3;
    let offset = file_len();
    run_benchmark(
        Workload::new(
            tx_count,
            WorkloadKind::PTB(PtbParams {
                num_dynamic_fields: 2,
                ..Default::default()
            }),
        ),
        Component::ExecutionOnly,
        calibration_config(),
    )
    .await;

    let rows = rows_after(offset);
    assert_eq!(
        rows.len(),
        tx_count as usize,
        "the capture must contain exactly the measured transactions, never \
         the setup transactions"
    );
    for (_, profile) in &rows {
        assert_eq!(
            profile["child_object_reads"], 2,
            "captured rows should be the measured dynamic-field workload"
        );
    }
}
