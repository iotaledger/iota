// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod bank;
pub mod benchmark_setup;
pub mod drivers;
pub mod embedded_reconfig_observer;
pub mod fullnode_reconfig_observer;
pub mod in_memory_wallet;
pub mod options;
pub mod system_state_observer;
pub mod util;
pub mod workloads;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::bail;
use async_trait::async_trait;
use embedded_reconfig_observer::EmbeddedReconfigObserver;
use fullnode_reconfig_observer::FullNodeReconfigObserver;
use futures::TryStreamExt;
use iota_config::genesis::Genesis;
use iota_core::{
    authority_aggregator::{AuthorityAggregator, AuthorityAggregatorBuilder},
    authority_client::{AuthorityAPI, NetworkAuthorityClient},
    quorum_driver::{
        QuorumDriver, QuorumDriverHandler, QuorumDriverHandlerBuilder, QuorumDriverMetrics,
        reconfig_observer::ReconfigObserver,
    },
};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockEffects,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponseOptions,
};
use iota_sdk::{IotaClient, IotaClientBuilder, PagedFn};
use iota_types::{
    base_types::{
        AuthorityName, ConciseableName, IotaAddress, ObjectID, ObjectRef, SequenceNumber,
    },
    committee::{Committee, EpochId},
    crypto::AuthorityStrongQuorumSignInfo,
    effects::{CertifiedTransactionEffects, TransactionEffectsAPI, TransactionEvents},
    execution_status::ExecutionFailureStatus,
    gas::GasCostSummary,
    gas_coin::GasCoin,
    iota_system_state::{IotaSystemStateTrait, iota_system_state_summary::IotaSystemStateSummary},
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::{QuorumDriverError, QuorumDriverResponse},
    transaction::{Argument, CallArg, SharedObjectRef, Transaction},
};
use prometheus::Registry;
use rand::Rng;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[derive(Debug)]
/// A wrapper on execution results to accommodate different types of
/// responses from LocalValidatorAggregatorProxy and FullNodeProxy
pub enum ExecutionEffects {
    CertifiedTransactionEffects(CertifiedTransactionEffects, TransactionEvents),
    IotaTransactionBlockEffects(IotaTransactionBlockEffects),
}

