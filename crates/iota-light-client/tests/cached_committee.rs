// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fs;

use iota_config::genesis::Genesis;
use iota_light_client::{
    checkpoint::{
        CheckpointList, verified_committee_for_epoch, write_checkpoint_list,
        write_checkpoint_summary,
    },
    config::Config,
};
use iota_sdk_types::{
    CheckpointContents, CheckpointSummary, EndOfEpochData, GasCostSummary, TransactionEffects,
};
use iota_swarm_config::{network_config::NetworkConfig, network_config_builder::ConfigBuilder};
use iota_types::{
    committee::Committee,
    iota_system_state::get_iota_system_state_objects,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, SignedCheckpointSummary,
    },
    object::Object,
};
use tempfile::TempDir;

fn certified_summary(
    network: &NetworkConfig,
    epoch: u64,
    seq: u64,
    next_committee: Option<&Committee>,
) -> CertifiedCheckpointSummary {
    let summary = CheckpointSummary {
        epoch,
        sequence_number: seq,
        network_total_transactions: 0,
        contents_digest: CheckpointContents::new_with_digests_only_for_tests(vec![]).digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: GasCostSummary::default(),
        end_of_epoch_data: next_committee.map(|committee| EndOfEpochData {
            next_epoch_committee: committee.committee_members(),
            next_epoch_protocol_version: 1,
            epoch_commitments: vec![],
            epoch_supply_change: 0,
        }),
        timestamp_ms: 0,
        version_specific_data: vec![],
        checkpoint_commitments: vec![],
    };
    certify_summary(network, summary)
}

fn certify_summary(
    network: &NetworkConfig,
    summary: CheckpointSummary,
) -> CertifiedCheckpointSummary {
    let epoch = summary.epoch;
    let committee = Committee::from_committee_members(
        epoch,
        &network.genesis.committee().unwrap().committee_members(),
    );
    let signatures = network
        .validator_configs()
        .iter()
        .map(|validator| {
            SignedCheckpointSummary::sign(
                epoch,
                &summary,
                validator.authority_key_pair(),
                validator.authority_public_key(),
            )
        })
        .collect();
    CertifiedCheckpointSummary::new(summary, signatures, &committee).unwrap()
}

