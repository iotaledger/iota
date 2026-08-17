// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::transaction::TransactionEnvelope;
pub use mixed_tx_generator::MixedTxGenerator;
pub use move_tx_generator::MoveTxGenerator;
pub use owned_object_create_tx_generator::OwnedObjectCreateTxGenerator;
pub use package_publish_tx_generator::PackagePublishTxGenerator;
pub use root_object_create_tx_generator::RootObjectCreateTxGenerator;
pub use shared_object_create_tx_generator::SharedObjectCreateTxGenerator;

use crate::mock_account::Account;

mod mixed_tx_generator;
mod move_tx_generator;
mod owned_object_create_tx_generator;
mod package_publish_tx_generator;
mod root_object_create_tx_generator;
mod shared_object_create_tx_generator;

pub(crate) trait TxGenerator: Send + Sync {
    /// Given an account that contains a sender address, a private key for that
    /// address, and a list of gas objects owned by this address, generate a
    /// single transaction.
    fn generate_tx(&self, account: Account) -> TransactionEnvelope;

    fn name(&self) -> &'static str;
}
