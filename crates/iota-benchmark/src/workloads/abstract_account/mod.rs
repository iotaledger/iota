// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod payload;
pub mod types;
pub mod utils;

use std::sync::Arc;

use async_trait::async_trait;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, get_key_pair},
};
pub use payload::AbstractAccountPayload;
use tracing::info;
pub use types::{AuthenticatorKind, TxPayloadObjType};

use crate::{
    ValidatorProxy,
    drivers::Interval,
    system_state_observer::SystemStateObserver,
    workloads::{
        ESTIMATED_COMPUTATION_COST, Gas, GasCoinConfig, MAX_BUDGET, MAX_GAS_FOR_TESTING,
        STORAGE_COST_PER_COIN, WorkloadBuilderInfo, WorkloadParams,
        abstract_account::utils::{
            create_abstract_account, init_bench_objects, mint_owned_coins_to_address,
            publish_aa_package_and_find_metadata,
        },
        payload::Payload,
        workload::{Workload, WorkloadBuilder},
    },
};

const GAS_BUDGET: u64 = 1_000_000_000;
const ABSTRACT_ACCOUNT_TY: &str = "::abstract_account::AbstractAccount";
const AA_MODULE_NAME: &str = "abstract_account";

/// For metrics/logging
const WORKLOAD_LABEL: &str = "abstract_account";
const WORKLOAD_PATH: &str = "abstract_account_for_benchmarks";

/// Each payload uses two coins: one for gas and one as the pay coin.
fn payload_coin_pairs_needed(num_payloads: u64) -> u64 {
    2 * num_payloads
}

/// How many nano-iota How many “nano-IOTA” per created coin for AA.
/// Important: this is NOT the gas budget, but the coin balance.
fn per_coin_amount_estimate() -> u64 {
    // Approximately similar to TransferObject workload:
    // - MAX_GAS_FOR_TESTING — upper estimate to prevent coin depletion.
    // - STORAGE_COST_PER_COIN — if objects are created/mutated
    // - ESTIMATED_COMPUTATION_COST — estimate of computation cost
    MAX_GAS_FOR_TESTING + ESTIMATED_COMPUTATION_COST + (STORAGE_COST_PER_COIN * 2)
}

/// Buffer for publish/create/init (conservative).
fn init_buffer_budget() -> u64 {
    // 10 transactions at MAX_BUDGET — rough “buffer”.
    10 * MAX_BUDGET
}

/// How many coins are created per one pay-transaction.
/// This is a local limit to avoid creating huge lists of recipients/amounts.
const PAY_CHUNK_SIZE: usize = 250;

/// ------------------------------
/// Workload runtime state
/// ------------------------------
#[derive(Debug)]
pub struct AbstractAccountWorkload {
    authenticator: AuthenticatorKind,
    split_amount: u64,
    should_fail: bool,
    num_payloads: u64,
    owner: (IotaAddress, Arc<AccountKeyPair>),

    // Owner coin for initialization (publish/create/mint).
    init_coin: Option<Gas>,

    // AA data filled in init():
    aa_object_id: Option<ObjectID>,
    aa_initial_shared_version: Option<SequenceNumber>,
    aa_address: Option<IotaAddress>,

    aa_package_id: Option<ObjectID>,
    // Transaction type: owned-object or shared-object in transaction.
    tx_payload_obj_type: TxPayloadObjType,

    // Bench objects for MaxArgs125 (if required).
    bench_objects: Vec<ObjectRef>,

    shared_objects: Vec<ObjectRef>,
    // (gas_coin_ref, pay_coin_ref) owned by aa_address.
    coin_pairs: Vec<(ObjectRef, ObjectRef)>,

    // Needed inside payload for gas_price.
    system_state_observer: Option<Arc<SystemStateObserver>>,
}