#[test]
fn cached_committee_chain_requires_every_authenticated_epoch() {
    let trusted_dir = TempDir::new().unwrap();
    let trusted = ConfigBuilder::new(&trusted_dir).build();
    let other_dir = TempDir::new().unwrap();
    let other = ConfigBuilder::new(&other_dir)
        .deterministic_committee_size(std::num::NonZeroUsize::new(1).unwrap())
        .build();
    assert_ne!(
        trusted.genesis.committee().unwrap(),
        other.genesis.committee().unwrap()
    );
    let trusted_committee = trusted.genesis.committee().unwrap();
    let other_committee = other.genesis.committee().unwrap();
    let temp = TempDir::new().unwrap();
    let mut config = Config::mainnet();
    config.checkpoints_dir = temp.path().to_path_buf();
    config.sync_before_check = false;
    config.genesis_checkpoint_digest = *trusted.genesis.checkpoint().digest();
    trusted
        .genesis
        .save(config.genesis_blob_file_path())
        .unwrap();
    assert_eq!(
        verified_committee_for_epoch(&config, 0).unwrap(),
        trusted_committee
    );
    assert!(verified_committee_for_epoch(&config, 1).is_err());

    let first = certified_summary(&trusted, 0, 10, Some(&other_committee));
    let second = certified_summary(&other, 1, 20, Some(&trusted_committee));
    write_checkpoint_summary(&config, &first).unwrap();
    write_checkpoint_summary(&config, &second).unwrap();
    // A later uncached/unverified epoch cannot prevent checking an older one.
    write_checkpoint_list(
        &config,
        &CheckpointList {
            checkpoints: vec![10, 20, 30],
        },
    )
    .unwrap();
    assert_eq!(
        verified_committee_for_epoch(&config, 1).unwrap(),
        Committee::from_committee_members(1, &other_committee.committee_members())
    );
    assert_eq!(
        verified_committee_for_epoch(&config, 2).unwrap(),
        Committee::from_committee_members(2, &trusted_committee.committee_members())
    );
    assert!(verified_committee_for_epoch(&config, u64::MAX).is_err());

    for invalid in [
        "missing",
        "malformed",
        "wrong-file",
        "nonclosing",
        "wrong-epoch",
        "incomplete",
        "wrong-signers",
    ] {
        write_checkpoint_list(
            &config,
            &CheckpointList {
                checkpoints: vec![10, 20],
            },
        )
        .unwrap();
        write_checkpoint_summary(&config, &first).unwrap();
        match invalid {
            "missing" => fs::remove_file(config.checkpoint_summary_file_path(10)).unwrap(),
            "malformed" => {
                fs::write(config.checkpoint_summary_file_path(10), b"invalid BCS").unwrap()
            }
            "wrong-file" => fs::write(
                config.checkpoint_summary_file_path(10),
                bcs::to_bytes(&certified_summary(&trusted, 0, 99, Some(&other_committee))).unwrap(),
            )
            .unwrap(),
            "nonclosing" => {
                write_checkpoint_summary(&config, &certified_summary(&trusted, 0, 10, None))
                    .unwrap()
            }
            "wrong-epoch" => write_checkpoint_summary(
                &config,
                &certified_summary(&trusted, 1, 10, Some(&other_committee)),
            )
            .unwrap(),
            "incomplete" => write_checkpoint_list(
                &config,
                &CheckpointList {
                    checkpoints: vec![10],
                },
            )
            .unwrap(),
            "wrong-signers" => write_checkpoint_summary(
                &config,
                &certified_summary(&other, 0, 10, Some(&other_committee)),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        assert!(
            verified_committee_for_epoch(&config, 2).is_err(),
            "{invalid} cache must be rejected"
        );
        assert!(
            invalid == "missing" || config.checkpoint_summary_file_path(10).exists(),
            "{invalid} cache must be left in place"
        );
    }
}

#[test]
fn genesis_blob_must_match_the_pinned_checkpoint_digest() {
    let trusted_dir = TempDir::new().unwrap();
    let trusted = ConfigBuilder::new(&trusted_dir).build();
    let other_dir = TempDir::new().unwrap();
    let other = ConfigBuilder::new(&other_dir)
        .deterministic_committee_size(std::num::NonZeroUsize::new(1).unwrap())
        .build();
    let temp = TempDir::new().unwrap();
    let mut config = Config::mainnet();
    config.checkpoints_dir = temp.path().to_path_buf();
    config.genesis_checkpoint_digest = *trusted.genesis.checkpoint().digest();

    trusted
        .genesis
        .save(config.genesis_blob_file_path())
        .unwrap();
    assert_eq!(
        verified_committee_for_epoch(&config, 0).unwrap(),
        trusted.genesis.committee().unwrap()
    );

    // Each case is rejected by exactly one check of `verified_committee`, so
    // that skipping any check fails the test.
    for invalid in [
        "other-genesis",
        "other-below-checkpoint",
        "other-wrapper-system-state-object",
        "other-inner-system-state-object",
        "forged-effects",
    ] {
        let [other_wrapper, other_inner] =
            get_iota_system_state_objects(&other.genesis.objects()).unwrap();
        let genesis = match invalid {
            "other-genesis" => other.genesis.clone(),
            "other-below-checkpoint" => genesis_from(
                &trusted.genesis,
                &other.genesis,
                &other.genesis,
                other.genesis.objects().to_vec(),
            ),
            "other-wrapper-system-state-object" => genesis_from(
                &trusted.genesis,
                &trusted.genesis,
                &trusted.genesis,
                objects_with(trusted.genesis.objects(), other_wrapper),
            ),
            "other-inner-system-state-object" => genesis_from(
                &trusted.genesis,
                &trusted.genesis,
                &trusted.genesis,
                objects_with(trusted.genesis.objects(), other_inner),
            ),
            // The effects keep the genuine transaction digest but list the
            // other inner system state object, so that only the effects digest
            // in the checkpoint contents can reject the blob.
            "forged-effects" => {
                let mut effects = trusted.genesis.effects().clone();
                let TransactionEffects::V1(forged) = &mut effects else {
                    unreachable!()
                };
                let TransactionEffects::V1(other_effects) = other.genesis.effects() else {
                    unreachable!()
                };
                let entry = forged
                    .changed_objects
                    .iter_mut()
                    .find(|changed| changed.object_id == other_inner.id())
                    .unwrap();
                *entry = other_effects
                    .changed_objects
                    .iter()
                    .find(|changed| changed.object_id == other_inner.id())
                    .unwrap()
                    .clone();
                Genesis::new(
                    trusted.genesis.checkpoint().into_inner(),
                    trusted.genesis.checkpoint_contents().clone(),
                    trusted.genesis.transaction().clone(),
                    effects,
                    trusted.genesis.events().clone(),
                    objects_with(trusted.genesis.objects(), other_inner),
                )
            }
            _ => unreachable!(),
        };
        genesis.save(config.genesis_blob_file_path()).unwrap();
        assert!(
            verified_committee_for_epoch(&config, 0).is_err(),
            "{invalid} genesis must be rejected"
        );
    }
}

fn genesis_from(
    checkpoint: &Genesis,
    contents: &Genesis,
    transaction: &Genesis,
    objects: Vec<Object>,
) -> Genesis {
    Genesis::new(
        checkpoint.checkpoint().into_inner(),
        contents.checkpoint_contents().clone(),
        transaction.transaction().clone(),
        transaction.effects().clone(),
        transaction.events().clone(),
        objects,
    )
}

fn objects_with(objects: &[Object], replacement: Object) -> Vec<Object> {
    let mut objects = objects.to_vec();
    let position = objects
        .iter()
        .position(|object| object.id() == replacement.id())
        .unwrap();
    objects[position] = replacement;
    objects
}
