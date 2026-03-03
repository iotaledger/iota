// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conflicting-transfer benchmark workload.
//!
//! Each payload manages one *contested object* and two participant accounts (A
//! and B) that take turns being the sender.  Every round the current sender
//! submits **two** transactions that both try to move the contested object to
//! the other account — creating an owned-object conflict that the white-flag
//! flow resolves post-consensus by dropping one of them.
//!
//! The workload exercises:
//! * Soft-bundle submission (multiple transactions sent together).
//! * Post-consensus owned-object conflict detection (`white_flag`).
//! * Fast rejection propagation via `dropped_tx_notify_read`.

use std::sync::Arc;

use async_trait::async_trait;
use iota_core::test_utils::make_transfer_object_transaction;
use iota_types::{
    base_types::{IotaAddress, ObjectRef, TransactionDigest},
    crypto::{AccountKeyPair, get_key_pair},
    effects::TransactionEffectsAPI,
    transaction::Transaction,
};
use tracing::warn;

use crate::{
    ExecutionEffects, ValidatorProxy,
    drivers::Interval,
    system_state_observer::SystemStateObserver,
    workloads::{
        Gas, GasCoinConfig, WorkloadBuilderInfo, WorkloadParams,
        payload::{Payload, SoftBundleExecutionResults, SoftBundleTransactionResult},
        workload::{
            ESTIMATED_COMPUTATION_COST, MAX_GAS_FOR_TESTING, STORAGE_COST_PER_COIN, Workload,
            WorkloadBuilder,
        },
    },
};

// ---------------------------------------------------------------------------
// Per-account state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct AccountState {
    address: IotaAddress,
    keypair: Arc<AccountKeyPair>,
    /// Two gas coins — one for each competing transaction.
    gas_coins: [Gas; 2],
}

// ---------------------------------------------------------------------------
// Payload
// ---------------------------------------------------------------------------

/// One unit of work: a pair of conflicting transfer transactions.
#[derive(Debug)]
pub struct ConflictingTransferTestPayload {
    /// The object being contested.  Alternates ownership between `accounts[0]`
    /// and `accounts[1]` every successful round.
    contested_object: ObjectRef,
    /// The two participant accounts.
    accounts: [AccountState; 2],
    /// Index (0 or 1) of the account that currently owns `contested_object`
    /// and therefore acts as the sender this round.
    current_sender_idx: usize,
    /// Digests of the transactions created in the last
    /// [`make_transaction_batch`] call (in order).  Used to match results in
    /// [`handle_batch_results`].
    last_batch_digests: Vec<TransactionDigest>,
    system_state_observer: Arc<SystemStateObserver>,
}

impl Payload for ConflictingTransferTestPayload {
    /// Not used in the batched code path; panics to surface misuse.
    fn make_new_payload(&mut self, _effects: &ExecutionEffects) {
        unimplemented!(
            "ConflictingTransferTestPayload uses handle_batch_results, not make_new_payload"
        );
    }

    /// Not used in the batched code path; returns a dummy single transaction.
    fn make_transaction(&mut self) -> Transaction {
        self.make_transaction_batch()
            .into_iter()
            .next()
            .expect("make_transaction_batch must return at least one tx")
    }

    fn is_batched(&self) -> bool {
        true
    }

    fn make_transaction_batch(&mut self) -> Vec<Transaction> {
        let sender_state = &self.accounts[self.current_sender_idx];
        let recipient_idx = 1 - self.current_sender_idx;
        let recipient = self.accounts[recipient_idx].address;

        let rgp = self
            .system_state_observer
            .state
            .borrow()
            .reference_gas_price;

        // Two transactions that both try to transfer `contested_object`.
        // They differ only in gas coin and gas price so they have distinct digests.
        let tx1 = make_transfer_object_transaction(
            self.contested_object,
            sender_state.gas_coins[0].0, // gas_ref
            sender_state.address,
            &sender_state.keypair,
            recipient,
            rgp,
        );
        let tx2 = make_transfer_object_transaction(
            self.contested_object,
            sender_state.gas_coins[1].0, // different gas coin
            sender_state.address,
            &sender_state.keypair,
            recipient,
            rgp + 1, // slightly higher price → different digest → avoids duplicate-tx error
        );

        self.last_batch_digests = vec![*tx1.digest(), *tx2.digest()];
        vec![tx1, tx2]
    }

    fn handle_batch_results(&mut self, results: &SoftBundleExecutionResults) {
        for (i, expected_digest) in self.last_batch_digests.iter().enumerate() {
            // Find this tx's result in the bundle response.
            let Some((_, tx_result)) = results.results.iter().find(|(d, _)| d == expected_digest)
            else {
                continue;
            };

            if let SoftBundleTransactionResult::Executed(effects) = tx_result {
                // Update the gas coin that was used by the winning tx.
                let (new_gas_ref, _) = effects.gas_object();
                self.accounts[self.current_sender_idx].gas_coins[i].0 = new_gas_ref;

                // Update the contested object ref (it moved to the recipient).
                if let Some((new_obj_ref, _)) = effects
                    .mutated()
                    .iter()
                    .find(|(obj_ref, _)| obj_ref.0 == self.contested_object.0)
                {
                    self.contested_object = *new_obj_ref;
                }

                // Swap sender/recipient for the next round.
                self.current_sender_idx = 1 - self.current_sender_idx;
                return;
            }
        }

        // All transactions were rejected — this is unexpected since white flag
        // should always let exactly one through.
        warn!(
            contested_object = ?self.contested_object.0,
            "All transactions in conflicting-transfer bundle were rejected; \
             state not updated"
        );
    }
}