impl ExecutionEffects {
    pub fn mutated(&self) -> Vec<(ObjectRef, Owner)> {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                certified_effects.data().mutated().to_vec()
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => iota_tx_effects
                .mutated()
                .iter()
                .map(|refe| (refe.reference, refe.owner))
                .collect(),
        }
    }

    pub fn created(&self) -> Vec<(ObjectRef, Owner)> {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                certified_effects.data().created()
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => iota_tx_effects
                .created()
                .iter()
                .map(|refe| (refe.reference, refe.owner))
                .collect(),
        }
    }

    pub fn deleted(&self) -> Vec<ObjectRef> {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                certified_effects.data().deleted().to_vec()
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => {
                iota_tx_effects.deleted().to_vec()
            }
        }
    }

    pub fn quorum_sig(&self) -> Option<&AuthorityStrongQuorumSignInfo> {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                Some(certified_effects.auth_sig())
            }
            ExecutionEffects::IotaTransactionBlockEffects(_) => None,
        }
    }

    pub fn gas_object(&self) -> (ObjectRef, Owner) {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                certified_effects.data().gas_object()
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => {
                let refe = &iota_tx_effects.gas_object();
                (refe.reference, refe.owner)
            }
        }
    }

    pub fn sender(&self) -> IotaAddress {
        *self.gas_object().1.as_address()
    }

    pub fn is_ok(&self) -> bool {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                certified_effects.data().status().is_success()
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => {
                iota_tx_effects.status().is_ok()
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(effects, ..) => {
                match effects.data().status() {
                    iota_types::execution_status::ExecutionStatus::Success => false,
                    iota_types::execution_status::ExecutionStatus::Failure {
                        error:
                            ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestion {
                                ..
                            }
                            | ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
                                ..
                            },
                        ..
                    } => true,
                    _ => false,
                }
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => {
                let status = format!("{}", iota_tx_effects.status());
                status.contains("ExecutionCancelledDueToSharedObjectCongestion")
            }
        }
    }

    pub fn status(&self) -> String {
        match self {
            ExecutionEffects::CertifiedTransactionEffects(certified_effects, ..) => {
                format!("{:#?}", certified_effects.data().status())
            }
            ExecutionEffects::IotaTransactionBlockEffects(iota_tx_effects) => {
                format!("{:#?}", iota_tx_effects.status())
            }
        }
    }

    pub fn gas_cost_summary(&self) -> GasCostSummary {
        match self {
            crate::ExecutionEffects::CertifiedTransactionEffects(a, _) => {
                a.data().gas_cost_summary().clone()
            }
            crate::ExecutionEffects::IotaTransactionBlockEffects(b) => {
                std::convert::Into::<GasCostSummary>::into(b.gas_cost_summary().clone())
            }
        }
    }

    pub fn gas_used(&self) -> u64 {
        self.gas_cost_summary().gas_used()
    }

    pub fn net_gas_used(&self) -> i64 {
        self.gas_cost_summary().net_gas_usage()
    }

    pub fn print_gas_summary(&self) {
        let gas_object = self.gas_object();
        let sender = self.sender();
        let status = self.status();
        let gas_cost_summary = self.gas_cost_summary();
        let gas_used = self.gas_used();
        let net_gas_used = self.net_gas_used();

        info!(
            "Summary:\n\
             Gas Object: {gas_object:?}\n\
             Sender: {sender:?}\n\
             status: {status}\n\
             Gas Cost Summary: {gas_cost_summary:#?}\n\
             Gas Used: {gas_used}\n\
             Net Gas Used: {net_gas_used}"
        );
    }
}

#[async_trait]
pub trait ValidatorProxy {
    async fn get_object(&self, object_id: ObjectID) -> Result<Object, anyhow::Error>;

    async fn get_owned_objects(
        &self,
        account_address: IotaAddress,
    ) -> Result<Vec<(u64, Object)>, anyhow::Error>;

    async fn get_latest_system_state_object(&self)
    -> Result<IotaSystemStateSummary, anyhow::Error>;

    async fn execute_transaction_block(&self, tx: Transaction) -> anyhow::Result<ExecutionEffects>;

    fn clone_committee(&self) -> Arc<Committee>;

    fn get_current_epoch(&self) -> EpochId;

    fn clone_new(&self) -> Box<dyn ValidatorProxy + Send + Sync>;

    /// This crate benchmarks committee performance, such as
    /// transaction execution (`execute_bench_transaction`).
    /// Therefore, we return the committee members here.
    async fn get_committee(&self) -> Result<Vec<IotaAddress>, anyhow::Error>;
}

// TODO: Eventually remove this proxy because we shouldn't rely on validators to
// read objects.
pub struct LocalValidatorAggregatorProxy {
    _qd_handler: QuorumDriverHandler<NetworkAuthorityClient>,
    // Stress client does not verify individual validator signatures since this is very expensive
    qd: Arc<QuorumDriver<NetworkAuthorityClient>>,
    committee: Committee,
    clients: BTreeMap<AuthorityName, NetworkAuthorityClient>,
}

impl LocalValidatorAggregatorProxy {
    pub async fn from_genesis(
        genesis: &Genesis,
        registry: &Registry,
        reconfig_fullnode_rpc_url: Option<&str>,
    ) -> Self {
        let (aggregator, clients) = AuthorityAggregatorBuilder::from_genesis(genesis)
            .with_registry(registry)
            .build_network_clients();
        let committee = genesis.committee().unwrap();

        Self::new_impl(
            aggregator,
            registry,
            reconfig_fullnode_rpc_url,
            clients,
            committee,
        )
        .await
    }

