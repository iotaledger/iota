// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fs;

use iota_light_client::{
    checkpoint::{
        CheckpointList, verified_committee_for_epoch, write_checkpoint_list,
        write_checkpoint_summary,
    },
    config::Config,
};
use iota_sdk_types::{CheckpointContents, CheckpointSummary, EndOfEpochData, GasCostSummary};
use iota_swarm_config::{network_config::NetworkConfig, network_config_builder::ConfigBuilder};
use iota_types::{
    committee::Committee,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, SignedCheckpointSummary,
    },
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
