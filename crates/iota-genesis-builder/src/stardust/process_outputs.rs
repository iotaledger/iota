// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use anyhow::{Result, anyhow};
use fastcrypto::{encoding::Hex, hash::HashFunction};
use iota_sdk::types::{
    api::plugins::participation::types::PARTICIPATION_TAG,
    block::{
        address::{Address, Ed25519Address},
        output::{
            AliasOutputBuilder, BasicOutput, BasicOutputBuilder, FoundryOutputBuilder,
            NftOutputBuilder, OUTPUT_INDEX_MAX, Output, OutputId,
            feature::SenderFeature,
            unlock_condition::{AddressUnlockCondition, StorageDepositReturnUnlockCondition},
        },
        payload::transaction::TransactionId,
    },
};
use iota_types::{
    base_types::IotaAddress,
    crypto::DefaultHash,
    timelock::timelock::{VESTED_REWARD_ID_PREFIX, is_vested_reward},
};
use tracing::debug;

use super::types::{
    address_swap_split_map::AddressSwapSplitMap, output_header::OutputHeader,
    output_index::OutputIndex,
};

/// Processes an iterator of outputs coming from a Hornet snapshot chaining some
/// filters:
/// - the `ScaleIotaAmountIterator` scales balances of IOTA Tokens from micro to
///   nano.
/// - the `UnlockedVestingIterator` takes vesting outputs that can be unlocked
///   and merges them into a unique basic output.
/// - the `ParticipationOutputFilter` removes all features from the basic
///   outputs with a participation tag.
/// - the `SwapSplitIterator` performs the operation of SwapSplit given a map as
///   input, i.e., for certain origin addresses it swaps the addressUC to a
///   destination address and splits some amounts of tokens and/or timelocked
///   tokens (this operation can be done for several destinations).
pub fn process_outputs_for_iota<'a>(
    target_milestone_timestamp: u32,
    swap_split_map: AddressSwapSplitMap,
    outputs: impl Iterator<Item = Result<(OutputHeader, Output)>> + 'a,
) -> impl Iterator<Item = Result<(OutputHeader, Output), anyhow::Error>> + 'a {
    // Create the iterator with the filters needed for an IOTA snapshot
    outputs
        .scale_iota_amount()
        .filter_unlocked_vesting_outputs(target_milestone_timestamp)
        .filter_participation_outputs()
        .perform_swap_split(swap_split_map)
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

/// Iterator that modifies some outputs address unlocked by certain origin
/// addresses found in the `swap_split_map`. For each origin address there can
/// be a set of destination addresses. Each destination address as either a
/// tokens target, a tokens timelocked target or both. So, each output found by
/// this filter with an address unlock condition being the origin address, a
/// SwapSplit operation is performed. This operation consists in splitting the
/// output in different outputs given the targets indicated for the destinations
/// and swapping the address unlock condition to be the destination address one.
/// This operation is performed on all basic outputs and (vesting) timelocked
/// basic outputs until the targets are reached.
struct SwapSplitIterator<I> {
    /// Iterator over `(OutputHeader, Output)` pairs.
    outputs: I,
    /// Map used for the SwapSplit operation. It associates an origin address to
    /// a vector of destinations. A destination is a tuple containing a
    /// destination address, a tokens target and a timelocked tokens target.
    swap_split_map: AddressSwapSplitMap,
    /// Basic outputs with timelock unlock conditions. These are candidate
    /// outputs that are kept in ascending order of timestamp and, when the
    /// iteration over all outputs has finished, some of them will be popped
    /// to be picked for the SwapSplit operation.
    timelock_candidates: BTreeSet<TimelockOrderedOutput>,
    /// Basic outputs that have been split during the processing. These can be
    /// either basic outputs or (vesting) timelocked basic outputs that will
    /// be added as new in the ledger, before the migration.
    split_basic_outputs: Vec<(OutputHeader, Output)>,
    num_swapped_basic: u64,
    num_swapped_timelocks: u64,
    num_splits: u64,
}