    async fn new_impl(
        aggregator: AuthorityAggregator<NetworkAuthorityClient>,
        registry: &Registry,
        reconfig_fullnode_rpc_url: Option<&str>,
        clients: BTreeMap<AuthorityName, NetworkAuthorityClient>,
        committee: Committee,
    ) -> Self {
        let quorum_driver_metrics = Arc::new(QuorumDriverMetrics::new(registry));
        let (aggregator, reconfig_observer): (
            Arc<_>,
            Arc<dyn ReconfigObserver<NetworkAuthorityClient> + Sync + Send>,
        ) = if let Some(reconfig_fullnode_rpc_url) = reconfig_fullnode_rpc_url {
            info!(
                "Using FullNodeReconfigObserver: {:?}",
                reconfig_fullnode_rpc_url
            );
            let committee_store = aggregator.clone_committee_store();
            let reconfig_observer = Arc::new(
                FullNodeReconfigObserver::new(
                    reconfig_fullnode_rpc_url,
                    committee_store,
                    aggregator.safe_client_metrics_base.clone(),
                    aggregator.metrics.clone(),
                )
                .await,
            );
            (Arc::new(aggregator), reconfig_observer)
        } else {
            info!("Using EmbeddedReconfigObserver");
            let reconfig_observer = Arc::new(EmbeddedReconfigObserver::new());
            // Get the latest committee from config observer
            let aggregator = reconfig_observer
                .get_committee(Arc::new(aggregator))
                .await
                .expect("Failed to get latest committee");
            (aggregator, reconfig_observer)
        };
        let qd_handler_builder =
            QuorumDriverHandlerBuilder::new(aggregator, quorum_driver_metrics.clone())
                .with_reconfig_observer(reconfig_observer.clone());
        let qd_handler = qd_handler_builder.start();
        let qd = qd_handler.clone_quorum_driver();
        Self {
            _qd_handler: qd_handler,
            qd,
            clients,
            committee,
        }
    }
}

#[async_trait]
impl ValidatorProxy for LocalValidatorAggregatorProxy {
    async fn get_object(&self, object_id: ObjectID) -> Result<Object, anyhow::Error> {
        let auth_agg = self.qd.authority_aggregator().load();
        Ok(auth_agg
            .get_latest_object_version_for_testing(object_id)
            .await?)
    }

    async fn get_owned_objects(
        &self,
        _account_address: IotaAddress,
    ) -> Result<Vec<(u64, Object)>, anyhow::Error> {
        unimplemented!("Not available for local proxy");
    }

    async fn get_latest_system_state_object(
        &self,
    ) -> Result<IotaSystemStateSummary, anyhow::Error> {
        let auth_agg = self.qd.authority_aggregator().load();
        Ok(auth_agg
            .get_latest_system_state_object_for_testing()
            .await?
            .into_iota_system_state_summary())
    }

    async fn execute_transaction_block(&self, tx: Transaction) -> anyhow::Result<ExecutionEffects> {
        let tx_digest = *tx.digest();
        let mut retry_cnt = 0;
        while retry_cnt < 3 {
            let ticket = self
                .qd
                .submit_transaction(
                    iota_types::quorum_driver_types::ExecuteTransactionRequestV1 {
                        transaction: tx.clone(),
                        include_events: true,
                        include_input_objects: false,
                        include_output_objects: false,
                        include_auxiliary_data: false,
                    },
                )
                .await?;
            // The ticket only times out when QuorumDriver exceeds the retry times
            match ticket.await {
                Ok(resp) => {
                    let QuorumDriverResponse {
                        effects_cert,
                        events,
                        ..
                    } = resp;
                    return Ok(ExecutionEffects::CertifiedTransactionEffects(
                        effects_cert.into(),
                        events.unwrap_or_default(),
                    ));
                }
                Err(QuorumDriverError::NonRecoverableTransactionError { errors }) => {
                    bail!(QuorumDriverError::NonRecoverableTransactionError { errors });
                }
                Err(err) => {
                    let delay = Duration::from_millis(rand::thread_rng().gen_range(100..1000));
                    warn!(
                        ?tx_digest,
                        retry_cnt,
                        "Transaction failed with err: {:?}. Sleeping for {:?} ...",
                        err,
                        delay,
                    );
                    retry_cnt += 1;
                    sleep(delay).await;
                }
            }
        }
        bail!("Transaction {:?} failed for {retry_cnt} times", tx_digest);
    }