#[async_trait]
impl Workload<dyn Payload> for AbstractAccountWorkload {
    async fn init(
        &mut self,
        proxy: Arc<dyn ValidatorProxy + Sync + Send>,
        system_state_observer: Arc<SystemStateObserver>,
    ) {
        self.system_state_observer = Some(system_state_observer.clone());

        let gas_price = system_state_observer.state.borrow().reference_gas_price;

        let mut init_coin = self
            .init_coin
            .take()
            .expect("AbstractAccountWorkload: init_coin missing");

        info!(
            "[{WORKLOAD_LABEL}] init start: publish package='abstract_account', authenticator={:?}, num_payloads={}",
            self.authenticator, self.num_payloads
        );

        // 1) Publish AA package
        let res = publish_aa_package_and_find_metadata(
            proxy.clone(),
            &mut init_coin,
            &self.owner,
            gas_price,
        )
        .await;

        if let Err(e) = res {
            eprintln!("publish_aa_package_and_find_metadata error chain: {:#}", e);
            panic!("publish_aa_package_and_find_metadata failed: {e:?}");
        } else if let Ok((package_id, package_metadata_ref)) = res {
            info!(
                "[{WORKLOAD_LABEL}] published AA package: id={:?}, metadata_ref={:?}",
                package_id, package_metadata_ref
            );
            self.aa_package_id = Some(package_id);
            // 2) Create AbstractAccount (shared object)
            let aa_ref = create_abstract_account(
                proxy.clone(),
                &mut init_coin,
                &self.owner,
                gas_price,
                package_id,
                package_metadata_ref,
                self.authenticator,
            )
            .await
            .expect("create_abstract_account failed");

            let aa_obj_id = aa_ref.0;
            let aa_initial_shared_version = aa_ref.1;
            let aa_address: IotaAddress = aa_obj_id.into();

            info!(
                "[{WORKLOAD_LABEL}] created AA: \n obj_id= {:?}, \n initial_shared_version={:?}, \n aa_address={:?}",
                aa_obj_id, aa_initial_shared_version, aa_address
            );

            self.aa_object_id = Some(aa_obj_id);
            self.aa_initial_shared_version = Some(aa_initial_shared_version);
            self.aa_address = Some(aa_address);

            // 3) (Optional) prepare bench_objects for MaxArgs.
            if self.authenticator.requires_bench_objects() {
                let objs = init_bench_objects(
                    proxy.clone(),
                    &mut init_coin,
                    &self.owner,
                    gas_price,
                    package_id,
                    122, // for MaxArgs125
                    false,
                )
                .await
                .expect("init_bench_objects failed");

                if let Some(expected) = self.authenticator.expected_bench_objects_count() {
                    if objs.len() < expected {
                        panic!(
                            "MaxArgs requires at least {} bench objects, got {}",
                            expected,
                            objs.len()
                        );
                    }
                }

                info!(
                    "[{WORKLOAD_LABEL}] prepared bench_objects: count={}",
                    objs.len()
                );
                self.bench_objects = objs;
            }
            if self.tx_payload_obj_type == TxPayloadObjType::SharedObject {
                let shared_objects = init_bench_objects(
                    proxy.clone(),
                    &mut init_coin,
                    &self.owner,
                    gas_price,
                    package_id,
                    self.num_payloads,
                    true, // shared objects for payloads
                )
                .await
                .expect("init_shared_objects failed");

                info!(
                    "[{WORKLOAD_LABEL}] prepared shared_objects: count={}",
                    shared_objects.len()
                );

                self.shared_objects = shared_objects;
            }

            // 4) Mint owned coins to AA address for payload pool.
            // Need 2*N coins: N gas, N pay.
            let needed = payload_coin_pairs_needed(self.num_payloads);
            let per_coin = per_coin_amount_estimate();

            let minted = mint_owned_coins_to_address(
                proxy.clone(),
                &mut init_coin,
                &self.owner,
                gas_price,
                aa_address,
                needed,
                per_coin,
            )
            .await
            .expect("mint_owned_coins_to_address failed");

            if minted.len() as u64 != needed {
                panic!(
                    "expected to mint {} coins to AA, got {}",
                    needed,
                    minted.len()
                );
            }

            let (gas_coins, pay_coins) = minted.split_at(self.num_payloads as usize);
            let coin_pairs: Vec<(ObjectRef, ObjectRef)> = gas_coins
                .iter()
                .copied()
                .zip(pay_coins.iter().copied())
                .collect();

            self.coin_pairs = coin_pairs;

            info!("[{WORKLOAD_LABEL}] init done");
        }
    }

    async fn make_test_payloads(
        &self,
        _proxy: Arc<dyn ValidatorProxy + Sync + Send>,
        system_state_observer: Arc<SystemStateObserver>,
    ) -> Vec<Box<dyn Payload>> {
        let aa_object_id = self.aa_object_id.expect("aa_object_id missing");
        let aa_package_id = self.aa_package_id.expect("aa_package_id missing");
        let aa_initial_shared_version = self
            .aa_initial_shared_version
            .expect("aa_initial_shared_version missing");
        let aa_address = self.aa_address.expect("aa_address missing");

        let recipient = get_key_pair::<AccountKeyPair>().0;

        match self.tx_payload_obj_type {
            TxPayloadObjType::OwnedObject => self
                .coin_pairs
                .iter()
                .map(|(gas_coin, pay_coin)| {
                    Box::new(AbstractAccountPayload::new(
                        self.authenticator,
                        self.owner.clone(),
                        aa_package_id,
                        aa_object_id,
                        aa_initial_shared_version,
                        aa_address,
                        *gas_coin,
                        *pay_coin,
                        recipient,
                        None,
                        self.split_amount,
                        self.should_fail,
                        self.tx_payload_obj_type,
                        self.bench_objects.clone(),
                        system_state_observer.clone(),
                    ))
                })
                .map(|p| Box::<dyn Payload>::from(p))
                .collect(),

            TxPayloadObjType::SharedObject => {
                assert!(
                    self.shared_objects.len() >= self.coin_pairs.len(),
                    "shared_objects({}) < coin_pairs({})",
                    self.shared_objects.len(),
                    self.coin_pairs.len(),
                );

                self.coin_pairs
                    .iter()
                    .zip(self.shared_objects.iter())
                    .map(|((gas_coin, pay_coin), shared_object)| {
                        Box::new(AbstractAccountPayload::new(
                            self.authenticator,
                            self.owner.clone(),
                            aa_package_id,
                            aa_object_id,
                            aa_initial_shared_version,
                            aa_address,
                            *gas_coin,
                            *pay_coin,
                            recipient,
                            Some(*shared_object),
                            self.split_amount,
                            self.should_fail,
                            self.tx_payload_obj_type,
                            self.bench_objects.clone(),
                            system_state_observer.clone(),
                        ))
                    })
                    .map(|p| Box::<dyn Payload>::from(p))
                    .collect()
            }
        }
    }
}

