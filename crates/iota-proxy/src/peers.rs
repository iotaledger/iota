// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use bcs;
use fastcrypto::{ed25519::Ed25519PublicKey, traits::ToFromBytes};
use iota_sdk::{IotaClient, IotaClientBuilder, rpc_types::IotaObjectDataOptions};
use iota_tls::Allower;
use iota_types::{
    base_types::ObjectID,
    dynamic_field::Field,
    iota_system_state::{
        iota_system_state_inner_v1::ValidatorV1,
        iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
    },
};
use itertools::Itertools;
use tracing::{debug, error, info};

/// IotaPeers is a mapping of public key to IotaPeer data
pub type IotaPeers = Arc<RwLock<HashMap<Ed25519PublicKey, IotaPeer>>>;

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub struct IotaPeer {
    pub name: String,
    pub public_key: Ed25519PublicKey,
}

/// IotaNodeProvider queries the iota blockchain and keeps a record of known
/// validators based on the response from iota_getValidators.  The node name,
/// public key and other info is extracted from the chain and stored in this
/// data structure.  We pass this struct to the tls verifier and it depends on
/// the state contained within. Handlers also use this data in an Extractor
/// extension to check incoming clients on the http api against known keys.
#[derive(Debug, Clone)]
pub struct IotaNodeProvider {
    active_validator_nodes: IotaPeers,
    pending_validator_nodes: IotaPeers,
    static_nodes: IotaPeers,
    rpc_url: String,
    rpc_poll_interval: Duration,
}

impl Allower for IotaNodeProvider {
    fn allowed(&self, key: &Ed25519PublicKey) -> bool {
        self.static_nodes.read().unwrap().contains_key(key)
            || self
                .active_validator_nodes
                .read()
                .unwrap()
                .contains_key(key)
            || self
                .pending_validator_nodes
                .read()
                .unwrap()
                .contains_key(key)
    }
}

impl IotaNodeProvider {
    pub fn new(rpc_url: String, rpc_poll_interval: Duration, static_peers: Vec<IotaPeer>) -> Self {
        // build our hashmap with the static pub keys. we only do this one time at
        // binary startup.
        let static_nodes: HashMap<Ed25519PublicKey, IotaPeer> = static_peers
            .into_iter()
            .map(|v| (v.public_key.clone(), v))
            .collect();
        let static_nodes = Arc::new(RwLock::new(static_nodes));
        let active_validator_nodes = Arc::new(RwLock::new(HashMap::new()));
        let pending_validator_nodes = Arc::new(RwLock::new(HashMap::new()));
        Self {
            active_validator_nodes,
            pending_validator_nodes,
            static_nodes,
            rpc_url,
            rpc_poll_interval,
        }
    }

    /// get is used to retrieve peer info in our handlers
    pub fn get(&self, key: &Ed25519PublicKey) -> Option<IotaPeer> {
        debug!("look for {:?}", key);
        // check static nodes first
        if let Some(v) = self.static_nodes.read().unwrap().get(key) {
            return Some(IotaPeer {
                name: v.name.to_owned(),
                public_key: v.public_key.to_owned(),
            });
        }
        // check active validators
        if let Some(v) = self.active_validator_nodes.read().unwrap().get(key) {
            return Some(IotaPeer {
                name: v.name.to_owned(),
                public_key: v.public_key.to_owned(),
            });
        }
        // check pending validators
        if let Some(v) = self.pending_validator_nodes.read().unwrap().get(key) {
            return Some(IotaPeer {
                name: v.name.to_owned(),
                public_key: v.public_key.to_owned(),
            });
        }
        None
    }

    /// Get a mutable reference to the allowed validator map
    pub fn get_mut(&mut self) -> &mut IotaPeers {
        &mut self.active_validator_nodes
    }
    fn update_active_validator_set(&self, summary: &IotaSystemStateSummary) {
        let validators = extract_validators_from_summaries(&summary.active_validators);
        let mut allow = self.active_validator_nodes.write().unwrap();
        allow.clear();
        allow.extend(validators);
        info!(
            "{} iota validators managed to make it on the allow list",
            allow.len()
        );
    }

    fn update_pending_validator_set(&self, pending_validators: Vec<ValidatorV1>) {
        let summaries = pending_validators
            .into_iter()
            .map(|v| v.into_iota_validator_summary())
            .collect_vec();
        let validators = extract_validators_from_summaries(&summaries);
        let mut allow = self.pending_validator_nodes.write().unwrap();
        allow.clear();
        allow.extend(validators);
        info!(
            "{} iota pending validators managed to make it on the allow list",
            allow.len()
        );
    }