    fn clone_committee(&self) -> Arc<Committee> {
        self.qd.clone_committee()
    }

    fn get_current_epoch(&self) -> EpochId {
        self.qd.current_epoch()
    }

    fn clone_new(&self) -> Box<dyn ValidatorProxy + Send + Sync> {
        let qdh = self._qd_handler.clone_new();
        let qd = qdh.clone_quorum_driver();
        Box::new(Self {
            _qd_handler: qdh,
            qd,
            clients: self.clients.clone(),
            committee: self.committee.clone(),
        })
    }

    async fn get_committee(&self) -> Result<Vec<IotaAddress>, anyhow::Error> {
        Ok(self
            .get_latest_system_state_object()
            .await?
            .iter_committee_members()
            .map(|v| v.iota_address)
            .collect())
    }
}

pub struct FullNodeProxy {
    iota_client: IotaClient,
    committee: Arc<Committee>,
}

impl FullNodeProxy {
    pub async fn from_url(http_url: &str) -> Result<Self, anyhow::Error> {
        // Each request times out after 60s (default value)
        let iota_client = IotaClientBuilder::default()
            .max_concurrent_requests(500_000)
            .build(http_url)
            .await?;

        let resp = iota_client
            .governance_api()
            .get_committee_info(None)
            .await?;
        let epoch = resp.epoch;
        let committee_vec = resp.validators;
        let committee_map = BTreeMap::from_iter(committee_vec);
        let committee =
            Committee::new_for_testing_with_normalized_voting_power(epoch, committee_map);

        Ok(Self {
            iota_client,
            committee: Arc::new(committee),
        })
    }
}

#[async_trait]
impl ValidatorProxy for FullNodeProxy {
    async fn get_object(&self, object_id: ObjectID) -> Result<Object, anyhow::Error> {
        let response = self
            .iota_client
            .read_api()
            .get_object_with_options(object_id, IotaObjectDataOptions::bcs_lossless())
            .await?;

        if let Some(iota_object) = response.data {
            iota_object.try_into()
        } else if let Some(error) = response.error {
            bail!("Error getting object {:?}: {}", object_id, error)
        } else {
            bail!("Object {:?} not found and no error provided", object_id)
        }
    }

    async fn get_owned_objects(
        &self,
        account_address: IotaAddress,
    ) -> Result<Vec<(u64, Object)>, anyhow::Error> {
        let mut stream = PagedFn::stream(async |cursor| {
            self.iota_client
                .read_api()
                .get_owned_objects(
                    account_address,
                    Some(IotaObjectResponseQuery::new_with_options(
                        IotaObjectDataOptions::bcs_lossless(),
                    )),
                    cursor,
                    None,
                )
                .await
        });

        let mut values_objects = Vec::new();

        while let Some(object) = stream.try_next().await? {
            let o = object.data;
            if let Some(o) = o {
                let temp: Object = o.clone().try_into()?;
                let gas_coin = GasCoin::try_from(&temp)?;
                values_objects.push((gas_coin.value(), o.clone().try_into()?));
            }
        }

        Ok(values_objects)
    }

    async fn get_latest_system_state_object(
        &self,
    ) -> Result<IotaSystemStateSummary, anyhow::Error> {
        Ok(self
            .iota_client
            .governance_api()
            .get_latest_iota_system_state()
            .await?)
    }