/// ------------------------------
/// Workload Builder
/// ------------------------------
#[derive(Debug)]
pub struct AbstractAccountWorkloadBuilder {
    authenticator: AuthenticatorKind,
    tx_payload_obj_type: TxPayloadObjType,
    split_amount: u64,
    should_fail: bool,
    num_payloads: u64,

    // We create a separate “owner” (a regular ed25519 address),
    // which pays for publish/create/mint and signs the auth_args.
    owner: (IotaAddress, Arc<AccountKeyPair>),
}

impl AbstractAccountWorkloadBuilder {
    pub fn from(
        workload_weight: f32,
        target_qps: u64,
        num_workers: u64,
        in_flight_ratio: u64,
        authenticator: AuthenticatorKind,
        tx_payload_obj_type: TxPayloadObjType,
        split_amount: u64,
        should_fail: bool,
        duration: Interval,
        group: u32,
    ) -> Option<WorkloadBuilderInfo> {
        let target_qps = (workload_weight * target_qps as f32).ceil() as u64;
        let num_workers = (workload_weight * num_workers as f32).ceil() as u64;
        let max_ops = match duration {
            Interval::Count(tx_count) => tx_count,
            Interval::Time(_) => std::cmp::max(num_workers * in_flight_ratio, target_qps),
        };

        if max_ops == 0 || num_workers == 0 {
            return None;
        }

        let (owner_addr, owner_kp) = get_key_pair();
        let owner_kp: Arc<AccountKeyPair> = Arc::new(owner_kp);

        let workload_params = WorkloadParams {
            target_qps,
            num_workers,
            max_ops,
            duration,
            group,
        };

        let workload_builder = Box::<dyn WorkloadBuilder<dyn Payload>>::from(Box::new(
            AbstractAccountWorkloadBuilder {
                authenticator,
                tx_payload_obj_type,
                split_amount,
                num_payloads: max_ops,
                owner: (owner_addr, owner_kp),
                should_fail,
            },
        ));

        Some(WorkloadBuilderInfo {
            workload_params,
            workload_builder,
        })
    }
}

#[async_trait]
impl WorkloadBuilder<dyn Payload> for AbstractAccountWorkloadBuilder {
    async fn generate_coin_config_for_init(&self) -> Vec<GasCoinConfig> {
        // We ask the Bank for one large coin to the owner,
        // 1) publish AA package
        // 2) create AbstractAccount
        // 3) mint N owned coins to AA address for payloads

        let num_coins = payload_coin_pairs_needed(self.num_payloads);
        let per_coin = per_coin_amount_estimate();
        let total_for_payload_coins = per_coin.saturating_mul(num_coins);
        let total = total_for_payload_coins.saturating_add(init_buffer_budget());

        vec![GasCoinConfig {
            amount: total,
            address: self.owner.0,
            keypair: self.owner.1.clone(),
        }]
    }

    async fn generate_coin_config_for_payloads(&self) -> Vec<GasCoinConfig> {
        // Payload gas/coins we do NOT request from the Bank.
        // We create owned coins to the AA address in init().
        vec![]
    }

    async fn build(
        &self,
        init_gas: Vec<Gas>,
        _payload_gas: Vec<Gas>,
    ) -> Box<dyn Workload<dyn Payload>> {
        // init_gas must contain exactly one coin for the owner.
        Box::<dyn Workload<dyn Payload>>::from(Box::new(AbstractAccountWorkload {
            authenticator: self.authenticator,
            split_amount: self.split_amount,
            should_fail: self.should_fail,
            num_payloads: self.num_payloads,
            owner: self.owner.clone(),
            init_coin: init_gas.into_iter().next(),
            // will be filled in init():
            tx_payload_obj_type: self.tx_payload_obj_type,
            aa_object_id: None,
            aa_initial_shared_version: None,
            aa_address: None,
            aa_package_id: None,
            bench_objects: vec![],
            shared_objects: vec![],
            coin_pairs: vec![],
            system_state_observer: None,
        }))
    }
}
