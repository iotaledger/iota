// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creating a stardust objects snapshot out of a Hornet snapshot.
//! TIP that defines the Hornet snapshot file format:
//! https://github.com/iotaledger/tips/blob/main/tips/TIP-0035/tip-0035.md
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
};

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use iota_genesis_builder::{
    stardust::{
        migration::{Migration, MigrationTargetNetwork},
        parse::HornetSnapshotParser,
        types::output_header::OutputHeader,
    },
    BROTLI_COMPRESSOR_BUFFER_SIZE, BROTLI_COMPRESSOR_LG_WINDOW_SIZE, BROTLI_COMPRESSOR_QUALITY,
    OBJECT_SNAPSHOT_FILE_PATH,
};
use iota_sdk::types::block::{
    address::Address,
    output::{
        unlock_condition::{AddressUnlockCondition, StorageDepositReturnUnlockCondition},
        AliasOutputBuilder, BasicOutputBuilder, FoundryOutputBuilder, NftOutputBuilder, Output,
    },
};
use iota_types::{stardust::coin_type::CoinType, timelock::timelock::is_vested_reward};
use itertools::Itertools;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[clap(about = "Tool for migrating Iota and Shimmer Hornet full-snapshot files")]
struct Cli {
    #[clap(subcommand)]
    snapshot: Snapshot,
    #[clap(
        short,
        long,
        default_value_t = false,
        help = "Compress the resulting object snapshot"
    )]
    compress: bool,
    #[clap(long, help = "Disable global snapshot verification")]
    disable_global_snapshot_verification: bool,
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
    let mut snapshot_parser = if cli.disable_global_snapshot_verification {
        HornetSnapshotParser::new::<false>(File::open(snapshot_path)?)?
    } else {
        HornetSnapshotParser::new::<true>(File::open(snapshot_path)?)?
    };
    let total_supply = match coin_type {
        CoinType::Iota => scale_amount_for_iota(snapshot_parser.total_supply()?)?,
        CoinType::Shimmer => snapshot_parser.total_supply()?,
    };

    // Prepare the migration using the parser output stream
    let migration = Migration::new(
        snapshot_parser.target_milestone_timestamp(),
        total_supply,
        target_network,
        coin_type,
    )?;

    // Prepare the writer for the objects snapshot
    let output_file = File::create(OBJECT_SNAPSHOT_FILE_PATH)?;
    let object_snapshot_writer: Box<dyn Write> = if cli.compress {
        Box::new(brotli::CompressorWriter::new(
            output_file,
            BROTLI_COMPRESSOR_BUFFER_SIZE,
            BROTLI_COMPRESSOR_QUALITY,
            BROTLI_COMPRESSOR_LG_WINDOW_SIZE,
        ))
    } else {
        Box::new(BufWriter::new(output_file))
    };

    let mut filtered_outputs_cnt = 0;
    match coin_type {
        CoinType::Shimmer => {
            // Run the migration and write the objects snapshot
            snapshot_parser
                .outputs()
                .process_results(|outputs| migration.run(outputs, object_snapshot_writer))??;
        }
        CoinType::Iota => {
            let mut unlocked_address_balances = HashMap::new();
            let snapshot_timestamp_s = snapshot_parser.target_milestone_timestamp();

            let filtered_outputs: Vec<_> = snapshot_parser
                .outputs()
                .filter(|res| {
                    let Ok((ref header, ref output)) = res else {
                        panic!("reading output from snapshot should not return an error")
                    };

                    // we filter all vesting outputs that are already unlockable
                    let filtered = collect_unlocked_vesting_outputs(
                        &mut unlocked_address_balances,
                        snapshot_timestamp_s,
                        header,
                        output,
                    );

                    if filtered {
                        *(&mut filtered_outputs_cnt) += 1;
                    }

                    !filtered
                })
                .collect(); // we need to collect here to be able to chain the new aggregated outputs from the unlocked vesting output balances

                println!("filtered vesting outputs: {}, filtered unique addresses: {}", filtered_outputs_cnt, unlocked_address_balances.len());

            // Run the migration and write the objects snapshot
            filtered_outputs
                .into_iter()
                .chain(
                    unlocked_address_balances
                        .into_iter()
                        .map(|(address, output_header_with_balance)| {
                            // create a new basic output which holds the aggregated balance from unlocked vesting outputs for this address
                            let basic = BasicOutputBuilder::new_with_amount(
                                output_header_with_balance.balance,
                            )
                            .add_unlock_condition(AddressUnlockCondition::new(address))
                            .finish()
                            .expect("should be able to create a basic output");

                            Ok((output_header_with_balance.output_header, basic.into()))
                        })
                        .into_iter(),
                )
                .map(|res| {
                    let (header, mut output) = res?;
                    scale_output_amount_for_iota(&mut output)?;

                    Ok::<_, anyhow::Error>((header, output))
                })
                .process_results(|outputs| migration.run(outputs, object_snapshot_writer))??;
        }
    }

    Ok(())
}

