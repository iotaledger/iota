// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use fastcrypto::encoding::Hex;
use iota_sdk::types::{
    api::plugins::participation::types::PARTICIPATION_TAG,
    block::{
        address::Address,
        output::{
            AliasOutputBuilder, BasicOutputBuilder, FoundryOutputBuilder, NftOutputBuilder, Output,
            OutputId,
            unlock_condition::{AddressUnlockCondition, StorageDepositReturnUnlockCondition},
        },
    },
};
use iota_types::timelock::timelock::is_vested_reward;
use tracing::info;

use super::types::output_header::OutputHeader;

/// Take an `amount` and scale it by a multiplier defined for the IOTA token.
pub fn scale_amount_for_iota(amount: u64) -> Result<u64> {
    const IOTA_MULTIPLIER: u64 = 1000;

    amount
        .checked_mul(IOTA_MULTIPLIER)
        .ok_or_else(|| anyhow!("overflow multiplying amount {amount} by {IOTA_MULTIPLIER}"))
}

// Check if the output is basic and has a feature Tag using the Participation
// Tag: https://github.com/iota-community/treasury/blob/main/specifications/hornet-participation-plugin.md
pub fn is_participation_output(output: &Output) -> bool {
    if let Some(feat) = output.features() {
        if output.is_basic() && !feat.is_empty() {
            if let Some(tag) = feat.tag() {
                return tag.to_string() == Hex::encode_with_format(PARTICIPATION_TAG);
            };
        }
    };
    false
}

/// Processes outputs from a Hornet snapshot considering 3 filters:
/// - the `ScaleIotaAmountFilter` scales balances of IOTA Tokens from micro to
///   nano
/// - the `UnlockedVestingOutputFilter` takes vesting outputs that can be
///   unlocked and merges them into a unique basic output.
/// - the `ParticipationOutputFilter` removes all features from the basic
///   outputs with a participation tag.
pub fn process_outputs_for_iota<'a>(
    target_milestone_timestamp: u32,
    outputs: impl Iterator<Item = Result<(OutputHeader, Output)>> + 'a,
) -> impl Iterator<Item = Result<(OutputHeader, Output), anyhow::Error>> + 'a {
    // Create the iterator with the filters
    HornetFilterIterator::new(target_milestone_timestamp, outputs).map(|res| {
        let (header, output) = res?;
        Ok((header, output))
    })
}

struct OutputHeaderWithBalance {
    output_header: OutputHeader,
    balance: u64,
}

/// An iterator that processes outputs based on some filters.
struct HornetFilterIterator<I> {
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
    /// Stores aggregated balances for eligible addresses.
    unlocked_address_balances: BTreeMap<Address, OutputHeaderWithBalance>,
    /// Timestamp used to evaluate timelock conditions.
    snapshot_timestamp_s: u32,
    /// Output picked to be merged
    vesting_outputs: Vec<OutputId>,
    voting_outputs: Vec<OutputId>,
    num_vesting_outputs: u64,
    num_scaled_outputs: u64,
}

