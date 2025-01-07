// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use iota_sdk::types::block::{
    address::Address,
    output::{
        AliasOutputBuilder, BasicOutputBuilder, FoundryOutputBuilder, NftOutputBuilder, Output,
        unlock_condition::{AddressUnlockCondition, StorageDepositReturnUnlockCondition},
    },
};
use iota_types::timelock::timelock::is_vested_reward;

use super::types::output_header::OutputHeader;

/// Take an `amount` and scale it by a multiplier defined for the IOTA token.
pub fn scale_amount_for_iota(amount: u64) -> Result<u64> {
    const IOTA_MULTIPLIER: u64 = 1000;

    amount
        .checked_mul(IOTA_MULTIPLIER)
        .ok_or_else(|| anyhow!("overflow multiplying amount {amount} by {IOTA_MULTIPLIER}"))
}

/// Processes and merges outputs from a Hornet snapshot considering balances as
/// IOTA tokens.
///
/// This function uses the `MergingIterator` to filter and aggregate vesting
/// balances and then scales the output amounts.
pub fn get_merged_outputs_for_iota<'a>(
    target_milestone_timestamp: u32,
    outputs: impl Iterator<Item = Result<(OutputHeader, Output)>> + 'a,
) -> impl Iterator<Item = Result<(OutputHeader, Output), anyhow::Error>> + 'a {
    MergingIterator::new(target_milestone_timestamp, outputs).map(|res| {
        let (header, mut output) = res?;
        // Scale the output amount according to IOTA token multiplier
        scale_output_amount_for_iota(&mut output)?;
        Ok((header, output))
    })
}

struct OutputHeaderWithBalance {
    output_header: OutputHeader,
    balance: u64,
}

/// An iterator that processes outputs, aggregates balances for eligible
/// addresses, and generates new "basic" outputs for unlocked vesting rewards.
///
/// `MergingIterator` filters outputs based on conditions:
/// - Must be "basic" outputs.
/// - Must represent vesting rewards that are timelocked relative to a snapshot
///   timestamp.
///
/// Eligible balances are aggregated into a map, and once all inputs are
/// processed, the iterator produces new outputs consolidating these balances.
///
/// Non-eligible outputs are returned as-is.
struct MergingIterator<I> {
    /// Stores aggregated balances for eligible addresses.
    unlocked_address_balances: BTreeMap<Address, OutputHeaderWithBalance>,
    /// Timestamp used to evaluate timelock conditions.
    snapshot_timestamp_s: u32,
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
}

impl<I> MergingIterator<I> {
    fn new(snapshot_timestamp_s: u32, outputs: I) -> Self {
        Self {
            unlocked_address_balances: Default::default(),
            snapshot_timestamp_s,
            outputs,
        }
    }
}

impl<I: Iterator<Item = Result<(OutputHeader, Output)>>> Iterator for MergingIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        // First process all the outputs, building the unlocked_address_balances map as
        // we go.
        for res in self.outputs.by_ref() {
            if let Ok((header, output)) = res {
                fn mergeable_address(
                    header: &OutputHeader,
                    output: &Output,
                    snapshot_timestamp_s: u32,
                ) -> Option<Address> {
                    // ignore all non-basic outputs and non vesting outputs
                    if !output.is_basic()
                        || !is_vested_reward(header.output_id(), output.as_basic())
                    {
                        return None;
                    }

                    if let Some(unlock_conditions) = output.unlock_conditions() {
                        // check if vesting unlock period is already done
                        if unlock_conditions.is_time_locked(snapshot_timestamp_s) {
                            return None;
                        }
                        unlock_conditions.address().map(|uc| *uc.address())
                    } else {
                        None
                    }
                }

                if let Some(address) =
                    mergeable_address(&header, &output, self.snapshot_timestamp_s)
                {
                    // collect the unlocked vesting balances
                    self.unlocked_address_balances
                        .entry(address)
                        .and_modify(|x| x.balance += output.amount())
                        .or_insert(OutputHeaderWithBalance {
                            output_header: header,
                            balance: output.amount(),
                        });
                    continue;
                } else {
                    return Some(Ok((header, output)));
                }
            } else {
                return Some(res);
            }
        }

        // Now that we are out
        self.unlocked_address_balances
            .pop_first()
            .map(|(address, output_header_with_balance)| {
                // create a new basic output which holds the aggregated balance from
                // unlocked vesting outputs for this address
                let basic = BasicOutputBuilder::new_with_amount(output_header_with_balance.balance)
                    .add_unlock_condition(AddressUnlockCondition::new(address))
                    .finish()
                    .expect("should be able to create a basic output");

                Ok((output_header_with_balance.output_header, basic.into()))
            })
    }
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