impl std::fmt::Display for ConflictingTransferTestPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "conflicting_transfer")
    }
}

// ---------------------------------------------------------------------------
// WorkloadBuilder
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ConflictingTransferWorkloadBuilder {
    num_payloads: u64,
}

impl ConflictingTransferWorkloadBuilder {
    pub fn from(
        workload_weight: f32,
        target_qps: u64,
        num_workers: u64,
        in_flight_ratio: u64,
        duration: Interval,
        group: u32,
    ) -> Option<WorkloadBuilderInfo> {
        let target_qps = (workload_weight * target_qps as f32).ceil() as u64;
        let num_workers = (workload_weight * num_workers as f32).ceil() as u64;
        let max_ops = target_qps * in_flight_ratio;
        if max_ops == 0 || num_workers == 0 {
            return None;
        }
        let workload_params = WorkloadParams {
            target_qps,
            num_workers,
            max_ops,
            duration,
            group,
        };
        let workload_builder = Box::<dyn WorkloadBuilder<dyn Payload>>::from(Box::new(
            ConflictingTransferWorkloadBuilder {
                num_payloads: max_ops,
            },
        ));
        Some(WorkloadBuilderInfo {
            workload_params,
            workload_builder,
        })
    }
}

#[async_trait]
impl WorkloadBuilder<dyn Payload> for ConflictingTransferWorkloadBuilder {
    async fn generate_coin_config_for_init(&self) -> Vec<GasCoinConfig> {
        vec![]
    }

    /// Per payload instance we allocate **5** coins:
    /// ```text
    /// [0] contested object  (owned by address A)
    /// [1] gas_A1            (owned by address A)
    /// [2] gas_A2            (owned by address A)
    /// [3] gas_B1            (owned by address B)
    /// [4] gas_B2            (owned by address B)
    /// ```
    async fn generate_coin_config_for_payloads(&self) -> Vec<GasCoinConfig> {
        let gas_amount = MAX_GAS_FOR_TESTING + ESTIMATED_COMPUTATION_COST + STORAGE_COST_PER_COIN;
        let mut configs = Vec::with_capacity(self.num_payloads as usize * 5);

        for _ in 0..self.num_payloads {
            let (addr_a, kp_a) = get_key_pair();
            let kp_a: Arc<AccountKeyPair> = Arc::new(kp_a);
            let (addr_b, kp_b) = get_key_pair();
            let kp_b: Arc<AccountKeyPair> = Arc::new(kp_b);

            // [0] contested object — use a plain gas amount so the bank can fund it.
            configs.push(GasCoinConfig {
                amount: gas_amount,
                address: addr_a,
                keypair: kp_a.clone(),
            });
            // [1] gas_A1
            configs.push(GasCoinConfig {
                amount: gas_amount,
                address: addr_a,
                keypair: kp_a.clone(),
            });
            // [2] gas_A2
            configs.push(GasCoinConfig {
                amount: gas_amount,
                address: addr_a,
                keypair: kp_a,
            });
            // [3] gas_B1
            configs.push(GasCoinConfig {
                amount: gas_amount,
                address: addr_b,
                keypair: kp_b.clone(),
            });
            // [4] gas_B2
            configs.push(GasCoinConfig {
                amount: gas_amount,
                address: addr_b,
                keypair: kp_b,
            });
        }

        configs
    }

    async fn build(
        &self,
        _init_gas: Vec<Gas>,
        payload_gas: Vec<Gas>,
    ) -> Box<dyn Workload<dyn Payload>> {
        Box::<dyn Workload<dyn Payload>>::from(Box::new(ConflictingTransferWorkload {
            num_payloads: self.num_payloads,
            payload_gas,
        }))
    }
}

// ---------------------------------------------------------------------------
// Workload
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ConflictingTransferWorkload {
    num_payloads: u64,
    payload_gas: Vec<Gas>,
}

#[async_trait]
impl Workload<dyn Payload> for ConflictingTransferWorkload {
    async fn init(
        &mut self,
        _proxy: Arc<dyn ValidatorProxy + Sync + Send>,
        _system_state_observer: Arc<SystemStateObserver>,
    ) {
    }

    async fn make_test_payloads(
        &self,
        _proxy: Arc<dyn ValidatorProxy + Sync + Send>,
        system_state_observer: Arc<SystemStateObserver>,
    ) -> Vec<Box<dyn Payload>> {
        // `payload_gas` is a flat Vec with 5 coins per payload instance (see above).
        assert_eq!(
            self.payload_gas.len(),
            self.num_payloads as usize * 5,
            "Expected 5 coins per payload"
        );

        self.payload_gas
            .chunks(5)
            .map(|chunk| {
                let contested_object = chunk[0].0; // ObjectRef
                let addr_a = chunk[0].1;
                let kp_a = chunk[0].2.clone();
                let addr_b = chunk[3].1;
                let kp_b = chunk[3].2.clone();

                let account_a = AccountState {
                    address: addr_a,
                    keypair: kp_a,
                    gas_coins: [chunk[1].clone(), chunk[2].clone()],
                };
                let account_b = AccountState {
                    address: addr_b,
                    keypair: kp_b,
                    gas_coins: [chunk[3].clone(), chunk[4].clone()],
                };

                Box::new(ConflictingTransferTestPayload {
                    contested_object,
                    accounts: [account_a, account_b],
                    current_sender_idx: 0,
                    last_batch_digests: vec![],
                    system_state_observer: system_state_observer.clone(),
                }) as Box<dyn Payload>
            })
            .collect()
    }
}