impl<I> HornetFilterIterator<I>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    fn new(snapshot_timestamp_s: u32, outputs: I) -> Self {
        Self {
            outputs,
            unlocked_address_balances: Default::default(),
            snapshot_timestamp_s,
            vesting_outputs: Default::default(),
            num_vesting_outputs: Default::default(),
            num_scaled_outputs: Default::default(),
            voting_outputs: Default::default(),
        }
    }

    fn print_metrics(&self) {
        info!("Number of scaled outputs: {}", self.num_scaled_outputs);
        info!(
            "Number of vesting outputs before merge: {}",
            self.vesting_outputs.len()
        );
        info!(
            "Number of vesting outputs after merging: {}",
            self.num_vesting_outputs
        );
        info!("Number of voting outputs: {}", self.voting_outputs.len());
        info!("Voting outputs: {:?}", self.voting_outputs);
    }

    /// Get the next filtered output
    fn filter_output(&mut self) -> Option<Result<(OutputHeader, Output)>> {
        // For each output result, try to apply all the filters.
        while let Some(mut output) = self.outputs.by_ref().next() {
            self.scale_iota_amount(&mut output);
            if !self.filter_unlocked_vesting_output(&output) {
                continue;
            }
            self.filter_voting_output(&mut output);
            return Some(output);
        }
        None
    }

    fn scale_iota_amount(&mut self, output: &mut Result<(OutputHeader, Output)>) {
        if let Ok((_, inner)) = output {
            self.num_scaled_outputs += 1;
            match inner {
                Output::Basic(ref basic_output) => {
                    // Update amount
                    let mut builder = BasicOutputBuilder::from(basic_output).with_amount(
                        scale_amount_for_iota(basic_output.amount())
                            .expect("should scale the amount for iota"),
                    );
                    // Update amount in potential storage deposit return unlock condition
                    if let Some(sdr_uc) = basic_output
                        .unlock_conditions()
                        .get(StorageDepositReturnUnlockCondition::KIND)
                    {
                        let sdr_uc = sdr_uc.as_storage_deposit_return();
                        builder = builder.replace_unlock_condition(
                            StorageDepositReturnUnlockCondition::new(
                                sdr_uc.return_address(),
                                scale_amount_for_iota(sdr_uc.amount())
                                    .expect("should scale the amount for iota"),
                                u64::MAX,
                            )
                            .unwrap(),
                        );
                    };
                    *inner = builder
                        .finish()
                        .expect("should be able to create a basic output")
                        .into()
                }
                Output::Alias(ref alias_output) => {
                    *inner = AliasOutputBuilder::from(alias_output)
                        .with_amount(
                            scale_amount_for_iota(alias_output.amount())
                                .expect("should scale the amount for iota"),
                        )
                        .finish()
                        .expect("should be able to create an alias output")
                        .into()
                }
                Output::Foundry(ref foundry_output) => {
                    *inner = FoundryOutputBuilder::from(foundry_output)
                        .with_amount(
                            scale_amount_for_iota(foundry_output.amount())
                                .expect("should scale the amount for iota"),
                        )
                        .finish()
                        .expect("should be able to create a foundry output")
                        .into()
                }
                Output::Nft(ref nft_output) => {
                    // Update amount
                    let mut builder = NftOutputBuilder::from(nft_output).with_amount(
                        scale_amount_for_iota(nft_output.amount())
                            .expect("should scale the amount for iota"),
                    );
                    // Update amount in potential storage deposit return unlock condition
                    if let Some(sdr_uc) = nft_output
                        .unlock_conditions()
                        .get(StorageDepositReturnUnlockCondition::KIND)
                    {
                        let sdr_uc = sdr_uc.as_storage_deposit_return();
                        builder = builder.replace_unlock_condition(
                            StorageDepositReturnUnlockCondition::new(
                                sdr_uc.return_address(),
                                scale_amount_for_iota(sdr_uc.amount())
                                    .expect("should scale the amount for iota"),
                                u64::MAX,
                            )
                            .unwrap(),
                        );
                    };
                    *inner = builder
                        .finish()
                        .expect("should be able to create an nft output")
                        .into();
                }
                Output::Treasury(_) => (),
            }
        }
    }

    fn filter_unlocked_vesting_output(&mut self, output: &Result<(OutputHeader, Output)>) -> bool {
        if let Ok((header, inner)) = output {
            if let Some(address) =
                get_address_if_vesting_output(header, inner, self.snapshot_timestamp_s)
            {
                self.vesting_outputs.push(header.output_id());
                self.unlocked_address_balances
                    .entry(address)
                    .and_modify(|x| x.balance += inner.amount())
                    .or_insert(OutputHeaderWithBalance {
                        output_header: header.clone(),
                        balance: inner.amount(),
                    });
                return false;
            }
        }
        true
    }

    fn filter_voting_output(&mut self, output: &mut Result<(OutputHeader, Output)>) {
        if let Ok((header, inner)) = output {
            if is_participation_output(&inner) {
                self.voting_outputs.push(header.output_id());
                // replace the inner output
                *inner = BasicOutputBuilder::from(inner.as_basic())
                    .clear_features()
                    .finish()
                    .expect("should be able to create a basic output")
                    .into()
            }
        }
    }
}

impl<I> Iterator for HornetFilterIterator<I>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(output) = self.filter_output() {
            return Some(output);
        }

        // Now that we are out of the loop we collect the processed outputs from the
        // filters
        let next = self.unlocked_address_balances.pop_first().map(
            |(address, output_header_with_balance)| {
                self.num_vesting_outputs += 1;
                // create a new basic output which holds the aggregated balance from
                // unlocked vesting outputs for this address
                let basic = BasicOutputBuilder::new_with_amount(output_header_with_balance.balance)
                    .add_unlock_condition(AddressUnlockCondition::new(address))
                    .finish()
                    .expect("should be able to create a basic output");

                Ok((output_header_with_balance.output_header, basic.into()))
            },
        );
        if let Some(processed_output) = next {
            return Some(processed_output);
        }

        // Iteration has finished
        self.print_metrics();
        None
    }
}

// Skip all outputs that are not basic or not vesting. For vesting (basic)
// outputs, extract and return the address from their address unlock condition.
fn get_address_if_vesting_output(
    header: &OutputHeader,
    output: &Output,
    snapshot_timestamp_s: u32,
) -> Option<Address> {
    if !output.is_basic() || !is_vested_reward(header.output_id(), output.as_basic()) {
        // if the output is not basic and a vested reward then skip
        return None;
    }

    output.unlock_conditions().and_then(|uc| {
        if uc.is_time_locked(snapshot_timestamp_s) {
            // if the output would still be time locked at snapshot_timestamp_s then skip
            None
        } else {
            // return the address of a vested output that is or can be unlocked
            uc.address().map(|a| *a.address())
        }
    })
}