    async fn get_pending_validators(
        iota_client: &IotaClient,
        pending_active_validators_id: ObjectID,
    ) -> Result<Vec<ValidatorV1>> {
        let pending_validators_ids = iota_client
            .read_api()
            .get_dynamic_fields(pending_active_validators_id, None, None)
            .await?
            .data
            .into_iter()
            .map(|dyi| dyi.object_id)
            .collect::<Vec<_>>();

        let responses = iota_client
            .read_api()
            .multi_get_object_with_options(
                pending_validators_ids,
                IotaObjectDataOptions::default().with_bcs(),
            )
            .await?;

        responses
            .into_iter()
            .map(|resp| {
                let object_id = resp.object_id()?;
                let bcs = resp.move_object_bcs().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Object {object_id} does not exist or does not return bcs bytes",
                    )
                })?;
                let field = bcs::from_bytes::<Field<u64, ValidatorV1>>(bcs).map_err(|e| {
                anyhow::anyhow!(
                    "Can't convert bcs bytes of object {object_id} to Field<u64, ValidatorV1>: {e}",
                )
            })?;

                Ok(field.value)
            })
            .collect()
    }

    /// poll_peer_list will act as a refresh interval for our cache
    pub fn poll_peer_list(&self) {
        info!("Started polling for peers using rpc: {}", self.rpc_url);

        let rpc_poll_interval = self.rpc_poll_interval;
        let cloned_self = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(rpc_poll_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                match IotaClientBuilder::default()
                    .build(&cloned_self.rpc_url)
                    .await
                {
                    Ok(client) => {
                        match client.governance_api().get_latest_iota_system_state().await {
                            Ok(system_state) => {
                                cloned_self.update_active_validator_set(&system_state);
                                info!("Successfully updated active validators");

                                match Self::get_pending_validators(
                                    &client,
                                    system_state.pending_active_validators_id,
                                )
                                .await
                                {
                                    Ok(pending_validators) => {
                                        cloned_self
                                            .update_pending_validator_set(pending_validators);
                                        info!("Successfully updated pending validators");
                                    }
                                    Err(e) => {
                                        error!("Failed to get pending validators: {:?}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to get latest iota system state: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to create IotaClient: {:?}", e);
                    }
                }
            }
        });
    }
}

/// extract_validators_from_summaries will get the network pubkey bytes from a
/// IotaValidatorSummary type. This type comes from a full node rpc result. The
/// key here, if extracted successfully, will ultimately be stored in the allow
/// list and let us communicate with those actual peers via tls.
fn extract_validators_from_summaries(
    validator_summaries: &[IotaValidatorSummary],
) -> impl Iterator<Item = (Ed25519PublicKey, IotaPeer)> + use<'_> {
    validator_summaries.iter().filter_map(|vm| {
        match Ed25519PublicKey::from_bytes(&vm.network_pubkey_bytes) {
            Ok(public_key) => {
                debug!(
                    "adding public key {:?} for iota validator {:?}",
                    public_key, vm.name
                );
                Some((
                    public_key.clone(),
                    IotaPeer {
                        name: vm.name.to_owned(),
                        public_key,
                    },
                )) // scoped to filter_map
            }
            Err(error) => {
                error!(
                    "unable to decode public key for name: {:?} iota_address: {:?} error: {error}",
                    vm.name, vm.iota_address
                );
                None // scoped to filter_map
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use iota_types::iota_system_state::iota_system_state_summary::IotaValidatorSummary;
    use multiaddr::Multiaddr;

    use super::*;
    use crate::admin::{CertKeyPair, generate_self_cert};
    #[test]
    fn extract_validators_from_summary() {
        let CertKeyPair(_, client_pub_key) = generate_self_cert("iota".into());
        let p2p_address: Multiaddr = "/ip4/127.0.0.1/tcp/10000"
            .parse()
            .expect("expected a multiaddr value");
        let summaries = vec![IotaValidatorSummary {
            network_pubkey_bytes: Vec::from(client_pub_key.as_bytes()),
            p2p_address: format!("{p2p_address}"),
            primary_address: "empty".into(),
            ..Default::default()
        }];
        let peers = extract_validators_from_summaries(&summaries);
        assert_eq!(peers.count(), 1, "peers should have been a length of 1");
    }
}
