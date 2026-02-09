// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::Arc};

use anyhow::{Context, Result, anyhow, bail, ensure};
use async_trait::async_trait;
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    traits::{Authenticator, Signer},
};
use iota_core::test_utils::make_pay_iota_transaction;
use iota_sdk::types::transaction::{
    Argument, CallArg, Command, ObjectArg, ProgrammableTransaction,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    Identifier,
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, KeypairTraits, get_key_pair},
    move_authenticator::MoveAuthenticator,
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{Transaction, TransactionData},
};
use move_core_types::ident_str;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::{
    ExecutionEffects, ValidatorProxy,
    drivers::Interval,
    system_state_observer::SystemStateObserver,
    workloads::{
        Gas, GasCoinConfig, WorkloadBuilderInfo, WorkloadParams,
        payload::Payload,
        workload::{
            ESTIMATED_COMPUTATION_COST, ExpectedFailureType, MAX_BUDGET, MAX_GAS_FOR_TESTING,
            STORAGE_COST_PER_COIN, Workload, WorkloadBuilder,
        },
    },
};

const GAS_BUDGET: u64 = 1_000_000_000;
const PACKAGE_METADATA_TY: &str = "::package_metadata::PackageMetadataV1";
const UPGRADE_CAP_TY: &str = "::package::UpgradeCap";
const ABSTRACT_ACCOUNT_TY: &str = "::abstract_account::AbstractAccount";
const AA_MODULE_NAME: &str = "abstract_account";

/// For metrics/logging
const WORKLOAD_LABEL: &str = "abstract_account";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum AuthenticatorKind {
    Ed25519,
    Ed25519Heavy,
    HelloWorld,
    MaxArgs128,
}

impl AuthenticatorKind {
    pub fn module_name(&self) -> &'static str {
        AA_MODULE_NAME
    }

    pub fn function_name(&self) -> &'static str {
        match self {
            AuthenticatorKind::Ed25519 => "authenticate_ed25519",
            AuthenticatorKind::Ed25519Heavy => "authenticate_ed25519_heavy",
            AuthenticatorKind::HelloWorld => "authenticate_hello_world",
            AuthenticatorKind::MaxArgs128 => "authenticate_max_args_128",
        }
    }

    pub fn requires_bench_objects(&self) -> bool {
        matches!(self, AuthenticatorKind::MaxArgs128)
    }

    pub fn expected_bench_objects_count(&self) -> Option<usize> {
        match self {
            AuthenticatorKind::MaxArgs128 => Some(125),
            _ => None,
        }
    }
}

impl FromStr for AuthenticatorKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ed25519" => Ok(AuthenticatorKind::Ed25519),
            "ed25519heavy" => Ok(AuthenticatorKind::Ed25519Heavy),
            "helloworld" => Ok(AuthenticatorKind::HelloWorld),
            "maxargs128" => Ok(AuthenticatorKind::MaxArgs128),
            _ => bail!("unknown AuthenticatorKind: {}", s),
        }
    }
}

impl std::fmt::Display for AuthenticatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            AuthenticatorKind::Ed25519 => "ed25519",
            AuthenticatorKind::Ed25519Heavy => "ed25519heavy",
            AuthenticatorKind::HelloWorld => "helloworld",
            AuthenticatorKind::MaxArgs128 => "maxargs128",
        };
        f.write_str(s)
    }
}

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
/// Workload Builder
/// ------------------------------
#[derive(Debug)]
pub struct AbstractAccountWorkloadBuilder {
    authenticator: AuthenticatorKind,
    tx_type: TxType,
    split_amount: u64,
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
        tx_type: TxType,
        split_amount: u64,
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
                tx_type,
                split_amount,
                num_payloads: max_ops,
                owner: (owner_addr, owner_kp),
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
            num_payloads: self.num_payloads,
            owner: self.owner.clone(),
            init_coin: init_gas.into_iter().next(),
            // will be filled in init():
            tx_type: self.tx_type,
            aa_shared_ref: None,
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

/// ------------------------------
/// Workload runtime state
/// ------------------------------
#[derive(Debug)]
pub struct AbstractAccountWorkload {
    authenticator: AuthenticatorKind,
    split_amount: u64,
    num_payloads: u64,
    owner: (IotaAddress, Arc<AccountKeyPair>),

    // Owner coin for initialization (publish/create/mint).
    init_coin: Option<Gas>,

    // AA data filled in init():
    aa_shared_ref: Option<ObjectRef>,
    aa_object_id: Option<ObjectID>,
    aa_initial_shared_version: Option<SequenceNumber>,
    aa_address: Option<IotaAddress>,

    aa_package_id: Option<ObjectID>,
    // Transaction type: owned-object or shared-object in transaction.
    tx_type: TxType,

