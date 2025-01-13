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
            feature::SenderFeature,
            unlock_condition::{AddressUnlockCondition, StorageDepositReturnUnlockCondition},
        },
    },
};
use iota_types::timelock::timelock::is_vested_reward;
use tracing::debug;

use super::types::output_header::OutputHeader;

/// Processes an iterator of outputs coming from a Hornet snapshot chaining 3
/// filters:
/// - the `ScaleIotaAmountIterator` scales balances of IOTA Tokens from micro to
///   nano
/// - the `UnlockedVestingIterator` takes vesting outputs that can be unlocked
///   and merges them into a unique basic output.
/// - the `ParticipationOutputFilter` removes all features from the basic
///   outputs with a participation tag.
pub fn process_outputs_for_iota<'a>(
    target_milestone_timestamp: u32,
    outputs: impl Iterator<Item = Result<(OutputHeader, Output)>> + 'a,
) -> impl Iterator<Item = Result<(OutputHeader, Output), anyhow::Error>> + 'a {
    // Create the iterator with the filters needed for an IOTA snapshot
    outputs
        .scale_iota_amount()
        .filter_unlocked_vesting_outputs(target_milestone_timestamp)
        .filter_participation_outputs()
        .map(|res| {
            let (header, output) = res?;
            Ok((header, output))
        })
}

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

struct OutputHeaderWithBalance {
    output_header: OutputHeader,
    balance: u64,
}

/// Iterator that modifies the amount of IOTA tokens for any output, scaling the
/// amount from micros to nanos.
struct ScaleIotaAmountIterator<I> {
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
    num_scaled_outputs: u64,
}

impl<I> ScaleIotaAmountIterator<I> {
    fn new(outputs: I) -> Self {
        Self {
            outputs,
            num_scaled_outputs: 0,
        }
    }
}

impl<I> Iterator for ScaleIotaAmountIterator<I>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    type Item = I::Item;

    /// Get the next from the chained self.outputs iterator and always apply the
    /// scaling (only an Output::Treasury kind is left out)
    fn next(&mut self) -> Option<Self::Item> {
        let mut output = self.outputs.next()?;
        if let Ok((_, inner)) = &mut output {
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
        Some(output)
    }
}

impl<I> Drop for ScaleIotaAmountIterator<I> {
    fn drop(&mut self) {
        debug!("Number of scaled outputs: {}", self.num_scaled_outputs);
    }
}

/// Filtering iterator that looks for vesting outputs that can be unlocked and
/// stores them during the iteration. At the end of the iteration it merges all
/// vesting outputs owned by a single address into a unique basic output.
struct UnlockedVestingIterator<I> {
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
    /// Stores aggregated balances for eligible addresses.
    unlocked_address_balances: BTreeMap<Address, OutputHeaderWithBalance>,
    /// Timestamp used to evaluate timelock conditions.
    snapshot_timestamp_s: u32,
    /// Output picked to be merged
    vesting_outputs: Vec<OutputId>,
    num_vesting_outputs: u64,
}

impl<I> UnlockedVestingIterator<I> {
    fn new(outputs: I, snapshot_timestamp_s: u32) -> Self {
        Self {
            outputs,
            unlocked_address_balances: Default::default(),
            snapshot_timestamp_s,
            vesting_outputs: Default::default(),
            num_vesting_outputs: Default::default(),
        }
    }
}

impl<I> Iterator for UnlockedVestingIterator<I>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    type Item = I::Item;

    /// Get the next from the chained self.outputs iterator and apply the
    /// processing only if the output is an unlocked vesting one
    fn next(&mut self) -> Option<Self::Item> {
        for output in self.outputs.by_ref() {
            if let Ok((header, inner)) = &output {
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
                    continue;
                }
                return Some(output);
            }
        }
        // Now that we are out of the loop we collect the processed outputs from the
        // filters
        let (address, output_header_with_balance) = self.unlocked_address_balances.pop_first()?;
        self.num_vesting_outputs += 1;
        // create a new basic output which holds the aggregated balance from
        // unlocked vesting outputs for this address
        let basic = BasicOutputBuilder::new_with_amount(output_header_with_balance.balance)
            .add_unlock_condition(AddressUnlockCondition::new(address))
            .finish()
            .expect("should be able to create a basic output");

        Some(Ok((output_header_with_balance.output_header, basic.into())))
    }
}

impl<I> Drop for UnlockedVestingIterator<I> {
    fn drop(&mut self) {
        debug!(
            "Number of vesting outputs before merge: {}",
            self.vesting_outputs.len()
        );
        debug!(
            "Number of vesting outputs after merging: {}",
            self.num_vesting_outputs
        );
    }
}

/// Iterator that looks for basic outputs having a tag being the Participation
/// Tag and removes all features from the basic output.
struct ParticipationOutputIterator<I> {
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
    participation_outputs: Vec<OutputId>,
}

impl<I> ParticipationOutputIterator<I> {
    fn new(outputs: I) -> Self {
        Self {
            outputs,
            participation_outputs: Default::default(),
        }
    }
}

impl<I> Iterator for ParticipationOutputIterator<I>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    type Item = I::Item;

    /// Get the next from the chained self.outputs iterator and apply the
    /// processing only if the output has a participation tag
    fn next(&mut self) -> Option<Self::Item> {
        let mut output = self.outputs.next()?;
        if let Ok((header, inner)) = &mut output {
            if is_participation_output(inner) {
                self.participation_outputs.push(header.output_id());
                let basic_output = inner.as_basic();
                // replace the inner output
                *inner = BasicOutputBuilder::from(basic_output)
                    .with_features(
                        vec![basic_output.features().get(SenderFeature::KIND).cloned()]
                            .into_iter()
                            .flatten(),
                    )
                    .finish()
                    .expect("should be able to create a basic output")
                    .into()
            }
        }
        Some(output)
    }
}

impl<I> Drop for ParticipationOutputIterator<I> {
    fn drop(&mut self) {
        debug!(
            "Number of participation outputs: {}",
            self.participation_outputs.len()
        );
        debug!("Participation outputs: {:?}", self.participation_outputs);
    }
}

/// Extension trait that provides convenient methods for chaining and filtering
/// iterator operations.
///
/// The iterators produced by this trait are designed to chain such that,
/// calling `next()` on the last iterator will recursively invoke `next()` on
/// the preceding iterators, maintaining the expected behavior.
trait IteratorExt: Iterator<Item = Result<(OutputHeader, Output)>> + Sized {
    fn scale_iota_amount(self) -> ScaleIotaAmountIterator<Self> {
        ScaleIotaAmountIterator::new(self)
    }

    fn filter_unlocked_vesting_outputs(
        self,
        snapshot_timestamp_s: u32,
    ) -> UnlockedVestingIterator<Self> {
        UnlockedVestingIterator::new(self, snapshot_timestamp_s)
    }

    fn filter_participation_outputs(self) -> ParticipationOutputIterator<Self> {
        ParticipationOutputIterator::new(self)
    }
}
impl<T: Iterator<Item = Result<(OutputHeader, Output)>>> IteratorExt for T {}

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
