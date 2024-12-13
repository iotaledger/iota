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
            unlock_condition::{AddressUnlockCondition, StorageDepositReturnUnlockCondition},
        },
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

// Check if the output is basic and has a feature Tag using the Participation
// Tag: https://github.com/iota-community/treasury/blob/main/specifications/hornet-participation-plugin.md
pub fn is_voting_output(output: &Output) -> bool {
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
/// - the `ScaleIotaAmountFilter` scales balances of IOTA Toksns from micro to
///   nano
/// - the `UnlockedVestingOutputFilter` takes vesting outputs that can be
///   unlocked and merges them into a unique basic output.
/// - the `VotingOutputFilter` removes all features from the basic outputs with
///   a participation tag.
pub fn process_outputs_for_iota<'a>(
    target_milestone_timestamp: u32,
    outputs: impl Iterator<Item = Result<(OutputHeader, Output)>> + 'a,
) -> impl Iterator<Item = Result<(OutputHeader, Output), anyhow::Error>> + 'a {
    let scale_amount_filter = Box::new(ScaleIotaAmountFilter::default());
    let vesting_filter = Box::new(UnlockedVestingOutputFilter::new(target_milestone_timestamp));
    let voting_filter = Box::new(VotingOutputFilter::default());

    // Create the iterator with the filters
    HornetFilterIterator::new(
        vec![scale_amount_filter, vesting_filter, voting_filter],
        outputs,
    )
    .map(|res| {
        let (header, output) = res?;
        Ok((header, output))
    })
}

struct OutputHeaderWithBalance {
    output_header: OutputHeader,
    balance: u64,
}

// Define the trait for filtering logic
trait HornetFilter<Item> {
    /// Filters the given output based on custom criteria. It returns Some if
    /// the output is modified in place or left as it is, but nonetheless
    /// finalized. It returns None if the output requires other modifications
    /// before being finalized (e.g., to be merged with other outputs).
    fn process(&mut self, output: Item) -> Option<Item>;
    /// Pop an output processed during the filtering process that was not
    /// finalized.
    fn pop_processed(&mut self) -> Option<Item>;
}

/// An iterator that processes outputs based on some filters.
struct HornetFilterIterator<I, Item>
where
    I: Iterator<Item = Item>,
{
    /// The vector of filters.
    filters: Vec<Box<dyn HornetFilter<Item>>>,
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
}

impl<I, Item> HornetFilterIterator<I, Item>
where
    I: Iterator<Item = Item>,
{
    fn new(filters: Vec<Box<dyn HornetFilter<Item>>>, outputs: I) -> Self {
        Self { filters, outputs }
    }
}

impl<I> Iterator for HornetFilterIterator<I, Result<(OutputHeader, Output)>>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        // For each output result, try to apply all the filters. If a filter returns
        // None, then continue the loop. Else, if all filters have acted, then break the
        // loop and return the output.
        'main: for mut output in self.outputs.by_ref() {
            for filter in &mut self.filters {
                if let Some(res) = filter.process(output) {
                    output = res; // Update output with the result of the filter
                } else {
                    continue 'main;
                }
            }
            return Some(output);
        }

        // Now that we are out of the loop we collect the processed outputs from the
        // filters
        for filter in &mut self.filters {
            if let Some(processed_output) = filter.pop_processed() {
                return Some(processed_output);
            }
        }
        None
    }
}

/// Filter that modifies the amount of IOTA tokens for any output, scaling the
/// amount from micros to nanos. Operates in place during the iteration.
#[derive(Default)]
struct ScaleIotaAmountFilter {}

impl HornetFilter<Result<(OutputHeader, Output)>> for ScaleIotaAmountFilter {
    fn process(
        &mut self,
        output: Result<(OutputHeader, Output)>,
    ) -> Option<Result<(OutputHeader, Output)>> {
        // continue filtering
        Some(output.map(|(header, inner)| {
            (header, match inner {
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
                    builder
                        .finish()
                        .expect("should be able to create a basic output")
                        .into()
                }
                Output::Alias(ref alias_output) => AliasOutputBuilder::from(alias_output)
                    .with_amount(
                        scale_amount_for_iota(alias_output.amount())
                            .expect("should scale the amount for iota"),
                    )
                    .finish()
                    .expect("should be able to create an alias output")
                    .into(),
                Output::Foundry(ref foundry_output) => FoundryOutputBuilder::from(foundry_output)
                    .with_amount(
                        scale_amount_for_iota(foundry_output.amount())
                            .expect("should scale the amount for iota"),
                    )
                    .finish()
                    .expect("should be able to create a foundry output")
                    .into(),
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
                    builder
                        .finish()
                        .expect("should be able to create an nft output")
                        .into()
                }
                Output::Treasury(_) => inner,
            })
        }))
    }

    fn pop_processed(&mut self) -> Option<Result<(OutputHeader, Output)>> {
        None
    }
}

/// Filter that looks for vesting outputs that can be unlocked and stores them
/// durng the iteration. At the end of the iteration it merges all vesting
/// outputs owned by a single address into a unique basic output.
struct UnlockedVestingOutputFilter {
    /// Stores aggregated balances for eligible addresses.
    unlocked_address_balances: BTreeMap<Address, OutputHeaderWithBalance>,
    /// Timestamp used to evaluate timelock conditions.
    snapshot_timestamp_s: u32,
}

impl UnlockedVestingOutputFilter {
    fn new(snapshot_timestamp_s: u32) -> Self {
        Self {
            unlocked_address_balances: Default::default(),
            snapshot_timestamp_s,
        }
    }
}

impl HornetFilter<Result<(OutputHeader, Output)>> for UnlockedVestingOutputFilter {
    fn process(
        &mut self,
        output: Result<(OutputHeader, Output)>,
    ) -> Option<Result<(OutputHeader, Output)>> {
        if let Ok((header, inner)) = output {
            if let Some(address) =
                get_address_if_vesting_output(&header, &inner, self.snapshot_timestamp_s)
            {
                self.unlocked_address_balances
                    .entry(address)
                    .and_modify(|x| x.balance += inner.amount())
                    .or_insert(OutputHeaderWithBalance {
                        output_header: header.clone(),
                        balance: inner.amount(),
                    });
                // stop filtering
                None
            } else {
                // continue filtering
                Some(Ok((header, inner)))
            }
        } else {
            // continue filtering
            Some(output)
        }
    }

    fn pop_processed(&mut self) -> Option<Result<(OutputHeader, Output)>> {
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

/// Filter that looks for basic outputs having a tag being the Participation Tag
/// and removes all features from the basic output. Operates in place during the
/// iteration.
#[derive(Default)]
struct VotingOutputFilter {}

impl HornetFilter<Result<(OutputHeader, Output)>> for VotingOutputFilter {
    fn process(
        &mut self,
        output: Result<(OutputHeader, Output)>,
    ) -> Option<Result<(OutputHeader, Output)>> {
        // continue filtering
        Some(output.map(|(header, inner)| {
            (
                header,
                if is_voting_output(&inner) {
                    // replace the inner output
                    BasicOutputBuilder::from(inner.as_basic())
                        .clear_features()
                        .finish()
                        .expect("should be able to create a basic output")
                        .into()
                } else {
                    // do NOT replace
                    inner
                },
            )
        }))
    }

    fn pop_processed(&mut self) -> Option<Result<(OutputHeader, Output)>> {
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
