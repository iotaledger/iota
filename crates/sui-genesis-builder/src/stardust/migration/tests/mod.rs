// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::address::AliasAddress;
use iota_sdk::types::block::output::feature::Irc30Metadata;
use iota_sdk::types::block::output::feature::MetadataFeature;
use iota_sdk::types::block::output::unlock_condition::ImmutableAliasAddressUnlockCondition;
use iota_sdk::types::block::output::AliasId;
use iota_sdk::types::block::output::Feature;
use iota_sdk::types::block::output::FoundryOutput;
use iota_sdk::types::block::output::FoundryOutputBuilder;
use iota_sdk::types::block::output::Output;
use iota_sdk::types::block::output::SimpleTokenScheme;
use iota_sdk::types::block::output::TokenScheme;

use crate::stardust::migration::executor::Executor;
use crate::stardust::migration::migration::Migration;
use crate::stardust::types::snapshot::OutputHeader;

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
