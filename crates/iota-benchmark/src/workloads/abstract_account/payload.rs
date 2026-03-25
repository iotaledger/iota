// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::{Context, Result};
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::{Authenticator, Signer},
};
use iota_sdk::types::transaction::{
    Argument, CallArg, Command, ObjectArg, ProgrammableTransaction,
};
use iota_types::{
    Identifier,
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::AccountKeyPair,
    move_authenticator::MoveAuthenticator,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{Transaction, TransactionData},
};
use tracing::{debug, error};

use crate::{
    ExecutionEffects,
    system_state_observer::SystemStateObserver,
    workloads::{
        abstract_account::{
            AA_MODULE_NAME, GAS_BUDGET, WORKLOAD_LABEL,
            types::{AuthenticatorKind, TxPayloadObjType},
        },
        payload::Payload,
        workload::ExpectedFailureType,
    },
};

/// ------------------------------
/// Payload
/// ------------------------------
#[derive(Debug)]
pub struct AbstractAccountPayload {
    authenticator: AuthenticatorKind,
    owner: (IotaAddress, Arc<AccountKeyPair>),

    aa_package_id: ObjectID,
    aa_object_id: ObjectID,
    aa_initial_shared_version: SequenceNumber,
    aa_address: IotaAddress,

    gas_coin: ObjectRef,
    pay_coin: ObjectRef,

    recipient: IotaAddress,
    shared_object: Option<ObjectRef>,
    split_amount: u64,
    should_fail: bool,

    tx_payload_obj_type: TxPayloadObjType,

    bench_objects: Vec<ObjectRef>,

    system_state_observer: Arc<SystemStateObserver>,
}

impl std::fmt::Display for AbstractAccountPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{WORKLOAD_LABEL}")
    }
}

impl Payload for AbstractAccountPayload {
    fn make_new_payload(&mut self, effects: &ExecutionEffects) {
        if !effects.is_ok() {
            effects.print_gas_summary();
            error!("[{WORKLOAD_LABEL}] tx failed. Status={}", effects.status());
        }

        // Gas object must always be updated (its version changes).
        self.gas_coin = effects.gas_object().0;

        // Pay coin update: it may be mutated, deleted, or unchanged depending on the
        // tx.
        //  Only applicable for OwnedObject payloads, as SharedObject payloads should
        // not be consuming the pay_coin.
        let pay_id = self.pay_coin.0;

        if let Some((new_ref, _owner)) = effects.mutated().iter().find(|(oref, _)| oref.0 == pay_id)
        {
            self.pay_coin = *new_ref;
        } else {
            // 2) If it was deleted/consumed, do NOT keep using the old ref. If your
            //    ExecutionEffects exposes deleted(), handle it; otherwise log and fail
            //    fast.
            let was_deleted = effects.deleted().iter().any(|oref| oref.0 == pay_id);

            if was_deleted {
                // At this point you need a replacement pay coin strategy:
                // - either ensure the PT does not fully consume pay_coin (recommended),
                // - or re-mint / rotate a fresh pay coin per tx.
                panic!(
                    "[{WORKLOAD_LABEL}] pay_coin was deleted/consumed; cannot reuse it. pay_coin_id={:?}",
                    pay_id
                );
            }

            // 3) Unchanged: tx did not touch pay_coin (common for your 'touch shared
            //    object' tx). Reuse the existing ObjectRef (version did not change).
            debug!(
                "[{WORKLOAD_LABEL}] pay_coin unchanged; reusing existing ref. pay_coin_id={:?}",
                pay_id
            );
        }

        if !self.bench_objects.is_empty() {
            let mutated = effects.mutated();
            for obj in self.bench_objects.iter_mut() {
                if let Some((new_ref, _)) = mutated.iter().find(|(oref, _)| oref.0 == obj.0) {
                    *obj = *new_ref;
                }
            }
        }
    }

    fn make_transaction(&mut self) -> Transaction {
        let gas_price = self
            .system_state_observer
            .state
            .borrow()
            .reference_gas_price;

        let pt = match self.tx_payload_obj_type {
            TxPayloadObjType::OwnedObject => self.build_split_and_transfer_pt(),
            TxPayloadObjType::SharedObject => self.build_touch_shared_object_pt(),
        };

        let tx_data = TransactionData::new_programmable(
            self.aa_address,
            vec![self.gas_coin],
            pt,
            GAS_BUDGET,
            gas_price,
        );

        // Build MoveAuthenticator args and signature
        let self_call_arg = CallArg::Object(ObjectArg::SharedObject {
            id: self.aa_object_id,
            initial_shared_version: self.aa_initial_shared_version,
            mutable: false,
        });

        let auth_args = build_move_auth_args(
            self.authenticator,
            &tx_data,
            &self.owner,
            &self.bench_objects,
            self.should_fail,
        )
        .expect("build_move_auth_args failed");

        let signatures = vec![GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(auth_args, vec![], self_call_arg),
        )];

