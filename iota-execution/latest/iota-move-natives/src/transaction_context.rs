// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use better_any::{Tid, TidAble};
use iota_types::{
    base_types::{IotaAddress, ObjectID, TxContext},
    committee::EpochId,
    digests::TransactionDigest,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, runtime_value::MoveTypeLayout, vm_status::StatusCode,
};
use move_vm_runtime::native_extensions::NativeExtensionMarker;
use move_vm_types::values::{GlobalValue, StructRef, Value};

use crate::utils;

// TransactionContext is a wrapper around TxContext that is exposed to
// NativeContextExtensions in order to provide transaction context information
// to Move native functions. Holds a Rc<RefCell<TxContext>> to allow for
// mutation of the TxContext.
#[derive(Tid)]
pub struct TransactionContext {
    pub(crate) tx_context: Rc<RefCell<TxContext>>,
    test_only: bool,

    /// Cached `GlobalValue` containing TxContext data. Caching is used to
    /// avoid redundant conversions and allocations.
    cached_digest: Option<GlobalValue>,
}

impl NativeExtensionMarker<'_> for TransactionContext {}

impl TransactionContext {
    pub fn new(tx_context: Rc<RefCell<TxContext>>) -> Self {
        Self {
            tx_context,
            test_only: false,
            cached_digest: None,
        }
    }

    pub fn new_for_testing(tx_context: Rc<RefCell<TxContext>>) -> Self {
        Self {
            tx_context,
            test_only: true,
            cached_digest: None,
        }
    }

    pub fn sender(&self) -> IotaAddress {
        self.tx_context.borrow().sender()
    }

    pub fn epoch(&self) -> EpochId {
        self.tx_context.borrow().epoch()
    }

    pub fn epoch_timestamp_ms(&self) -> u64 {
        self.tx_context.borrow().epoch_timestamp_ms()
    }

    pub fn digest(&self) -> TransactionDigest {
        self.tx_context.borrow().digest()
    }

    /// Returns a `Value` containing a transaction digest ref.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    pub fn digest_ref(&mut self) -> PartialVMResult<Value> {
        if self.cached_digest.is_none() {
            let tx_context = self.tx_context.borrow();

            // Wrap in a tuple to match the expected Move layout of
            // `struct TxContext {
            //     digest: vector<u8>
            // }`
            let rust_value = (tx_context.digest(),);
            let digest_move_layout = MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8));

            self.cached_digest = Some(utils::to_global_value(&rust_value, digest_move_layout)?.0);
        }

        self.cached_digest
            .as_ref()
            .unwrap()
            .borrow_global()
            .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
            .value_as::<StructRef>()?
            .borrow_field(0)
    }

    pub fn sponsor(&self) -> Option<IotaAddress> {
        self.tx_context.borrow().sponsor()
    }

    pub fn rgp(&self) -> u64 {
        self.tx_context.borrow().rgp()
    }

    pub fn gas_price(&self) -> u64 {
        self.tx_context.borrow().gas_price()
    }

    pub fn gas_budget(&self) -> u64 {
        self.tx_context.borrow().gas_budget()
    }

    pub fn ids_created(&self) -> u64 {
        self.tx_context.borrow().ids_created()
    }

    pub fn fresh_id(&self) -> ObjectID {
        self.tx_context.borrow_mut().fresh_id()
    }

    // Test only function: replace all fields of the wrapped TxContext.
    pub fn replace(
        &mut self,
        sender: AccountAddress,
        tx_hash: Vec<u8>,
        epoch: u64,
        epoch_timestamp_ms: u64,
        ids_created: u64,
        rgp: u64,
        gas_price: u64,
        gas_budget: u64,
        sponsor: Option<AccountAddress>,
    ) -> PartialVMResult<()> {
        if !self.test_only {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("`replace` called on a non testing scenario".to_string()),
            );
        }
        self.tx_context.borrow_mut().replace(
            sender,
            tx_hash,
            epoch,
            epoch_timestamp_ms,
            ids_created,
            rgp,
            gas_price,
            gas_budget,
            sponsor,
        );

        // Drop cached values to ensure they are recreated with the updated TxContext
        // data
        self.cached_digest = None;

        Ok(())
    }
}
