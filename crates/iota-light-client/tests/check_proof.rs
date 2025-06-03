// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::{self},
    io::Read,
    path::PathBuf,
};

use anyhow::Context;
use iota_light_client::{
    checkpoint::read_checkpoint_list,
    config::Config,
    construct::construct_proof,
    proof::{Proof, ProofTarget, verify_proof},
};
use iota_types::{
    committee::Committee,
    effects::TransactionEffectsAPI,
    event::{Event, EventID},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CertifiedCheckpointSummary,
    object::Object,
};

const FIXTURES_DIR: &str = "tests/fixtures";

async fn read_test_data() -> (Committee, CheckpointData) {
    let mut config = Config::get_mainnet_config();
    config.checkpoints_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);

    let checkpoint_list =
        read_checkpoint_list(&config).expect("reading the checkpoints.yaml should not fail");
    let committee_seq = checkpoint_list
        .checkpoints()
        .first()
        .expect("there should be a first checkpoint in the checkpoints.yaml");
    let seq = checkpoint_list
        .checkpoints()
        .get(1)
        .expect("there should be a second checkpoint in the checkpoints.yaml");

    read_data(*committee_seq, *seq).await
}

async fn read_data(committee_seq: u64, seq: u64) -> (Committee, CheckpointData) {
    let checkpoint_summary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES_DIR)
        .join(format!("{committee_seq}.sum"));
    let summary = read_checkpoint_summary(&checkpoint_summary_path)
        .await
        .unwrap();
    let prev_committee = summary
        .end_of_epoch_data
        .as_ref()
        .expect("Expected all checkpoints to be end-of-epoch checkpoints")
        .next_epoch_committee
        .iter()
        .cloned()
        .collect();

    let committee = Committee::new(summary.epoch().checked_add(1).unwrap(), prev_committee);

    let full_checkpoint_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES_DIR)
        .join(format!("{seq}.chk"));
    let full_checkpoint = read_full_checkpoint(&full_checkpoint_path).await.unwrap();

    (committee, full_checkpoint)
}

async fn read_checkpoint_summary(
    checkpoint_path: &PathBuf,
) -> anyhow::Result<CertifiedCheckpointSummary> {
    let mut reader = fs::File::open(checkpoint_path.clone())?;
    let metadata = fs::metadata(checkpoint_path)?;
    let mut buffer = vec![0; metadata.len() as usize];
    reader.read_exact(&mut buffer)?;
    bcs::from_bytes(&buffer).context("failed to deserialize summary from bcs bytes")
}

async fn read_full_checkpoint(checkpoint_path: &PathBuf) -> anyhow::Result<CheckpointData> {
    let mut reader = fs::File::open(checkpoint_path.clone())?;
    let metadata = fs::metadata(checkpoint_path)?;
    let mut buffer = vec![0; metadata.len() as usize];
    reader.read_exact(&mut buffer)?;
    bcs::from_bytes(&buffer).context("failed to deserialize full checkpoint from bcs bytes")
}

#[tokio::test]
async fn check_can_read_test_data() {
    let (_committee, full_checkpoint) = read_test_data().await;
    assert!(
        full_checkpoint
            .checkpoint_summary
            .end_of_epoch_data
            .is_some()
    );
}

#[tokio::test]
async fn test_new_committee() {
    let (committee, full_checkpoint) = read_test_data().await;

    let new_committee_data = full_checkpoint
        .checkpoint_summary
        .end_of_epoch_data
        .as_ref()
        .expect("Expected checkpoint to be end-of-epoch")
        .next_epoch_committee
        .iter()
        .cloned()
        .collect();

    // Make a committee object using this
    let new_committee = Committee::new(
        full_checkpoint
            .checkpoint_summary
            .epoch()
            .checked_add(1)
            .unwrap(),
        new_committee_data,
    );

    let committee_proof = Proof {
        checkpoint_summary: full_checkpoint.checkpoint_summary.clone(),
        contents_proof: None,
        targets: ProofTarget::new().set_committee(new_committee.clone()),
    };

    verify_proof(&committee, &committee_proof).unwrap()
}

// Fail if the new committee does not match the target of the proof
#[tokio::test]
async fn test_incorrect_new_committee() {
    let (committee, full_checkpoint) = read_test_data().await;

    let committee_proof = Proof {
        checkpoint_summary: full_checkpoint.checkpoint_summary.clone(),
        contents_proof: None,
        targets: ProofTarget::new().set_committee(committee.clone()), // WRONG,
    };

    assert!(verify_proof(&committee, &committee_proof).is_err());
}

