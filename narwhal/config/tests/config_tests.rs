// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::mutable_key_type)]

// This file contains tests that detect changes in Narwhal configs and
// parameters. If a PR breaks one or more tests here, the PR probably has a real
// impact on a Narwhal configuration file. When test failure happens, the PR
// should be marked as a breaking change and reviewers should be aware of this.
//
// Owners and operators of production configuration files can add themselves to
// .github/CODEOWNERS for the corresponding snapshot tests, so they can get
// notified of changes. PRs that modifies snapshot files should wait for reviews
// from code owners (if any) before merging.
//
// To review snapshot changes, and fix snapshot differences,
// 0. Install cargo-insta
// 1. Run `cargo insta test --review` under `./config`.
// 2. Review, accept or reject changes.

use std::collections::HashMap;

use insta::assert_json_snapshot;
use rand::{SeedableRng, rngs::StdRng};
use test_utils::CommitteeFixture;

#[test]
fn leader_election_rotates_through_all() {
    // this committee has equi-sized stakes
    let fixture = CommitteeFixture::builder().build();
    let committee = fixture.committee();
    let mut leader_counts = HashMap::new();
    // We most probably will only call `leader` on even rounds, so let's check this
    // still lets us use the whole roster of leaders.
    let mut leader_counts_stepping_by_2 = HashMap::new();
    for i in 0..100 {
        let leader = committee.leader(i);
        let leader_id = leader.id();

        let leader_stepping_by_2 = committee.leader(i * 2);
        let leader_steeping_by_2_id = leader_stepping_by_2.id();

        *leader_counts.entry(leader_id).or_insert(0) += 1;
        *leader_counts_stepping_by_2
            .entry(leader_steeping_by_2_id)
            .or_insert(0) += 1;
    }
    assert!(leader_counts.values().all(|v| *v >= 20));
    assert!(leader_counts_stepping_by_2.values().all(|v| *v >= 20));
}

#[test]
fn commmittee_snapshot_matches() {
    // The shape of this configuration is load-bearing in the NW benchmarks,
    // and in Iota (prod)
    let rng = StdRng::from_seed([0; 32]);
    let fixture = CommitteeFixture::builder().rng(rng).build();
    let committee = fixture.committee();

    // we need authorities to be serialized in order
    let mut settings = insta::Settings::clone_current();
    settings.set_sort_maps(true);
    settings.bind(|| assert_json_snapshot!("committee", committee));
}

#[test]
fn workers_snapshot_matches() {
    // The shape of this configuration is load-bearing in the NW benchmarks,
    // and in Iota (prod)
    let rng = StdRng::from_seed([0; 32]);
    let fixture = CommitteeFixture::builder().rng(rng).build();
    let worker_cache = fixture.worker_cache();

    // we need authorities to be serialized in order
    let mut settings = insta::Settings::clone_current();
    settings.set_sort_maps(true);
    settings.bind(|| assert_json_snapshot!("worker_cache", worker_cache));
}