    // Bench objects for MaxArgs128 (if required).
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
                "[{WORKLOAD_LABEL}] created AA: obj_id={:?}, initial_shared_version={:?}, aa_address={:?}",
                aa_obj_id, aa_initial_shared_version, aa_address
            );

            self.aa_shared_ref = Some(aa_ref);
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
                    125, // for MaxArgs128
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
            } else if self.tx_type == TxType::SharedObject {
                let shared_objects = init_bench_objects(
                    proxy.clone(),
                    &mut init_coin,
                    &self.owner,
                    gas_price,
                    package_id,
                    self.num_payloads,
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

        match self.tx_type {
            TxType::OwnedObject => {
                self.coin_pairs
                    .iter()
                    .map(|(gas_coin, pay_coin)| {
                        Box::new(AbstractAccountPayload {
                            authenticator: self.authenticator,
                            owner: self.owner.clone(),
                            aa_package_id,
                            aa_object_id,
                            aa_initial_shared_version,
                            aa_address,
                            tx_type: self.tx_type,
                            gas_coin: *gas_coin,
                            pay_coin: *pay_coin,
                            recipient,
                            shared_object: None,
                            split_amount: self.split_amount,
                            bench_objects: self.bench_objects.clone(),
                            system_state_observer: system_state_observer.clone(),
                        })
                    })
                    .map(|p| Box::<dyn Payload>::from(p))
                    .collect()
            }

            TxType::SharedObject => {
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
                        Box::new(AbstractAccountPayload {
                            authenticator: self.authenticator,
                            owner: self.owner.clone(),
                            aa_package_id,
                            aa_object_id,
                            aa_initial_shared_version,
                            aa_address,
                            tx_type: self.tx_type,
                            gas_coin: *gas_coin,
                            pay_coin: *pay_coin,
                            recipient,
                            shared_object: Some(*shared_object),
                            split_amount: self.split_amount,
                            bench_objects: self.bench_objects.clone(),
                            system_state_observer: system_state_observer.clone(),
                        })
                    })
                    .map(|p| Box::<dyn Payload>::from(p))
                    .collect()
            }
        }
    }
}

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

    tx_type: TxType,

    bench_objects: Vec<ObjectRef>,

    system_state_observer: Arc<SystemStateObserver>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum TxType {
    OwnedObject,
    SharedObject,
}

impl FromStr for TxType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owned-object" => Ok(TxType::OwnedObject),
            "shared-object" => Ok(TxType::SharedObject),
            _ => bail!("unknown TxType: {}", s),
        }
    }
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

        let pt = match self.tx_type {
            TxType::OwnedObject => self.built_split_and_transfer_pt(),
            TxType::SharedObject => self.build_touch_shared_object_pt(),
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
        )
        .expect("build_move_auth_args failed");

        let signatures = vec![GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new(auth_args, vec![], self_call_arg),
        )];

        Transaction::from_generic_sig_data(tx_data, signatures)
    }

    fn get_failure_type(&self) -> Option<ExpectedFailureType> {
        None
    }
}

impl AbstractAccountPayload {
    pub fn built_split_and_transfer_pt(&self) -> ProgrammableTransaction {
        let pt = {
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
        };
        pt
    }

    pub fn build_touch_shared_object_pt(&self) -> ProgrammableTransaction {
        let pt = {
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
                self.aa_package_id.into(),
                Identifier::new(AA_MODULE_NAME).unwrap(),
                Identifier::new("touch").unwrap(),
                vec![],
                vec![shared_obj_arg],
            );

            b.finish()
        };
        pt
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
) -> Result<Vec<CallArg>> {
    let mut auth_args = Vec::new();

    match authenticator {
        AuthenticatorKind::Ed25519 | AuthenticatorKind::Ed25519Heavy => {
            let digest = tx_data.digest().into_inner();
            let sig: Ed25519Signature = owner.1.sign(&digest);

            let hex_encoded: String = Hex::encode(sig.as_ref())
                .chars()
                .take(Ed25519Signature::LENGTH * 2)
                .collect();

            auth_args.push(CallArg::Pure(bcs::to_bytes(&hex_encoded)?));
        }

        AuthenticatorKind::HelloWorld => {
            auth_args.push(CallArg::Pure(
                bcs::to_bytes("HelloWorld").context("bcs::to_bytes(HelloWorld)")?,
            ));
        }

        AuthenticatorKind::MaxArgs128 => {
            // These modes assume that bench_objects are already created and belong to the
            // AA address.
            for obj in bench_objects.iter() {
                auth_args.push(CallArg::Object(ObjectArg::ImmOrOwnedObject(*obj)));
            }
        }
    }

    Ok(auth_args)
}

/// ------------------------------
/// AA init helpers
/// ------------------------------