impl<I> SwapSplitIterator<I> {
    fn new(outputs: I, swap_split_map: AddressSwapSplitMap) -> Self {
        Self {
            outputs,
            swap_split_map,
            timelock_candidates: Default::default(),
            split_basic_outputs: Default::default(),
            num_swapped_basic: 0,
            num_swapped_timelocks: 0,
            num_splits: 0,
        }
    }

    /// Pop an output from `split_basic_outputs`. Since this contains newly
    /// created outputs, there is the need to create a new OutputHeader that
    /// is not in conflict with any other one in the ledger. Use some data
    /// coming from the original output header plus some unique information
    /// about the new output.
    fn get_split_output(&mut self) -> Option<(OutputHeader, Output)> {
        let (original_header, output) = self.split_basic_outputs.pop()?;
        self.num_splits += 1;
        let pos = self.split_basic_outputs.len();

        let (transaction_id, output_index) = if original_header
            .output_id()
            .to_string()
            .starts_with(VESTED_REWARD_ID_PREFIX)
        {
            // If the original basic output is a vesting output, generate the new OutputId
            // as: original-transaction-id|index
            // where index is a unique input
            // index being a number in the range 1 to OUTPUT_INDEX_MAX is safe because
            // vesting output indexes are always 0
            // https://github.com/iotaledger/snapshot-tool-new-supply
            if original_header.output_id().index() != 0 {
                debug!(
                    "Found a vesting output with output index different than 0: {}",
                    original_header.output_id()
                );
            }
            let index = 1 + (pos as u16 % (OUTPUT_INDEX_MAX - 1));
            (
                *original_header.output_id().transaction_id(),
                OutputIndex::new(index).unwrap(),
            )
        } else {
            // Otherwise, generate the new OutputId as:
            // DefaultHash("iota-genesis-outputs"|original-output-id|pos)|index
            // where pos is a unique input
            let index = pos as u16 % OUTPUT_INDEX_MAX;
            let mut hasher = DefaultHash::default();
            hasher.update(b"iota-genesis-outputs");
            hasher.update(original_header.output_id().hash());
            hasher.update(pos.to_le_bytes());
            let hash = hasher.finalize();
            (
                TransactionId::new(hash.into()),
                OutputIndex::new(index).unwrap(),
            )
        };

        Some((
            OutputHeader::new(
                *transaction_id,
                output_index,
                *original_header.block_id(),
                *original_header.ms_index(),
                original_header.ms_timestamp(),
            ),
            output,
        ))
    }
}

