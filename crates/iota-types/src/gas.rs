// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
pub mod checked {

    use std::collections::HashMap;

    use enum_dispatch::enum_dispatch;
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{ObjectReference, gas::GasCostSummary};

    use crate::{
        ObjectId,
        error::{ExecutionError, IotaResult, UserInputError, UserInputResult},
        gas_model::{gas_v1::IotaGasStatus as IotaGasStatusV1, tables::GasStatus},
        object::{MoveStructExt, Object},
        transaction::{InputObjects, ObjectReadResult},
    };

    #[enum_dispatch]
    pub trait IotaGasStatusAPI {
        fn is_unmetered(&self) -> bool;
        fn move_gas_status(&self) -> &GasStatus;
        fn move_gas_status_mut(&mut self) -> &mut GasStatus;
        fn bucketize_computation(&mut self) -> Result<(), ExecutionError>;
        fn summary(&self) -> GasCostSummary;
        fn gas_budget(&self) -> u64;
        fn gas_price(&self) -> u64;
        fn reference_gas_price(&self) -> u64;
        fn storage_gas_units(&self) -> u64;
        fn storage_rebate(&self) -> u64;
        fn unmetered_storage_rebate(&self) -> u64;
        fn gas_used(&self) -> u64;
        fn reset_storage_cost_and_rebate(&mut self);
        fn charge_storage_read(&mut self, size: usize) -> Result<(), ExecutionError>;
        fn charge_publish_package(&mut self, size: usize) -> Result<(), ExecutionError>;
        fn track_storage_mutation(
            &mut self,
            object_id: ObjectId,
            new_size: usize,
            storage_rebate: u64,
        ) -> u64;
        fn charge_storage_and_rebate(&mut self) -> Result<(), ExecutionError>;
        fn adjust_computation_on_out_of_gas(&mut self);
    }

    /// Version aware enum for gas status.
    #[enum_dispatch(IotaGasStatusAPI)]
    #[derive(Debug)]
    pub enum IotaGasStatus {
        V1(IotaGasStatusV1),
    }

    impl IotaGasStatus {
        pub fn new(
            gas_budget: u64,
            gas_price: u64,
            reference_gas_price: u64,
            config: &ProtocolConfig,
        ) -> IotaResult<Self> {
            Self::check_gas_preconditions(gas_price, reference_gas_price, config)?;

            Ok(Self::V1(IotaGasStatusV1::new_with_budget(
                gas_budget,
                gas_price,
                reference_gas_price,
                config,
            )))
        }

        pub fn new_unmetered() -> Self {
            // Always return V1 as unmetered gas status is identical from V1 to V2.
            // This is only used for system transactions which do not pay gas.
            Self::V1(IotaGasStatusV1::new_unmetered())
        }

        // This is the only public API on IotaGasStatus, all other gas related
        // operations should go through `GasCharger`
        pub fn check_gas_balance(
            &self,
            gas_objs: &[&ObjectReadResult],
            gas_budget: u64,
        ) -> UserInputResult {
            match self {
                Self::V1(status) => status.check_gas_balance(gas_objs, gas_budget),
            }
        }

        fn check_gas_preconditions(
            gas_price: u64,
            reference_gas_price: u64,
            config: &ProtocolConfig,
        ) -> IotaResult<()> {
            // Common checks. We may pull them into version specific status as needed, but
            // they are unlikely to change.

            // The gas price must be greater than or equal to the reference gas price.
            if gas_price < reference_gas_price {
                return Err(UserInputError::GasPriceUnderRGP {
                    gas_price,
                    reference_gas_price,
                }
                .into());
            }
            if gas_price > config.max_gas_price() {
                return Err(UserInputError::GasPriceTooHigh {
                    max_gas_price: config.max_gas_price(),
                }
                .into());
            }

            Ok(())
        }
    }

    // Helper functions to deal with gas coins operations.

    pub fn deduct_gas(gas_object: &mut Object, charge_or_rebate: i64) {
        // The object must be a gas coin as we have checked in transaction handle phase.
        let gas_coin = gas_object.data.as_opt_mut_struct().unwrap();
        let balance = gas_coin.get_coin_value_unchecked();
        let new_balance = if charge_or_rebate < 0 {
            balance + (-charge_or_rebate as u64)
        } else {
            assert!(balance >= charge_or_rebate as u64);
            balance - charge_or_rebate as u64
        };
        gas_coin.set_coin_value_unchecked(new_balance)
    }

    pub fn get_gas_balance(gas_object: &Object) -> UserInputResult<u64> {
        if let Some(move_obj) = gas_object.data.as_opt_struct() {
            if !move_obj.struct_tag().is_gas_coin() {
                return Err(UserInputError::InvalidGasObject {
                    object_id: gas_object.id(),
                });
            }
            Ok(move_obj.get_coin_value_unchecked())
        } else {
            Err(UserInputError::InvalidGasObject {
                object_id: gas_object.id(),
            })
        }
    }

    /// Checks that every object `gas` refers to is an address-owned gas coin
    /// present in `input_objects`, and that their combined balance covers
    /// `gas_budget`.
    ///
    /// This is [`IotaGasStatus::check_gas_balance`] without the bounds on the
    /// budget itself, for a simulation that skips the gas checks: it still has
    /// to reject gas the engine cannot take the budget from, because
    /// `GasCharger::smash_gas` treats the input checks as having guaranteed
    /// that and panics rather than returning an error. It deliberately does not
    /// bound the budget, which a simulation leaves to metering, so a caller
    /// probing with a small budget runs out of gas instead of being rejected.
    pub fn check_gas_coins_cover_budget(
        input_objects: &InputObjects,
        gas: &[ObjectReference],
        gas_budget: u64,
    ) -> UserInputResult {
        let objects: HashMap<_, _> = input_objects
            .iter()
            .map(|object| (object.id(), object))
            .collect();

        let mut gas_balance = 0u128;
        for gas_ref in gas {
            let read = objects
                .get(&gas_ref.object_id)
                .ok_or(UserInputError::ObjectNotFound {
                    object_id: gas_ref.object_id,
                    version: Some(gas_ref.version),
                })?;
            // `as_object` returning `None` means the object was deleted, which makes
            // it a shared one, and gas cannot be shared.
            let object = read.as_object().ok_or(UserInputError::MissingGasPayment)?;
            if !object.is_address_owned() {
                return Err(UserInputError::GasObjectNotOwnedObject {
                    owner: object.owner,
                });
            }
            gas_balance += get_gas_balance(object)? as u128;
        }

        if gas_balance < gas_budget as u128 {
            return Err(UserInputError::GasBalanceTooLow {
                gas_balance,
                needed_gas_amount: gas_budget as u128,
            });
        }

        Ok(())
    }
}
