// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a stardust objects snapshot out of a Hornet snapshot.
//! TIP that defines the Hornet snapshot file format:
//! https://github.com/iotaledger/tips/blob/main/tips/TIP-0035/tip-0035.md
use std::fs::File;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use iota_genesis_builder::{
    stardust::{
        migration::{Migration, MigrationTargetNetwork},
        parse::FullSnapshotParser,
    },
    BROTLI_COMPRESSOR_BUFFER_SIZE, BROTLI_COMPRESSOR_LG_WINDOW_SIZE, BROTLI_COMPRESSOR_QUALITY,
    OBJECT_SNAPSHOT_FILE_PATH,
};
use iota_sdk::types::block::output::{
    unlock_condition::StorageDepositReturnUnlockCondition, AliasOutputBuilder, BasicOutputBuilder,
    FoundryOutputBuilder, NftOutputBuilder, Output,
};
use iota_types::stardust::coin_type::CoinType;
use itertools::Itertools;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[clap(about = "Tool for migrating Iota and Shimmer Hornet full-snapshot files")]
struct Cli {
    #[clap(subcommand)]
    snapshot: Snapshot,
}

#[derive(Subcommand, Debug)]
enum Snapshot {
    #[clap(about = "Migrate an Iota Hornet full-snapshot file")]
    Iota {
        #[clap(long, help = "Path to the Iota Hornet full-snapshot file")]
        snapshot_path: String,
        #[clap(long, value_parser = clap::value_parser!(MigrationTargetNetwork), help = "Target network for migration")]
        target_network: MigrationTargetNetwork,
    },
    #[clap(about = "Migrate a Shimmer Hornet full-snapshot file")]
    Shimmer {
        #[clap(long, help = "Path to the Shimmer Hornet full-snapshot file")]
        snapshot_path: String,
        #[clap(long, value_parser = clap::value_parser!(MigrationTargetNetwork), help = "Target network for migration")]
        target_network: MigrationTargetNetwork,
    },
}

fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Parse the CLI arguments
    let cli = Cli::parse();
    let (snapshot_path, target_network, coin_type) = match cli.snapshot {
        Snapshot::Iota {
            snapshot_path,
            target_network,
        } => (snapshot_path, target_network, CoinType::Iota),
        Snapshot::Shimmer {
            snapshot_path,
            target_network,
        } => (snapshot_path, target_network, CoinType::Shimmer),
    };

    // Start the Hornet snapshot parser
    let snapshot_parser = FullSnapshotParser::new(File::open(snapshot_path)?)?;

    // Prepare the migration using the parser output stream
    let migration = Migration::new(
        snapshot_parser.target_milestone_timestamp(),
        snapshot_parser.total_supply()?,
        target_network,
        coin_type,
    )?;

    // Prepare the compressor writer for the objects snapshot
    let object_snapshot_writer = brotli::CompressorWriter::new(
        File::create(OBJECT_SNAPSHOT_FILE_PATH)?,
        BROTLI_COMPRESSOR_BUFFER_SIZE,
        BROTLI_COMPRESSOR_QUALITY,
        BROTLI_COMPRESSOR_LG_WINDOW_SIZE,
    );

    // Run the migration and write the objects snapshot
    snapshot_parser
        .outputs()
        .map(|res| {
            if coin_type == CoinType::Iota {
                let (header, mut output) = res?;
                scale_output_amount_for_iota(&mut output)?;
                Ok((header, output))
            } else {
                res
            }
        })
        .process_results(|outputs| migration.run(outputs, object_snapshot_writer))??;
    Ok(())
}

fn scale_output_amount_for_iota(output: &mut Output) -> Result<()> {
    const IOTA_MULTIPLIER: u64 = 1000;

    fn multiply_amount(amount: u64) -> Result<u64> {
        amount
            .checked_mul(IOTA_MULTIPLIER)
            .ok_or_else(|| anyhow!("overflow multiplying amount {amount} by {IOTA_MULTIPLIER}"))
    }

    *output = match output {
        Output::Basic(ref basic_output) => {
            // Update amount
            let mut builder = BasicOutputBuilder::from(basic_output)
                .with_amount(multiply_amount(basic_output.amount())?);

            // Update amount in potential storage deposit return unlock condition
            if let Some(sdr_uc) = basic_output
                .unlock_conditions()
                .get(StorageDepositReturnUnlockCondition::KIND)
            {
                let sdr_uc = sdr_uc.as_storage_deposit_return();
                builder = builder.replace_unlock_condition(
                    StorageDepositReturnUnlockCondition::new(
                        sdr_uc.return_address(),
                        multiply_amount(sdr_uc.amount())?,
                        u64::MAX,
                    )
                    .unwrap(),
                );
            };

            Output::from(builder.finish()?)
        }
        Output::Alias(ref alias_output) => Output::from(
            AliasOutputBuilder::from(alias_output)
                .with_amount(multiply_amount(alias_output.amount())?)
                .finish()?,
        ),
        Output::Foundry(ref foundry_output) => Output::from(
            FoundryOutputBuilder::from(foundry_output)
                .with_amount(multiply_amount(foundry_output.amount())?)
                .finish()?,
        ),
        Output::Nft(ref nft_output) => {
            // Update amount
            let mut builder = NftOutputBuilder::from(nft_output)
                .with_amount(multiply_amount(nft_output.amount())?);

            // Update amount in potential storage deposit return unlock condition
            if let Some(sdr_uc) = nft_output
                .unlock_conditions()
                .get(StorageDepositReturnUnlockCondition::KIND)
            {
                let sdr_uc = sdr_uc.as_storage_deposit_return();
                builder = builder.replace_unlock_condition(
                    StorageDepositReturnUnlockCondition::new(
                        sdr_uc.return_address(),
                        multiply_amount(sdr_uc.amount())?,
                        u64::MAX,
                    )
                    .unwrap(),
                );
            };

            Output::from(builder.finish()?)
        }
        Output::Treasury(_) => return Ok(()),
    };
    Ok(())
}