impl<I> Iterator for SwapSplitIterator<I>
where
    I: Iterator<Item = Result<(OutputHeader, Output)>>,
{
    type Item = I::Item;

    /// Get the next from the chained self.outputs iterator and apply the
    /// SwapSplit filter if that's the case.
    fn next(&mut self) -> Option<Self::Item> {
        for mut output in self.outputs.by_ref() {
            if let Ok((header, inner)) = &mut output {
                if let Output::Basic(ref basic_output) = inner {
                    let uc = basic_output.unlock_conditions();
                    // Only for outputs with timelock and/or address unlock conditions (and not
                    // holding native tokens) the SwapSplit operation can be performed
                    if uc.storage_deposit_return().is_none()
                        && uc.expiration().is_none()
                        && basic_output.native_tokens().is_empty()
                    {
                        // Now check if the addressUC's address is to swap
                        if let Some(destinations) = self
                            .swap_split_map
                            .get_destination_maybe_mut(uc.address().unwrap().address())
                        {
                            if uc.timelock().is_some() {
                                // If the output has a timelock UC (and it is a vested reward) and
                                // at least one destination requires some timelocked tokens, then
                                // store it as a candidate and continue with the iterator
                                if is_vested_reward(header.output_id(), basic_output)
                                    && destinations.contains_tokens_timelocked_target()
                                {
                                    // Here we store all the timelocked basic outputs we find,
                                    // because we need all the ones owned by the origin address
                                    // sorted by the unlocking timestamp; outside this loop,
                                    // i.e., once all have been collected, we'll start the
                                    // SwapSplit operation in order, starting from the one that
                                    // unlocks later in time.
                                    self.timelock_candidates.insert(TimelockOrderedOutput {
                                        header: header.clone(),
                                        output: inner.clone(),
                                    });
                                    continue;
                                }
                            } else {
                                // If it is just a basic output, try to perform the SwapSplit
                                // operation for several destinations once all tokens targets are
                                // meet.
                                let (original_output_opt, split_outputs) = swap_split_operation(
                                    destinations.iter_by_tokens_target_mut_filtered(),
                                    basic_output,
                                );
                                // If some SwapSplit were performed, their result are basic inputs
                                // stored in split_outputs; so, we save them in
                                // split_basic_outputs to return them later
                                if !split_outputs.is_empty() {
                                    self.num_swapped_basic += 1;
                                }
                                self.split_basic_outputs.extend(
                                    split_outputs
                                        .into_iter()
                                        .map(|output| (header.clone(), output)),
                                );
                                // If there was a remainder, the original output is returned for the
                                // iterator, possibly with a modified amount; else, continue the
                                // loop
                                if let Some(original_output) = original_output_opt {
                                    *inner = original_output;
                                } else {
                                    continue;
                                }
                            };
                        }
                    }
                }
            }
            return Some(output);
        }
        // Now that we are out of the loop we collect the processed outputs from the
        // timelock filter and try to fulfill the target.
        // First, resolve timelocks SwapSplit operations, taking those out from
        // timelocks; the ordered_timelock_candidates is ordered by timestamp
        // and we want to take the latest ones first.
        while let Some(TimelockOrderedOutput { header, output }) =
            self.timelock_candidates.pop_last()
        {
            // We know that all of them are timelocked basic outputs
            let timelocked_basic_output = output.as_basic();
            let uc = timelocked_basic_output.unlock_conditions();
            // Get destination address and mutable timelocked tokens target
            let destinations = self
                .swap_split_map
                .get_destination_maybe_mut(uc.address().unwrap().address())
                .expect("ordered timelock candidates should be part of the swap map");

            // Try to perform the SwapSplit operation for several destinations once all
            // tokens timelocked targets are met
            let (original_output_opt, split_outputs) = swap_split_operation(
                destinations.iter_by_tokens_timelocked_target_mut_filtered(),
                timelocked_basic_output,
            );
            // If some SwapSplit were performed, their result are timelocked basic inputs
            // stored in split_outputs; so, we save them in
            // split_basic_outputs to return them later
            if !split_outputs.is_empty() {
                self.num_swapped_timelocks += 1;
            }
            self.split_basic_outputs.extend(
                split_outputs
                    .into_iter()
                    .map(|output| (header.clone(), output)),
            );
            // If there was a remainder, the original output is returned for the
            // iterator, possibly with a modified amount; otherwise, continue the
            // loop
            if let Some(original_output) = original_output_opt {
                return Some(Ok((header, original_output)));
            } else {
                continue;
            }
        }
        // Second, return all the remaining split outputs generated suring SwapSplit
        // operations
        Some(Ok(self.get_split_output()?))
    }
}

