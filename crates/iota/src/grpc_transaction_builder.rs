// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transaction building for the client commands over the gRPC client.
//!
//! [`TransactionBuilderExt`] adds the transaction shapes the client commands
//! need to the SDK [`TransactionBuilder`], mirroring the `*_tx_kind` helpers of
//! the JSON-RPC [`iota_transaction_builder::TransactionBuilder`]. The builder
//! resolves the object inputs itself; finish it with
//! [`finish_kind`](TransactionBuilder::finish_kind), which leaves the gas alone
//! because the client commands select it themselves.

#[cfg(test)]
#[path = "unit_tests/grpc_transaction_builder_tests.rs"]
mod grpc_transaction_builder_tests;

use anyhow::{bail, ensure};
use iota_grpc_client::{Client, ReadMask, read_mask_fields::ObjectField};
use iota_sdk_transaction_builder::{
    TransactionBuilder, TransactionBuilderClient, unresolved::Argument,
};
use iota_sdk_types::{Address, ObjectId, ObjectReference};

/// Resolve `object_ids` to the [`ObjectReference`]s of their current versions,
/// keeping the given order.
///
/// The client commands need these for the gas payment, which they pass to
/// `dry_run_or_execute_or_serialize` rather than to the builder.
pub(crate) async fn input_refs(
    client: &Client,
    object_ids: &[ObjectId],
) -> Result<Vec<ObjectReference>, anyhow::Error> {
    if object_ids.is_empty() {
        return Ok(Vec::new());
    }
    let requests: Vec<_> = object_ids.iter().map(|id| (*id, None)).collect();
    let objects = client
        .get_objects(&requests, Some(ReadMask::from(ObjectField::REFERENCE)))
        .await?
        .into_inner();
    objects
        .iter()
        .map(|object| object.object_reference().map_err(anyhow::Error::from))
        .collect()
}

/// The transaction shapes the client commands build, as an extension of the SDK
/// [`TransactionBuilder`].
pub(crate) trait TransactionBuilderExt {
    /// Merge `input_coins` into the first of them and pay `amounts` to
    /// `recipients` out of it.
    ///
    /// Errors if `input_coins` is empty, or if `recipients` and `amounts` are
    /// empty or differ in length.
    fn pay(
        &mut self,
        input_coins: &[ObjectId],
        recipients: &[Address],
        amounts: &[u64],
    ) -> Result<&mut Self, anyhow::Error>;

    /// Pay `amounts` to `recipients` out of the gas coin.
    ///
    /// Errors if `recipients` and `amounts` are empty or differ in length.
    fn pay_iota(
        &mut self,
        recipients: &[Address],
        amounts: &[u64],
    ) -> Result<&mut Self, anyhow::Error>;

    /// Transfer the whole gas coin to `recipient`.
    fn pay_all_iota(&mut self, recipient: Address) -> &mut Self;
}

impl<C: TransactionBuilderClient> TransactionBuilderExt for TransactionBuilder<C> {
    fn pay(
        &mut self,
        input_coins: &[ObjectId],
        recipients: &[Address],
        amounts: &[u64],
    ) -> Result<&mut Self, anyhow::Error> {
        let coin_args = self.apply_arguments(input_coins.to_vec());
        let Some((&primary_coin, coins_to_merge)) = coin_args.split_first() else {
            bail!("Pay transaction requires a non-empty list of input coins");
        };
        if !coins_to_merge.is_empty() {
            self.merge_coins(primary_coin, coins_to_merge.to_vec());
        }
        pay_from(self, primary_coin, recipients, amounts)
    }

    fn pay_iota(
        &mut self,
        recipients: &[Address],
        amounts: &[u64],
    ) -> Result<&mut Self, anyhow::Error> {
        pay_from(self, Argument::Gas, recipients, amounts)
    }

    fn pay_all_iota(&mut self, recipient: Address) -> &mut Self {
        self.transfer_objects(recipient, vec![Argument::Gas])
    }
}

/// Split `amounts` off `coin` and transfer each split coin to the recipient at
/// the same index. Repeated recipients are transferred to in a single command.
fn pay_from<'b, C: TransactionBuilderClient>(
    builder: &'b mut TransactionBuilder<C>,
    coin: Argument,
    recipients: &[Address],
    amounts: &[u64],
) -> Result<&'b mut TransactionBuilder<C>, anyhow::Error> {
    ensure!(
        recipients.len() == amounts.len(),
        "Found {} recipient addresses, but {} recipient amounts",
        recipients.len(),
        amounts.len(),
    );
    ensure!(!amounts.is_empty(), "No amounts to pay");

    let Argument::Result(split) = builder.split_coins(coin, amounts.to_vec()).arg() else {
        unreachable!("`split_coins` adds a command, so its argument is a command result");
    };

    let mut per_recipient: Vec<(Address, Vec<Argument>)> = Vec::new();
    for (i, recipient) in recipients.iter().enumerate() {
        let split_coin = Argument::NestedResult(split, i as u16);
        match per_recipient.iter_mut().find(|(r, _)| r == recipient) {
            Some((_, coins)) => coins.push(split_coin),
            None => per_recipient.push((*recipient, vec![split_coin])),
        }
    }
    for (recipient, coins) in per_recipient {
        builder.transfer_objects(recipient, coins);
    }
    Ok(builder)
}