    async fn execute_transaction_block(&self, tx: Transaction) -> anyhow::Result<ExecutionEffects> {
        let tx_digest = *tx.digest();
        let mut retry_cnt = 0;
        while retry_cnt < 10 {
            // Fullnode could time out after WAIT_FOR_FINALITY_TIMEOUT (30s) in
            // TransactionOrchestrator IotaClient times out after 60s
            match self
                .iota_client
                .quorum_driver_api()
                .execute_transaction_block(
                    tx.clone(),
                    IotaTransactionBlockResponseOptions::new().with_effects(),
                    None,
                )
                .await
            {
                Ok(resp) => {
                    return Ok(ExecutionEffects::IotaTransactionBlockEffects(
                        resp.effects.expect("effects field should not be None"),
                    ));
                }
                Err(err) => {
                    error!(
                        ?tx_digest,
                        retry_cnt, "Transaction failed with err: {:?}", err
                    );
                    retry_cnt += 1;
                }
            }
        }
        bail!("Transaction {:?} failed for {retry_cnt} times", tx_digest);
    }

    fn clone_committee(&self) -> Arc<Committee> {
        self.committee.clone()
    }

    fn get_current_epoch(&self) -> EpochId {
        self.committee.epoch
    }

    fn clone_new(&self) -> Box<dyn ValidatorProxy + Send + Sync> {
        Box::new(Self {
            iota_client: self.iota_client.clone(),
            committee: self.clone_committee(),
        })
    }

    async fn get_committee(&self) -> Result<Vec<IotaAddress>, anyhow::Error> {
        Ok(self
            .iota_client
            .governance_api()
            .get_latest_iota_system_state()
            .await?
            .iter_committee_members()
            .map(|v| v.iota_address)
            .collect())
    }
}

/// A proxy that sends transactions directly to a single validator via its gRPC
/// `handle_transaction` endpoint, bypassing quorum. Used by `ResilienceBench`
/// to concentrate injection traffic on one focal node.
pub struct DirectValidatorProxy {
    client: NetworkAuthorityClient,
    committee: Arc<Committee>,
}

impl DirectValidatorProxy {
    /// Build a proxy targeting the focal validator selected by `selector`.
    pub fn from_genesis(genesis: &Genesis, selector: &str) -> anyhow::Result<Self> {
        let (_, clients) =
            AuthorityAggregatorBuilder::from_genesis(genesis).build_network_clients();
        let committee = genesis.committee()?;
        let committee_size = committee.num_members();

        let (resolved_idx, target_name, client) = if selector.contains('.') {
            // This form is preferred for remote (AWS) benchmarks because the
            // BTreeMap is sorted by BLS public-key bytes, which is independent
            // of the genesis IP order — using an integer index would silently
            // attack the wrong validator.
            let network_committee = genesis.committee_with_network();
            clients
                .into_iter()
                .enumerate()
                .find(|(_, (name, _))| {
                    network_committee
                        .validators()
                        .get(name)
                        .map(|(_, meta)| meta.network_address.to_string().contains(selector))
                        .unwrap_or(false)
                })
                .map(|(i, (name, client))| (i, name, client))
                .ok_or_else(|| {
                    let addrs: Vec<String> = network_committee
                        .validators()
                        .values()
                        .map(|(_, m)| m.network_address.to_string())
                        .collect();
                    anyhow::anyhow!(
                        "focal_validator IP {:?} did not match any validator network address. \
                         Available: {:?} \
                         (hint: pass the IP used in --benchmark-ips, e.g. '3.124.187.12')",
                        selector,
                        addrs,
                    )
                })?
        } else if let Ok(idx) = selector.parse::<usize>() {
            // Integer index: pick the nth entry in BTreeMap<AuthorityName> order.
            // WARNING: this order is sorted by BLS key bytes and does NOT match
            // the genesis IP list order.  Prefer the IP-based selector above for
            // remote benchmarks.
            clients
                .into_iter()
                .enumerate()
                .nth(idx)
                .map(|(i, (name, client))| (i, name, client))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "focal_validator index {} out of range (committee size {})",
                        idx,
                        committee_size
                    )
                })?
        } else {
            // Key substring: find the first validator whose concise key contains
            // the selector string (e.g. "8dcff6" matches "k#8dcff6d1..").
            clients
                .into_iter()
                .enumerate()
                .find(|(_, (name, _))| format!("{}", name.concise()).contains(selector))
                .map(|(i, (name, client))| (i, name, client))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "focal_validator {:?} did not match any validator key \
                         (hint: use a substring of the key printed as \
                         'Metric address for validator <KEY>: ...' in the log)",
                        selector
                    )
                })?
        };

        // Print the authority key so the monitoring scripts can correlate it
        // with the metric address logged by benchmark_setup.rs.
        // benchmark_setup.rs prints "Metric address for validator <CONCISE>: <ADDR>"
        // using the same concise format, making the mapping unambiguous.
        eprintln!(
            "ResilienceBench focal node: validator_index={} authority={}",
            resolved_idx,
            target_name.concise(),
        );

        Ok(Self {
            client,
            committee: Arc::new(committee),
        })
    }
}

