// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::error::ExecutionErrorKind;

use crate::{
    compat::{
        ExecutionErrorTrait, TRANSLATION_PER_COMMAND_BASE_CHARGE,
        TRANSLATION_PER_INPUT_BASE_CHARGE, TRANSLATION_PER_LINKAGE_ENTRY_CHARGE,
        TRANSLATION_PER_LIVE_REFERENCE_CHARGE, TRANSLATION_PER_REFERENCE_NODE_CHARGE,
        TRANSLATION_PER_TYPE_NODE_CHARGE, TRANSLATION_PURE_INPUT_PER_BYTE_CHARGE,
    },
    gas_charger::GasCharger,
    static_programmable_transactions::loading::ast::{DeserializedPackage, PackagePayload},
};

/// The [`TranslationMeter`] is responsible for metering gas usage for various operations
/// during the translation of a transaction. It interacts with and exposes interfaces to the
/// [`GasCharger`] it holds in order to deduct gas based on the operations performed.
pub struct TranslationMeter<'gas> {
    charger: &'gas mut GasCharger,
}

impl<'gas> TranslationMeter<'gas> {
    pub fn new(gas_charger: &'gas mut GasCharger) -> TranslationMeter<'gas> {
        TranslationMeter {
            charger: gas_charger,
        }
    }

    pub fn charge_base_inputs<E: ExecutionErrorTrait>(
        &mut self,
        num_inputs: usize,
    ) -> Result<(), E> {
        let amount = (num_inputs as u64)
            .max(1)
            .saturating_mul(TRANSLATION_PER_INPUT_BASE_CHARGE);
        self.charge(amount)
    }

    pub fn charge_pure_input_bytes<E: ExecutionErrorTrait>(
        &mut self,
        num_bytes: usize,
    ) -> Result<(), E> {
        let amount = (num_bytes as u64)
            .max(1)
            .saturating_mul(TRANSLATION_PURE_INPUT_PER_BYTE_CHARGE);
        self.charge(amount)
    }

    pub fn charge_base_command<E: ExecutionErrorTrait>(
        &mut self,
        num_args: usize,
    ) -> Result<(), E> {
        let amount = (num_args as u64)
            .max(1)
            .saturating_mul(TRANSLATION_PER_COMMAND_BASE_CHARGE);
        self.charge(amount)
    }

    /// Charge gas for loading types based on the number of type nodes loaded.
    /// The cost is calculated as `num_type_nodes * TYPE_LOAD_PER_NODE_MULTIPLIER`.
    /// This function assumes that `num_type_nodes` is non-zero.
    pub fn charge_num_type_nodes<E: ExecutionErrorTrait>(
        &mut self,
        num_type_nodes: u64,
    ) -> Result<(), E> {
        let amount = num_type_nodes
            .max(1)
            .saturating_mul(TRANSLATION_PER_TYPE_NODE_CHARGE);
        self.charge(amount)
    }

    pub fn charge_num_type_references<E: ExecutionErrorTrait>(
        &mut self,
        num_type_references: u64,
    ) -> Result<(), E> {
        let amount = self.reference_cost_formula(num_type_references.max(1))?;
        let amount = amount.saturating_mul(TRANSLATION_PER_REFERENCE_NODE_CHARGE);
        self.charge(amount)
    }

    /// Charge for the references live at a single command, should include ALL live references,
    /// not just those created by the command
    pub fn charge_num_live_references<E: ExecutionErrorTrait>(
        &mut self,
        num_live_references: u64,
    ) -> Result<(), E> {
        let amount = self
            .live_reference_cost_formula(num_live_references)
            .saturating_mul(TRANSLATION_PER_LIVE_REFERENCE_CHARGE);
        if amount == 0 {
            // a command with no live references has no additional cost
            return Ok(());
        }
        self.charge(amount)
    }

    pub fn charge_num_linkage_entries<E: ExecutionErrorTrait>(
        &mut self,
        num_linkage_entries: usize,
    ) -> Result<(), E> {
        if TRANSLATION_PER_LINKAGE_ENTRY_CHARGE == 0 {
            return Ok(());
        }
        let amount = (num_linkage_entries as u64)
            .saturating_mul(TRANSLATION_PER_LINKAGE_ENTRY_CHARGE)
            .max(1);
        self.charge(amount)
    }

    pub fn charge_package_load<E: ExecutionErrorTrait>(
        &mut self,
        payload: &PackagePayload,
    ) -> Result<(), E> {
        match payload {
            PackagePayload::Serialized(_) => {
                // Payload stays serialized; deserialization (and charge) happen at execution time.
                Ok(())
            }
            PackagePayload::Deserialized(DeserializedPackage { total_bytes, .. }) => {
                self.charger.charge_publish_package(*total_bytes)?;
                Ok(())
            }
        }
    }

    // We use a non-linear cost function for type references to account for the increased
    // complexity they introduce. The cost is calculated as:
    // cost = (num_type_references * (num_type_references + 1)) / 2
    //
    // Take &self to access protocol config if needed in the future.
    fn reference_cost_formula<E: ExecutionErrorTrait>(&self, n: u64) -> Result<u64, E> {
        let Some(n_succ) = n.checked_add(1) else {
            invariant_violation!("u64 overflow when calculating type reference cost")
        };
        Ok(n.saturating_mul(n_succ) / 2)
    }

    // Live references are charged with a cubic cost function to account for the increased
    // complexity in the underlying borrow graph:
    // cost = (n * (n + 1) * (n + 2)) / 6
    //
    // Take &self to access protocol config if needed in the future.
    fn live_reference_cost_formula(&self, n: u64) -> u64 {
        let product = n
            .saturating_mul(n.saturating_add(1))
            .saturating_mul(n.saturating_add(2));
        product / 6
    }

    // Charge gas using a point charge mechanism based on the cumulative number of units charged so
    // far.
    fn charge<E: ExecutionErrorTrait>(&mut self, amount: u64) -> Result<(), E> {
        if amount == 0 {
            return Ok(());
        }
        self.charger
            .move_gas_status_mut()
            .deduct_gas(amount.into())
            .map_err(Self::gas_error)
    }

    fn gas_error<T, E>(e: T) -> E
    where
        T: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
        E: ExecutionErrorTrait,
    {
        E::new_with_source(ExecutionErrorKind::InsufficientGas, e)
    }
}
