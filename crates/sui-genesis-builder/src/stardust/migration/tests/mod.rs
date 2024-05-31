// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::{
    address::AliasAddress,
    output::{
        feature::{Irc30Metadata, MetadataFeature},
        unlock_condition::ImmutableAliasAddressUnlockCondition,
        AliasId, Feature, FoundryOutput, FoundryOutputBuilder, Output, SimpleTokenScheme,
        TokenScheme,
    },
};

use crate::stardust::{
    migration::{executor::Executor, migration::Migration},
    types::snapshot::OutputHeader,
};

mod alias;
mod executor;

fn random_output_header() -> OutputHeader {
    OutputHeader::new_testing(
        rand::random(),
        rand::random(),
        rand::random(),
        rand::random(),
    )
}

fn run_migration(outputs: impl IntoIterator<Item = (OutputHeader, Output)>) -> Executor {
    let mut migration = Migration::new(1).unwrap();
    migration.run_migration(outputs).unwrap();
    migration.into_executor()
}

fn create_foundry(
    iota_amount: u64,
    token_scheme: SimpleTokenScheme,
    irc_30_metadata: Irc30Metadata,
    alias_id: AliasId,
) -> (OutputHeader, FoundryOutput) {
    let builder =
        FoundryOutputBuilder::new_with_amount(iota_amount, 1, TokenScheme::Simple(token_scheme))
            .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                AliasAddress::new(alias_id),
            ))
            .add_feature(Feature::Metadata(
                MetadataFeature::new(irc_30_metadata).unwrap(),
            ));
    let foundry_output = builder.finish().unwrap();

    (random_output_header(), foundry_output)
}
