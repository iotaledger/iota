// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use iota_core::test_utils::make_transfer_iota_transaction;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, ObjectRef},
    crypto::{AccountKeyPair, get_key_pair},
    gas_coin::NANOS_PER_IOTA,
    transaction::Transaction,
};
use rand::seq::IteratorRandom;
use tracing::error;

use crate::{
    ExecutionEffects, ValidatorProxy,
    drivers::Interval,
    system_state_observer::SystemStateObserver,
    workloads::{
        Gas, GasCoinConfig, WorkloadBuilderInfo, WorkloadParams,
        payload::Payload,
        workload::{
            ESTIMATED_COMPUTATION_COST, MAX_GAS_FOR_TESTING, STORAGE_COST_PER_COIN, Workload,
            WorkloadBuilder,
        },
    },
};

#[derive(Debug)]
pub struct DelegationTestPayload {
    coin: Option<ObjectRef>,
    gas: ObjectRef,
    validator: IotaAddress,
    sender: IotaAddress,
    keypair: Arc<AccountKeyPair>,
    system_state_observer: Arc<SystemStateObserver>,
}

impl std::fmt::Display for DelegationTestPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "delegation")
    }
}

impl Payload for DelegationTestPayload {
    fn make_new_payload(&mut self, effects: &ExecutionEffects) {
        if !effects.is_ok() {
            effects.print_gas_summary();
            error!("Delegation tx failed... Status: {:?}", effects.status());
        }

        let coin = match self.coin {
            None => Some(effects.created().first().unwrap().0),
            Some(_) => None,
        };
        self.coin = coin;
        self.gas = effects.gas_object().0;
    }

    /// delegation flow is split into two phases
    /// first `make_transaction` call creates separate coin object for future
    /// delegation followup call creates delegation transaction itself
    fn make_transaction(&mut self) -> Transaction {
        match self.coin {
            Some(coin) => TestTransactionBuilder::new(
                self.sender,
                self.gas,
                self.system_state_observer
                    .state
                    .borrow()
                    .reference_gas_price,
            )
            .call_staking(coin, self.validator)
            .build_and_sign(self.keypair.as_ref()),
            None => make_transfer_iota_transaction(
                self.gas,
                self.sender,
                Some(NANOS_PER_IOTA),
                self.sender,
                &self.keypair,
                self.system_state_observer
                    .state
                    .borrow()
                    .reference_gas_price,
            ),
        }
    }
}

#[derive(Debug)]
pub struct DelegationWorkloadBuilder {
    count: u64,
}

impl DelegationWorkloadBuilder {
    pub fn from(
        workload_weight: f32,
        target_qps: u64,
        num_workers: u64,
        in_flight_ratio: u64,
        duration: Interval,
        group: u32,
    ) -> Option<WorkloadBuilderInfo> {
        let target_qps = (workload_weight * target_qps as f32) as u64;
        let num_workers = (workload_weight * num_workers as f32).ceil() as u64;
        let max_ops = target_qps * in_flight_ratio;
        if max_ops == 0 || num_workers == 0 {
            None
        } else {
            let workload_params = WorkloadParams {
                target_qps,
                num_workers,
                max_ops,
                duration,
                group,
            };
            let workload_builder = Box::<dyn WorkloadBuilder<dyn Payload>>::from(Box::new(
                DelegationWorkloadBuilder { count: max_ops },
            ));
            let builder_info = WorkloadBuilderInfo {
                workload_params,
                workload_builder,
            };
            Some(builder_info)
        }
    }
}

#[async_trait]
impl WorkloadBuilder<dyn Payload> for DelegationWorkloadBuilder {
    async fn generate_coin_config_for_init(&self) -> Vec<GasCoinConfig> {
        vec![]
    }
    async fn generate_coin_config_for_payloads(&self) -> Vec<GasCoinConfig> {
        let amount = MAX_GAS_FOR_TESTING + ESTIMATED_COMPUTATION_COST + STORAGE_COST_PER_COIN;
        (0..self.count)
            .map(|_| {
                let (address, keypair) = get_key_pair();
                GasCoinConfig {
                    amount,
                    address,
                    keypair: Arc::new(keypair),
                }
            })
            .collect()
    }
    async fn build(
        &self,
        _init_gas: Vec<Gas>,
        payload_gas: Vec<Gas>,
    ) -> Box<dyn Workload<dyn Payload>> {
        Box::<dyn Workload<dyn Payload>>::from(Box::new(DelegationWorkload { payload_gas }))
    }
}

#[derive(Debug)]
pub struct DelegationWorkload {
    payload_gas: Vec<Gas>,
}

#[async_trait]
impl Workload<dyn Payload> for DelegationWorkload {
    async fn init(
        &mut self,
        _: Arc<dyn ValidatorProxy + Sync + Send>,
        _system_state_observer: Arc<SystemStateObserver>,
    ) {
    }

    async fn make_test_payloads(
        &self,
        proxy: Arc<dyn ValidatorProxy + Sync + Send>,
        system_state_observer: Arc<SystemStateObserver>,
    ) -> Vec<Box<dyn Payload>> {
        // Here we fetch the committee members from the proxy for profiling.
        let committee_members = proxy
            .get_committee()
            .await
            .expect("failed to fetch validators");

        self.payload_gas
            .iter()
            .map(|(gas, owner, keypair)| {
                let committee_member = *committee_members
                    .iter()
                    .choose(&mut rand::thread_rng())
                    .unwrap();
                Box::new(DelegationTestPayload {
                    coin: None,
                    gas: *gas,
                    validator: committee_member,
                    sender: *owner,
                    keypair: keypair.clone(),
                    system_state_observer: system_state_observer.clone(),
                })
            })
            .map(|b| Box::<dyn Payload>::from(b))
            .collect()
    }
}