        Transaction::from_generic_sig_data(tx_data, signatures)
    }

    fn get_failure_type(&self) -> Option<ExpectedFailureType> {
        if self.should_fail {
            Some(ExpectedFailureType::MoveAuthenticatorFailure)
        } else {
            None
        }
    }
}

impl AbstractAccountPayload {
    pub fn new(
        authenticator: AuthenticatorKind,
        owner: (IotaAddress, Arc<AccountKeyPair>),
        aa_package_id: ObjectID,
        aa_object_id: ObjectID,
        aa_initial_shared_version: SequenceNumber,
        aa_address: IotaAddress,
        gas_coin: ObjectRef,
        pay_coin: ObjectRef,
        recipient: IotaAddress,
        shared_object: Option<ObjectRef>,
        split_amount: u64,
        should_fail: bool,
        tx_payload_obj_type: TxPayloadObjType,
        bench_objects: Vec<ObjectRef>,
        system_state_observer: Arc<SystemStateObserver>,
    ) -> Self {
        Self {
            authenticator,
            owner,
            aa_package_id,
            aa_object_id,
            aa_initial_shared_version,
            aa_address,
            gas_coin,
            pay_coin,
            recipient,
            shared_object,
            split_amount,
            should_fail,
            tx_payload_obj_type,
            bench_objects,
            system_state_observer,
        }
    }

    pub fn build_split_and_transfer_pt(&self) -> ProgrammableTransaction {
        {
            let mut builder = ProgrammableTransactionBuilder::new();

            let pay_arg: Argument = builder
                .obj(ObjectArg::ImmOrOwnedObject(self.pay_coin))
                .expect("pt builder: pay coin");

            let amt_arg: Argument = builder
                .pure(self.split_amount)
                .expect("pt builder: split amount");

            let recipient_arg: Argument =
                builder.pure(self.recipient).expect("pt builder: recipient");

            let new_coins = builder.command(Command::SplitCoins(pay_arg, vec![amt_arg]));

            builder.command(Command::TransferObjects(vec![new_coins], recipient_arg));

            builder.finish()
        }
    }

    pub fn build_touch_shared_object_pt(&self) -> ProgrammableTransaction {
        {
            let mut b = ProgrammableTransactionBuilder::new();

            let shared = self.shared_object.unwrap();
            let shared_obj_arg = b
                .obj(ObjectArg::SharedObject {
                    id: shared.0,
                    initial_shared_version: shared.1,
                    mutable: true,
                })
                .unwrap();
            // Move call: iota_system::request_add_stake(state, pay_coin,
            // validator_to_stake_address)
            b.programmable_move_call(
                self.aa_package_id,
                Identifier::new(AA_MODULE_NAME).unwrap(),
                Identifier::new("touch").unwrap(),
                vec![],
                vec![shared_obj_arg],
            );

            b.finish()
        }
    }
}

/// ------------------------------
/// Auth args builder (MoveAuthenticator)
/// ------------------------------
fn build_move_auth_args(
    authenticator: AuthenticatorKind,
    tx_data: &TransactionData,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    bench_objects: &[ObjectRef],
    should_fail: bool,
) -> Result<Vec<CallArg>> {
    let mut auth_args = Vec::new();

    match authenticator {
        AuthenticatorKind::Ed25519 | AuthenticatorKind::Ed25519Heavy => {
            let digest = tx_data.digest().into_inner();
            let sig: Ed25519Signature = owner.1.sign(&digest);

            let mut sig_bytes = sig.as_ref().to_vec();
            if should_fail {
                // Corrupt the signature by flipping a bit, ensuring it remains the same length.
                sig_bytes[0] ^= 0x01;
            }

            let hex_encoded = Hex::encode(sig_bytes)
                .chars()
                .take(Ed25519Signature::LENGTH * 2)
                .collect::<String>();

            auth_args.push(CallArg::Pure(bcs::to_bytes(&hex_encoded)?));
        }

        AuthenticatorKind::HelloWorld => {
            auth_args.push(CallArg::Pure(
                bcs::to_bytes("HelloWorld").context("bcs::to_bytes(HelloWorld)")?,
            ));
        }

        AuthenticatorKind::MaxArgs125 => {
            // These modes assume that bench_objects are already created and belong to the
            // AA address.
            for obj in bench_objects.iter() {
                auth_args.push(CallArg::Object(ObjectArg::ImmOrOwnedObject(*obj)));
            }
        }
    }

    Ok(auth_args)
}
