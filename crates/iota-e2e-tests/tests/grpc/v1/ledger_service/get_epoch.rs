// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil, read_masks::GET_EPOCH_READ_MASK, v1::ledger_service::GetEpochRequest,
};
use iota_macros::sim_test;
use prost_types::FieldMask;

use crate::utils::{assert_field_presence, comma_separated_field_mask_to_paths, setup_grpc_test};

#[sim_test]
async fn get_epoch_current_and_by_number() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut ledger_client = client.ledger_service_client();

    // The test cluster only spans a single epoch, so a default request
    // (current epoch) and an explicit `epoch=0` request must return the same
    // committee. Request `committee` explicitly — it is not in the default
    // `GET_EPOCH_READ_MASK`, and without it both sides would be `None` and
    // the comparison would be trivially true.
    let read_mask = FieldMask::from_paths(["epoch", "first_checkpoint", "committee"]);

    let latest_epoch = ledger_client
        .get_epoch(GetEpochRequest::default().with_read_mask(read_mask.clone()))
        .await
        .unwrap()
        .into_inner()
        .epoch
        .unwrap();

    let epoch_0 = ledger_client
        .get_epoch(
            GetEpochRequest::default()
                .with_epoch(0)
                .with_read_mask(read_mask),
        )
        .await
        .unwrap()
        .into_inner()
        .epoch
        .unwrap();

    assert!(
        epoch_0.committee.is_some(),
        "committee must be populated when requested in the read mask"
    );
    assert_eq!(latest_epoch.committee, epoch_0.committee);
    assert_eq!(epoch_0.epoch, Some(0));
    assert_eq!(epoch_0.first_checkpoint, Some(0));
}

#[sim_test]
async fn get_epoch_readmask_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut ledger_client = client.ledger_service_client();

    // The default `GET_EPOCH_READ_MASK` requests `end` and `last_checkpoint`,
    // but the server only populates them for epochs that have already ended.
    // The test cluster sits in epoch 0 (no epoch boundary is crossed), so for
    // the default-readmask scenario both fields are deterministically `None` —
    // we drop them from the expected list and let the per-field assertion
    // verify they're absent.
    let default_expected_paths: Vec<&str> =
        comma_separated_field_mask_to_paths(GET_EPOCH_READ_MASK)
            .into_iter()
            .filter(|p| *p != "end" && *p != "last_checkpoint")
            .collect();

    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        ("default readmask", None, default_expected_paths),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        (
            "partial readmask (bcs_system_state only)",
            Some(FieldMask::from_paths(["bcs_system_state"])),
            vec!["bcs_system_state"],
        ),
        (
            "partial readmask (epoch + reference_gas_price)",
            Some(FieldMask::from_paths(["epoch", "reference_gas_price"])),
            vec!["epoch", "reference_gas_price"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        let mut request = GetEpochRequest::default();
        if let Some(mask) = mask {
            request = request.with_read_mask(mask);
        }

        let epoch = ledger_client
            .get_epoch(request)
            .await
            .unwrap()
            .into_inner()
            .epoch
            .unwrap();

        assert_field_presence(&epoch, &expected_paths, &[], scenario);
    }
}
