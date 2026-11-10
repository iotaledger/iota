// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use better_any::{Tid, TidAble};
use iota_sdk_types::{Address, ObjectId, TransactionDigest};
use iota_types::{base_types::TxContext, committee::EpochId};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress,
    runtime_value::{MoveStructLayout, MoveTypeLayout},
    vm_status::StatusCode,
};
use move_vm_runtime::{
    execution::values::{GlobalValue, StructRef, Value},
    natives::extensions::NativeExtensionMarker,
};

// TransactionContext is a wrapper around TxContext that is exposed to NativeContextExtensions
// in order to provide transaction context information to Move native functions.
// Holds a Rc<RefCell<TxContext>> to allow for mutation of the TxContext.
#[derive(Tid)]
pub struct TransactionContext {
    cached_digest: Option<GlobalValue>,
    pub(crate) tx_context: Rc<RefCell<TxContext>>,
    test_only: bool,
}

impl NativeExtensionMarker<'_> for TransactionContext {}

impl TransactionContext {
    pub fn new(tx_context: Rc<RefCell<TxContext>>) -> Self {
        Self {
            cached_digest: None,
            tx_context,
            test_only: false,
        }
    }

    pub fn new_for_testing(tx_context: Rc<RefCell<TxContext>>) -> Self {
        Self {
            cached_digest: None,
            tx_context,
            test_only: true,
        }
    }

    pub fn sender(&self) -> Address {
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

    /// Returns a `Value` holding a reference to the transaction digest, as expected by the
    /// `tx_context::native_digest(): &vector<u8>` native. The underlying value is cached so
    /// repeated calls do not re-serialize the digest.
    pub fn digest_ref(&mut self) -> PartialVMResult<Value> {
        if self.cached_digest.is_none() {
            let digest = self.tx_context.borrow().digest();
            // Wrap in a single-field struct so the digest can be borrowed as a field reference.
            let bytes = bcs::to_bytes(&(digest,)).map_err(|err| {
                PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
                    .with_message(format!("Failed to serialize the transaction digest: {err}"))
            })?;
            let layout = MoveTypeLayout::Struct(Box::new(MoveStructLayout(Box::new(vec![
                MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)),
            ]))));
            let value = Value::simple_deserialize(&bytes, &layout).ok_or_else(|| {
                PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
                    .with_message("Failed to deserialize the transaction digest".to_string())
            })?;
            self.cached_digest = Some(GlobalValue::create(value)?);
        }
        self.cached_digest
            .as_ref()
            .ok_or_else(|| PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR))?
            .borrow_global()?
            .value_as::<StructRef>()?
            .borrow_field(0)
    }

    pub fn sponsor(&self) -> Option<Address> {
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

    pub fn fresh_id(&self) -> ObjectId {
        self.tx_context.borrow_mut().fresh_id()
    }

    // Test only function
    //
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
        // Drop the cached digest so it is recreated from the updated TxContext.
        self.cached_digest = None;
        Ok(())
    }
}
