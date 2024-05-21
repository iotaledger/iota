use iota_sdk::types::block::{
  address::Ed25519Address,
  output::{
      feature::{IssuerFeature, MetadataFeature, SenderFeature},
      unlock_condition::{GovernorAddressUnlockCondition, StateControllerAddressUnlockCondition},
      AliasId, AliasOutput as StardustAlias, AliasOutputBuilder, Feature,
  },
};
use crate::stardust::{
  migration::Migration,
  types::{snapshot::OutputHeader, Alias, AliasOutput},
};
use sui_types::{base_types::ObjectID, object::Object};

fn migrate_alias(
  header: OutputHeader,
  stardust_alias: StardustAlias,
) -> (ObjectID, Alias, AliasOutput) {
  let alias_id: AliasId = stardust_alias
      .alias_id()
      .or_from_output_id(&header.output_id())
      .to_owned();
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
  let alias_object_id = ObjectID::new(*alias_id);
  let alias_object = migrated_objects
      .iter()
      .find(|obj| obj.id() == alias_object_id)
      .expect("alias object should be present in the migrated snapshot");
  assert_eq!(alias_object.struct_tag().unwrap(), Alias::tag(),);
  let alias_output_object = migrated_objects
      .iter()
      .find(|obj| match obj.struct_tag() {
          Some(tag) => tag == AliasOutput::tag(),
          None => false,
      })
      .expect("alias object should be present in the migrated snapshot");

  // Version is set to 1 when the alias is created based on the computed lamport timestamp.
  // When the alias is attached to the alias output, the version should be incremented.
  assert!(
      alias_object.version().value() > 1,
      "alias object version should have been incremented"
  );
  assert!(
      alias_output_object.version().value() > 1,
      "alias output object version should have been incremented"
  );

  let alias_output: AliasOutput =
      bcs::from_bytes(alias_output_object.data.try_as_move().unwrap().contents()).unwrap();
  let alias: Alias =
      bcs::from_bytes(alias_object.data.try_as_move().unwrap().contents()).unwrap();

  (alias_object_id, alias, alias_output)
}

/// Test that the migrated alias objects in the snapshot contain the expected data.
#[test]
fn test_alias_migration() {
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

  let (alias_object_id, alias, alias_output) = migrate_alias(header, stardust_alias.clone());
  let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

  // Compare only the balance. The ID is newly generated and the bag is tested separately.
  assert_eq!(stardust_alias.amount(), alias_output.iota.value());

  assert_eq!(expected_alias, alias);
}

#[test]
fn test_alias_migration_with_zeroed_id() {
  let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
  let header = OutputHeader::new_testing(
      rand::random(),
      rand::random(),
      rand::random(),
      rand::random(),
  );

  let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::null())
      .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
      .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
      .finish()
      .unwrap();

  // If this function does not panic, then the created aliases
  // were found at the correct non-zeroed Alias ID.
  migrate_alias(header, stardust_alias);
}