// Fail if the certificate is incorrect even if no proof targets are given
#[tokio::test]
async fn test_fail_incorrect_cert() {
    let (_committee, full_checkpoint) = read_test_data().await;

    let new_committee_data = full_checkpoint
        .checkpoint_summary
        .end_of_epoch_data
        .as_ref()
        .expect("expected checkpoint to be end-of-epoch")
        .next_epoch_committee
        .iter()
        .cloned()
        .collect();

    // Make a committee object using this
    let new_committee = Committee::new(
        full_checkpoint
            .checkpoint_summary
            .epoch()
            .checked_add(1)
            .unwrap(),
        new_committee_data,
    );

    let committee_proof = Proof {
        checkpoint_summary: full_checkpoint.checkpoint_summary.clone(),
        contents_proof: None,
        targets: ProofTarget::new(),
    };

    assert!(
        verify_proof(
            &new_committee, // WRONG
            &committee_proof
        )
        .is_err()
    );
}

#[tokio::test]
async fn test_object_target_fail_no_data() {
    let (committee, full_checkpoint) = read_test_data().await;

    let sample_object: Object = full_checkpoint.transactions[0].output_objects[0].clone();
    let sample_ref = sample_object.compute_object_reference();

    let bad_proof = Proof {
        checkpoint_summary: full_checkpoint.checkpoint_summary.clone(),
        contents_proof: None, // WRONG
        targets: ProofTarget::new().add_object(sample_ref, sample_object),
    };

    assert!(verify_proof(&committee, &bad_proof).is_err());
}

#[tokio::test]
async fn test_object_target_success() {
    let (committee, full_checkpoint) = read_test_data().await;

    let sample_object: Object = full_checkpoint.transactions[0].output_objects[0].clone();
    let sample_ref = sample_object.compute_object_reference();

    let target = ProofTarget::new().add_object(sample_ref, sample_object);
    let object_proof = construct_proof(target, &full_checkpoint).unwrap();

    assert!(verify_proof(&committee, &object_proof).is_ok());
}

#[tokio::test]
async fn test_object_target_fail_wrong_object() {
    let (committee, full_checkpoint) = read_test_data().await;

    let sample_object: Object = full_checkpoint.transactions[0].output_objects[0].clone();
    let wrong_object: Object = full_checkpoint.transactions[1].output_objects[1].clone();
    let mut sample_ref = sample_object.compute_object_reference();
    let wrong_ref = wrong_object.compute_object_reference();

    let target = ProofTarget::new().add_object(wrong_ref, sample_object.clone()); // WRONG
    let object_proof = construct_proof(target, &full_checkpoint).unwrap();
    assert!(verify_proof(&committee, &object_proof).is_err());

    // Does not exist
    sample_ref.1 = sample_ref.1.next(); // WRONG

    let target = ProofTarget::new().add_object(sample_ref, sample_object);
    let object_proof = construct_proof(target, &full_checkpoint).unwrap();
    assert!(verify_proof(&committee, &object_proof).is_err());
}

#[tokio::test]
async fn test_event_target_fail_no_data() {
    let (committee, full_checkpoint) = read_test_data().await;

    let sample_event: Event = full_checkpoint.transactions[1]
        .events
        .as_ref()
        .unwrap()
        .data[0]
        .clone();
    let sample_eid = EventID::from((
        *full_checkpoint.transactions[1].effects.transaction_digest(),
        0,
    ));

    let bad_proof = Proof {
        checkpoint_summary: full_checkpoint.checkpoint_summary.clone(),
        contents_proof: None, // WRONG
        targets: ProofTarget::new().add_event(sample_eid, sample_event),
    };

    assert!(verify_proof(&committee, &bad_proof).is_err());
}

#[tokio::test]
async fn test_event_target_success() {
    let (committee, full_checkpoint) = read_test_data().await;

    let sample_event: Event = full_checkpoint.transactions[1]
        .events
        .as_ref()
        .unwrap()
        .data[0]
        .clone();
    let sample_eid = EventID::from((
        *full_checkpoint.transactions[1].effects.transaction_digest(),
        0,
    ));

    let target = ProofTarget::new().add_event(sample_eid, sample_event);
    let event_proof = construct_proof(target, &full_checkpoint).unwrap();

    assert!(verify_proof(&committee, &event_proof).is_ok());
}

#[tokio::test]
async fn test_event_target_fail_bad_event() {
    let (committee, full_checkpoint) = read_test_data().await;

    let sample_event: Event = full_checkpoint.transactions[1]
        .events
        .as_ref()
        .unwrap()
        .data[0]
        .clone();
    let sample_eid = EventID::from((
        *full_checkpoint.transactions[1].effects.transaction_digest(),
        1, // WRONG
    ));

    let target = ProofTarget::new().add_event(sample_eid, sample_event);
    let event_proof = construct_proof(target, &full_checkpoint).unwrap();

    assert!(verify_proof(&committee, &event_proof).is_err());
}
