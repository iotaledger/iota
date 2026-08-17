// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::transaction::TransactionEnvelope;

use crate::{
    mock_account::Account,
    tx_generator::{MoveTxGenerator, TxGenerator},
};

/// Interleaves several PTB shapes within one run: each account is assigned
/// one shape, weighted, as a pure function of its address — deterministic
/// across rounds and runs.
pub struct MixedTxGenerator {
    // (cumulative weight, generator), in spec order
    entries: Vec<(u64, MoveTxGenerator)>,
    total_weight: u64,
}

impl MixedTxGenerator {
    pub fn new(weighted: Vec<(u64, MoveTxGenerator)>) -> Self {
        let mut entries = Vec::with_capacity(weighted.len());
        let mut total_weight = 0;
        for (weight, generator) in weighted {
            total_weight += weight;
            entries.push((total_weight, generator));
        }
        assert!(total_weight > 0);
        Self {
            entries,
            total_weight,
        }
    }
}

impl TxGenerator for MixedTxGenerator {
    fn generate_tx(&self, account: Account) -> TransactionEnvelope {
        let bytes: [u8; 8] = account.sender.as_bytes()[..8].try_into().unwrap();
        let pick = u64::from_le_bytes(bytes) % self.total_weight;
        let generator = &self
            .entries
            .iter()
            .find(|(cumulative, _)| pick < *cumulative)
            .expect("pick is below the total weight")
            .1;
        generator.generate_tx(account)
    }

    fn name(&self) -> &'static str {
        "Mixed-Shape TransactionEnvelope Generator"
    }
}