#[async_trait]
impl ValidatorProxy for DirectValidatorProxy {
    async fn get_object(&self, _object_id: ObjectID) -> Result<Object, anyhow::Error> {
        unimplemented!(
            "DirectValidatorProxy: object reads go through LocalValidatorAggregatorProxy during setup"
        )
    }

    async fn get_owned_objects(
        &self,
        _account_address: IotaAddress,
    ) -> Result<Vec<(u64, Object)>, anyhow::Error> {
        unimplemented!(
            "DirectValidatorProxy: object reads go through LocalValidatorAggregatorProxy during setup"
        )
    }

    async fn get_latest_system_state_object(
        &self,
    ) -> Result<IotaSystemStateSummary, anyhow::Error> {
        unimplemented!(
            "DirectValidatorProxy: system state goes through LocalValidatorAggregatorProxy during setup"
        )
    }

    /// Submits the transaction to the focal validator's handle_transaction
    /// endpoint. Always returns Err because injected TXs are expected to fail.
    async fn execute_transaction_block(&self, tx: Transaction) -> anyhow::Result<ExecutionEffects> {
        // Fire-and-forget: send to focal node, ignore the (error) response.
        // Propagate the error so bench_driver counts it as a failed TX,
        // which is the expected outcome for injection workloads.
        let _ = self.client.handle_transaction(tx, None).await?;
        // If the node somehow accepted it (unexpected for injection TXs),
        // return an error anyway — DirectValidatorProxy never produces effects.
        anyhow::bail!("DirectValidatorProxy: handle_transaction unexpectedly succeeded")
    }

    fn clone_committee(&self) -> Arc<Committee> {
        self.committee.clone()
    }

    fn get_current_epoch(&self) -> EpochId {
        self.committee.epoch
    }

    fn clone_new(&self) -> Box<dyn ValidatorProxy + Send + Sync> {
        Box::new(Self {
            client: self.client.clone(),
            committee: self.committee.clone(),
        })
    }