/// Publish AA package and return:
/// - package_id (ObjectID)
/// - package_metadata_ref (ObjectRef) required by abstract_account::create
async fn publish_aa_package_and_find_metadata(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
) -> Result<(ObjectID, ObjectRef)> {
    info!("[{WORKLOAD_LABEL}] publishing Move package: abstract_account");

    let tx = TestTransactionBuilder::new(owner.0, init_coin.0, gas_price)
        .publish_examples(WORKLOAD_LABEL)
        .build_and_sign(owner.1.as_ref());

    let effects = proxy
        .execute_transaction_block(tx)
        .await
        .context("execute publish tx")?;

    ensure!(effects.is_ok(), "publish failed: {}", effects.status());

    // Update init gas ref (publish consumed/mutated it).
    *init_coin = update_gas_from_effects(init_coin, &effects)
        .context("update init gas from publish effects")?;

    let created = effects.created();
    ensure!(
        !created.is_empty(),
        "publish succeeded but effects.created() is empty"
    );

    // Strategy to find package object and PackageMetadataV1:
    // - First, find package ref: either by inspecting object data (Data::Package).
    // - Then, find metadata ref by strict type check == PackageMetadataV1.

    let mut package_ref: Option<ObjectRef> = None;
    let mut metadata_ref: Option<ObjectRef> = None;

    let mut diag: Vec<String> = Vec::new();

    // Helper closure - load object and get printable (ty, is_package).
    async fn describe_created(
        proxy: &Arc<dyn ValidatorProxy + Sync + Send>,
        r: ObjectRef,
        owner: iota_types::object::Owner,
    ) -> Result<(bool, String, String)> {
        let obj = proxy
            .get_object(r.0)
            .await
            .with_context(|| format!("get_object({:?})", r.0))?;

        let (is_package, ty) = match &obj.data {
            iota_types::object::Data::Package(_) => (true, "<package>".to_string()),
            iota_types::object::Data::Move(m) => (false, m.type_().to_string()),
        };

        Ok((
            is_package,
            ty.clone(),
            format!("id={:?} owner={:?} type={}", r.0, owner, ty),
        ))
    }

    // Attempt to find package and metadata among created objects.
    for (r, o) in created.iter().copied() {
        let (is_package, ty, line) = describe_created(&proxy, r, o).await?;

        // We only store diag if we end up failing
        diag.push(line);

        if is_package && package_ref.is_none() {
            package_ref = Some(r);
            continue;
        }

        if !is_package {
            // Ignore UpgradeCap explicitly.
            if ty.contains(UPGRADE_CAP_TY) {
                continue;
            }

            if ty.contains(PACKAGE_METADATA_TY) {
                metadata_ref = Some(r);
            }
        }

        if package_ref.is_some() && metadata_ref.is_some() {
            break;
        }
    }

    let package_ref = package_ref.ok_or_else(|| {
        anyhow!(
            "publish: created package object not found\ncreated objects:\n{}",
            diag.join("\n")
        )
    })?;
    let package_id = package_ref.0;

    let metadata_ref = metadata_ref.ok_or_else(|| {
        anyhow!(
            "publish: PackageMetadataV1 not found among created objects\ncreated objects:\n{}",
            diag.join("\n")
        )
    })?;

    info!(
        "[{WORKLOAD_LABEL}] publish done: package_id={:?}, package_metadata_ref={:?}",
        package_id, metadata_ref
    );

    Ok((package_id, metadata_ref))
}

/// Create AbstractAccount shared object via `abstract_account::create`.
async fn create_abstract_account(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
    aa_package_id: ObjectID,
    aa_package_metadata_ref: ObjectRef,
    authenticator: AuthenticatorKind,
) -> Result<ObjectRef> {
    info!(
        "[{WORKLOAD_LABEL}] creating AbstractAccount via {}::{}::create ...",
        aa_package_id,
        authenticator.module_name()
    );

    let owner_pk = owner.1.public();
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let args = vec![
            builder.obj(ObjectArg::ImmOrOwnedObject(aa_package_metadata_ref))?,
            builder.pure(authenticator.module_name())?,
            builder.pure(authenticator.function_name())?,
            builder.pure(owner_pk.as_ref())?,
        ];

        builder.programmable_move_call(
            aa_package_id,
            Identifier::new(authenticator.module_name())?,
            ident_str!("create").into(),
            vec![],
            args,
        );

        builder.finish()
    };
    let tx_data =
        TransactionData::new_programmable(owner.0, vec![init_coin.0], pt, GAS_BUDGET, gas_price);

    let tx = Transaction::from_data_and_signer(tx_data, vec![owner.1.as_ref()]);
    let effects = proxy
        .execute_transaction_block(tx)
        .await
        .context("execute create AbstractAccount tx")?;

    if !effects.is_ok() {
        effects.print_gas_summary();
        bail!("create AbstractAccount failed");
    }

    *init_coin = update_gas_from_effects(init_coin, &effects)?;

    // Find created aa shared object
    let abstract_account_ref: Vec<ObjectRef> = effects
        .created()
        .into_iter()
        .filter_map(|(r, o)| {
            if matches!(o, Owner::Shared { .. }) {
                Some(r)
            } else {
                None
            }
        })
        .collect();

    if abstract_account_ref.is_empty() {
        bail!("create AbstractAccount: no shared objects created");
    }
    if abstract_account_ref.len() == 1 {
        return Ok(abstract_account_ref[0]);
    }

    for r in abstract_account_ref.iter().copied() {
        let obj = proxy.get_object(r.0).await?;
        let ty = object_type_string(&obj).unwrap_or_default();
        if ty.contains(ABSTRACT_ACCOUNT_TY) {
            return Ok(r);
        }
    }

    Ok(abstract_account_ref[0])
}

