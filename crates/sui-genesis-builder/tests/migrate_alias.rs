use fastcrypto::hash::{Blake2b256, HashFunction};
use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        feature::{IssuerFeature, MetadataFeature, SenderFeature},
        unlock_condition::{GovernorAddressUnlockCondition, StateControllerAddressUnlockCondition},
        AliasId, AliasOutput as StardustAlias, AliasOutputBuilder, Feature,
    },
};
use sui_genesis_builder::stardust::{
    migration::Migration,
    types::{snapshot::OutputHeader, Alias, AliasOutput},
};
use sui_types::{base_types::ObjectID, id::UID, object::Object};

fn migrate_alias(
    header: OutputHeader,
    stardust_alias: StardustAlias,
) -> (ObjectID, Alias, ObjectID, AliasOutput) {
    let alias_id: AliasId = stardust_alias.alias_id().to_owned();
    let mut snapshot_buffer = Vec::new();
    Migration::new()
        .unwrap()
        .run(
            [].into_iter(),
            [(header, stardust_alias.into())].into_iter(),
            &mut snapshot_buffer,
        )
        .unwrap();

    let migrated_objects: Vec<Object> = bcs::from_bytes(&snapshot_buffer).unwrap();

    // Ensure the migrated objects exist under the expected identifiers.
    // We know the alias_id is non-zero here, so we don't need to use `or_from_output_id`.
    let alias_object_id = ObjectID::new(*alias_id);
    let alias_output_object_id = ObjectID::new(Blake2b256::digest(*alias_id).into());
    let alias_object = migrated_objects
        .iter()
        .find(|obj| obj.id() == alias_object_id)
        .expect("alias object should be present in the migrated snapshot");
    assert_eq!(alias_object.struct_tag().unwrap(), Alias::tag(),);
    let alias_output_object = migrated_objects
        .iter()
        .find(|obj| obj.id() == alias_output_object_id)
        .expect("alias object should be present in the migrated snapshot");
    assert_eq!(
        alias_output_object.struct_tag().unwrap(),
        AliasOutput::tag(),
    );

    let alias_output: AliasOutput =
        bcs::from_bytes(alias_output_object.data.try_as_move().unwrap().contents()).unwrap();
    let alias: Alias =
        bcs::from_bytes(alias_object.data.try_as_move().unwrap().contents()).unwrap();

    (alias_object_id, alias, alias_output_object_id, alias_output)
}

/// Test that the migrated alias objects in storage contain the expected data.
#[test]
fn alias_migration_test() {
    let alias_id = AliasId::new(rand::random());
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = OutputHeader::new_testing(
        rand::random(),
        rand::random(),
        rand::random(),
        rand::random(),
    );

    let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, alias_id)
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
        .with_state_metadata([0xff; 1])
        .with_features(vec![
            Feature::Metadata(MetadataFeature::new([0xdd; 1]).unwrap()),
            Feature::Sender(SenderFeature::new(random_address)),
        ])
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new([0xaa; 1]).unwrap()),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .with_state_index(3)
        .finish()
        .unwrap();

    let (alias_object_id, alias, alias_output_object_id, alias_output) =
        migrate_alias(header, stardust_alias.clone());
    let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

    // Compare only ID and iota balance. The bag is tested separately.
    assert_eq!(UID::new(alias_output_object_id), alias_output.id);
    assert_eq!(stardust_alias.amount(), alias_output.iota.value());

    assert_eq!(expected_alias, alias);
}