impl<I> Drop for SwapSplitIterator<I> {
    fn drop(&mut self) {
        if let Some((origin, destination, tokens_target, tokens_timelocked_target)) =
            self.swap_split_map.validate_successful_swap_split()
        {
            panic!(
                "For at least one address, the SwapSplit operation was not fully performed. Origin: {}, destination: {}, tokens left: {}, timelocked tokens left: {}",
                origin, destination, tokens_target, tokens_timelocked_target
            )
        }
        debug!(
            "Number of basic outputs used for a SwapSplit (no timelock): {}",
            self.num_swapped_basic
        );
        debug!(
            "Number of timelocked basic outputs used for a SwapSplit: {}",
            self.num_swapped_timelocks
        );
        debug!("Number of outputs created with splits: {}", self.num_splits);
    }
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
                        .expect("failed to create basic output")
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

struct OutputHeaderWithBalance {
    output_header: OutputHeader,
    balance: u64,
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
            }
            return Some(output);
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
            .expect("failed to create basic output");

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
                    .expect("failed to create basic output")
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
    fn perform_swap_split(self, swap_split_map: AddressSwapSplitMap) -> SwapSplitIterator<Self> {
        SwapSplitIterator::new(self, swap_split_map)
    }

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

/// Skip all outputs that are not basic or not vesting. For vesting (basic)
/// outputs, extract and return the address from their address unlock condition.
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

/// SwapSplit operation. Take a `basic_output` and split it until all targets
/// found in the `destinations` are meet. In the meantime, swap the address
/// unlock condition origin address with the destination address. Finally, if
/// the original `basic_output` has some remainder amount, then return it
/// (without swapping its address unlock condition).
fn swap_split_operation<'a>(
    destinations: impl Iterator<Item = (&'a mut IotaAddress, &'a mut u64)>,
    basic_output: &BasicOutput,
) -> (Option<Output>, Vec<Output>) {
    let mut original_output_opt = None;
    let mut split_outputs = vec![];
    let mut original_basic_output_remainder = basic_output.amount();

    // if the addressUC's address is to swap, then it can have several
    // destinations
    for (destination, target) in destinations {
        // break if the basic output was drained already
        if original_basic_output_remainder == 0 {
            break;
        }
        // we need to make sure that we split at most OUTPUT_INDEX_MAX - 1 times
        debug_assert!(
            split_outputs.len() < OUTPUT_INDEX_MAX as usize,
            "Too many swap split operations to perform for a single output"
        );
        // if the target for this destination is less than the basic output remainder,
        // then use it to split the basic output and swap address;
        // otherwise split and swap using the original_basic_output_remainder, and then
        // break the loop.
        let swap_split_amount = original_basic_output_remainder.min(*target);
        split_outputs.push(
            BasicOutputBuilder::from(basic_output)
                .with_amount(swap_split_amount)
                .replace_unlock_condition(AddressUnlockCondition::new(Ed25519Address::new(
                    destination.to_inner(),
                )))
                .finish()
                .expect("failed to create basic output during split")
                .into(),
        );
        *target -= swap_split_amount;
        original_basic_output_remainder -= swap_split_amount;
    }

    // if the basic output remainder is some, it means that all destinations are
    // already covered; so the original basic output can be just kept with (maybe)
    // an adjusted amount.
    if original_basic_output_remainder > 0 {
        original_output_opt = Some(
            BasicOutputBuilder::from(basic_output)
                .with_amount(original_basic_output_remainder)
                .finish()
                .expect("failed to create basic output")
                .into(),
        );
    }
    (original_output_opt, split_outputs)
}

/// Utility struct that defines the ordering between timelocked basic outputs.
/// It is required that the output is a basic outputs with timelock unlock
/// condition.
#[derive(PartialEq, Eq)]
struct TimelockOrderedOutput {
    header: OutputHeader,
    output: Output,
}

impl TimelockOrderedOutput {
    fn get_timestamp(&self) -> u32 {
        self.output
            .as_basic()
            .unlock_conditions()
            .timelock()
            .unwrap()
            .timestamp()
    }
}

impl Ord for TimelockOrderedOutput {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_timestamp()
            .cmp(&other.get_timestamp())
            .then_with(|| self.header.output_id().cmp(&other.header.output_id()))
    }
}
impl PartialOrd for TimelockOrderedOutput {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