/// Mint `count` owned coins (objects) to `recipient` with `amount` each.
/// Returns object refs of minted coins.
async fn mint_owned_coins_to_address(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
    recipient: IotaAddress,
    count: u64,
    amount: u64,
) -> Result<Vec<ObjectRef>> {
    info!(
        "[{WORKLOAD_LABEL}] minting {} coins to AA address {:?}, amount={} each ...",
        count, recipient, amount
    );

    let mut remaining = count as usize;
    let mut minted: Vec<ObjectRef> = Vec::with_capacity(count as usize);

    while remaining > 0 {
        let batch = remaining.min(PAY_CHUNK_SIZE);
        remaining -= batch;

        let recipients: Vec<IotaAddress> = vec![recipient; batch];
        let amounts: Vec<u64> = vec![amount; batch];

        let tx = make_pay_iota_transaction(
            init_coin.0,
            vec![],
            recipients,
            amounts,
            owner.0,
            owner.1.as_ref(),
            gas_price,
            GAS_BUDGET,
        );

        let effects = proxy.execute_transaction_block(tx).await?;

        if !effects.is_ok() {
            effects.print_gas_summary();
            bail!("mint pay tx failed");
        }

        // update init_coin to the mutated ref
        *init_coin = update_gas_from_effects(init_coin, &effects)?;

        for (r, o) in effects.created().into_iter() {
            if matches!(o, Owner::AddressOwner(a) if a == recipient) {
                minted.push(r);
            }
        }
    }

    info!("[{WORKLOAD_LABEL}] minted coins: {}", minted.len());

    Ok(minted)
}

/// Initialize bench objects for MaxArgs authenticators.
async fn init_bench_objects(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
    aa_package_id: ObjectID,
    amount: u64,
) -> Result<Vec<ObjectRef>> {
    let module = ident_str!(AA_MODULE_NAME).to_owned();
    let function = Identifier::new("create_bench_objects")?;

    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let amount_arg: Argument = b.pure(amount)?;
        b.programmable_move_call(aa_package_id, module, function, vec![], vec![amount_arg]);
        b.finish()
    };

    // Take a gas object to pay for this init transaction
    let gas_obj = init_coin.0;

    let gas_budget = 2_000_000_000u64;

    let sender = owner.0;
    let signer = &*owner.1;

    let tx_data =
        TransactionData::new_programmable(sender, vec![gas_obj], pt, gas_budget, gas_price);

    // Sign + execute via proxy
    let tx = Transaction::from_data_and_signer(tx_data, vec![signer]);

    let effects = proxy
        .execute_transaction_block(tx)
        .await
        .context("execute_transaction(create bench objects) failed")?;

    let bench_refs = effects
        .created()
        .into_iter()
        .map(|(r, _adapter)| r)
        .collect::<Vec<_>>();

    ensure!(
        bench_refs.len() == amount as usize,
        "Expected {amount} BenchObject, got {}",
        bench_refs.len()
    );

    *init_coin = update_gas_from_effects(init_coin, &effects)?;

    Ok(bench_refs)
}

/// Update init gas object ref from effects.
fn update_gas_from_effects(current: &Gas, effects: &ExecutionEffects) -> Result<Gas> {
    let updated = effects
        .mutated()
        .into_iter()
        .find(|(r, _)| r.0 == current.0.0)
        .ok_or_else(|| anyhow::anyhow!("init coin not found in mutated effects"))?;

    Ok((updated.0, updated.1.get_owner_address()?, current.2.clone()))
}

/// If the object is not a Move object — returns None.
fn object_type_string(obj: &Object) -> Option<String> {
    obj.type_().map(|t| t.to_string())
}