    async fn get_committee(&self) -> Result<Vec<IotaAddress>, anyhow::Error> {
        unimplemented!(
            "DirectValidatorProxy: committee address lookup not supported \
             (only used by delegation workload, which ResilienceBench does not run)"
        )
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum BenchMoveCallArg {
    Pure(Vec<u8>),
    Shared((ObjectID, SequenceNumber, bool)),
    ImmOrOwnedObject(ObjectRef),
    ImmOrOwnedObjectVec(Vec<ObjectRef>),
    SharedObjectVec(Vec<(ObjectID, SequenceNumber, bool)>),
}

impl From<bool> for BenchMoveCallArg {
    fn from(b: bool) -> Self {
        // unwrap safe because every u8 value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(&b).unwrap())
    }
}

impl From<u8> for BenchMoveCallArg {
    fn from(n: u8) -> Self {
        // unwrap safe because every u8 value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(&n).unwrap())
    }
}

impl From<u16> for BenchMoveCallArg {
    fn from(n: u16) -> Self {
        // unwrap safe because every u16 value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(&n).unwrap())
    }
}

impl From<u32> for BenchMoveCallArg {
    fn from(n: u32) -> Self {
        // unwrap safe because every u32 value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(&n).unwrap())
    }
}

impl From<u64> for BenchMoveCallArg {
    fn from(n: u64) -> Self {
        // unwrap safe because every u64 value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(&n).unwrap())
    }
}

impl From<u128> for BenchMoveCallArg {
    fn from(n: u128) -> Self {
        // unwrap safe because every u128 value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(&n).unwrap())
    }
}

impl From<&Vec<u8>> for BenchMoveCallArg {
    fn from(v: &Vec<u8>) -> Self {
        // unwrap safe because every vec<u8> value is BCS-serializable
        BenchMoveCallArg::Pure(bcs::to_bytes(v).unwrap())
    }
}

impl From<ObjectRef> for BenchMoveCallArg {
    fn from(obj: ObjectRef) -> Self {
        BenchMoveCallArg::ImmOrOwnedObject(obj)
    }
}

impl From<CallArg> for BenchMoveCallArg {
    fn from(ca: CallArg) -> Self {
        match ca {
            CallArg::Pure(value) => BenchMoveCallArg::Pure(value),
            CallArg::ImmutableOrOwned(obj_ref) => BenchMoveCallArg::ImmOrOwnedObject(obj_ref),
            CallArg::Shared(SharedObjectRef {
                object_id,
                initial_shared_version,
                mutable,
            }) => BenchMoveCallArg::Shared((object_id, initial_shared_version, mutable)),
            CallArg::Receiving(_) => {
                unimplemented!("Receiving is not supported for benchmarks")
            }
            _ => unimplemented!("a new CallArg enum variant was added and needs to be handled"),
        }
    }
}

/// Convert MoveCallArg to Vector of Argument for PT
pub fn convert_move_call_args(
    args: &[BenchMoveCallArg],
    pt_builder: &mut ProgrammableTransactionBuilder,
) -> Vec<Argument> {
    args.iter()
        .map(|arg| match arg {
            BenchMoveCallArg::Pure(bytes) => pt_builder.pure(bytes.clone()).unwrap(),
            BenchMoveCallArg::Shared((id, initial_shared_version, mutable)) => pt_builder
                .input(CallArg::Shared(SharedObjectRef {
                    object_id: *id,
                    initial_shared_version: *initial_shared_version,
                    mutable: *mutable,
                }))
                .unwrap(),
            BenchMoveCallArg::ImmOrOwnedObject(obj_ref) => pt_builder
                .input(CallArg::ImmutableOrOwned(*obj_ref))
                .unwrap(),
            BenchMoveCallArg::ImmOrOwnedObjectVec(obj_refs) => pt_builder
                .make_obj_vec(obj_refs.iter().map(|q| CallArg::ImmutableOrOwned(*q)))
                .unwrap(),
            BenchMoveCallArg::SharedObjectVec(obj_refs) => pt_builder
                .make_obj_vec(
                    obj_refs
                        .iter()
                        .map(|(id, initial_shared_version, mutable)| {
                            CallArg::Shared(SharedObjectRef {
                                object_id: *id,
                                initial_shared_version: *initial_shared_version,
                                mutable: *mutable,
                            })
                        }),
                )
                .unwrap(),
        })
        .collect()
}
