// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{unlock_condition::AddressUnlockCondition, BasicOutputBuilder},
};
use sui_types::base_types::ObjectID;

use crate::stardust::migration::{tests::random_output_header, Migration};

/// Test the id of a `BasicOutput` that is transformed to a simple coin.
///
/// Skips checks included in the verification step of the migration.
#[test]
fn basic_simple_coin_id() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_basic = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    let mut migration = Migration::new(1).unwrap();
    migration
        .run_migration([(header.clone(), stardust_basic.clone().into())])
        .unwrap();
    let migrated_object_id = migration
        .output_objects_map
        .get(&header.output_id())
        .unwrap()
        .coin()
        .unwrap();
    let expected_object_id = ObjectID::new(header.output_id().hash());
    assert_eq!(expected_object_id, *migrated_object_id);
}