struct OutputHeaderWithBalance {
    output_header: OutputHeader,
    balance: u64,
}

/// returns true if the output is an already unlocked vesting output.
/// it collects the aggregated unlocked balance per address in `unlocked_address_balances`.
fn collect_unlocked_vesting_outputs(
    unlocked_address_balances: &mut HashMap<Address, OutputHeaderWithBalance>,
    snapshot_timestamp_s: u32,
    header: &OutputHeader,
    output: &Output,
) -> bool {
    // ignore all non-basic outputs
    if !output.is_basic() {
        return false;
    }

    // ignore all non vesting outputs
    if !is_vested_reward(header.output_id(), output.as_basic()) {
        return false;
    }

    let unlock_conds = output.unlock_conditions().expect("no unlock conditions found");

    // check if vesting unlock period is already done
    if unlock_conds.is_time_locked(snapshot_timestamp_s) {
        return false;
    }

    let Some(address) = unlock_conds.address() else {
        panic!("no address unlock condition found")
    };

    // collect the unlocked vesting balances
    unlocked_address_balances
        .entry(address.address().clone())
        .and_modify(|x| x.balance = x.balance + output.amount())
        .or_insert(OutputHeaderWithBalance {
            output_header: header.clone(),
            balance: output.amount(),
        });

    true
}

fn scale_output_amount_for_iota(output: &mut Output) -> Result<()> {
    *output = match output {
        Output::Basic(ref basic_output) => {
            // Update amount
            let mut builder = BasicOutputBuilder::from(basic_output)
                .with_amount(scale_amount_for_iota(basic_output.amount())?);

            // Update amount in potential storage deposit return unlock condition
            if let Some(sdr_uc) = basic_output
                .unlock_conditions()
                .get(StorageDepositReturnUnlockCondition::KIND)
            {
                let sdr_uc = sdr_uc.as_storage_deposit_return();
                builder = builder.replace_unlock_condition(
                    StorageDepositReturnUnlockCondition::new(
                        sdr_uc.return_address(),
                        scale_amount_for_iota(sdr_uc.amount())?,
                        u64::MAX,
                    )
                    .unwrap(),
                );
            };

            Output::from(builder.finish()?)
        }
        Output::Alias(ref alias_output) => Output::from(
            AliasOutputBuilder::from(alias_output)
                .with_amount(scale_amount_for_iota(alias_output.amount())?)
                .finish()?,
        ),
        Output::Foundry(ref foundry_output) => Output::from(
            FoundryOutputBuilder::from(foundry_output)
                .with_amount(scale_amount_for_iota(foundry_output.amount())?)
                .finish()?,
        ),
        Output::Nft(ref nft_output) => {
            // Update amount
            let mut builder = NftOutputBuilder::from(nft_output)
                .with_amount(scale_amount_for_iota(nft_output.amount())?);

            // Update amount in potential storage deposit return unlock condition
            if let Some(sdr_uc) = nft_output
                .unlock_conditions()
                .get(StorageDepositReturnUnlockCondition::KIND)
            {
                let sdr_uc = sdr_uc.as_storage_deposit_return();
                builder = builder.replace_unlock_condition(
                    StorageDepositReturnUnlockCondition::new(
                        sdr_uc.return_address(),
                        scale_amount_for_iota(sdr_uc.amount())?,
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

fn scale_amount_for_iota(amount: u64) -> Result<u64> {
    const IOTA_MULTIPLIER: u64 = 1000;

    amount
        .checked_mul(IOTA_MULTIPLIER)
        .ok_or_else(|| anyhow!("overflow multiplying amount {amount} by {IOTA_MULTIPLIER}"))
}
