// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The attestor role of a fullnode: dry-runs the transactions it submits and
//! attaches an explicit attestation signed with its configured key.

use std::sync::Arc;

use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::Address;
use iota_types::{
    attestation::{Attestation, AttestedTransaction},
    committee::EpochId,
    deny_rule_governance::DenyRuleConfig,
    error::{IotaError, IotaResult},
    iota_system_state::attestor_registry::attestor_pubkey_bytes,
    transaction::VerifiedTransaction,
};
use parking_lot::Mutex;
use tracing::{error, warn};

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    authority_server::DenyRuleUnion,
};

pub(crate) struct FullnodeAttestor {
    keypair: SimpleKeypair,
    /// The key as registered on chain, `flag || raw`.
    pubkey: Vec<u8>,
    /// The last epoch the key was reported inactive in, so the warning is
    /// logged once per epoch.
    inactive_warned_epoch: Mutex<Option<EpochId>>,
}

impl FullnodeAttestor {
    pub(crate) fn new(keypair: SimpleKeypair) -> Self {
        Self {
            pubkey: attestor_pubkey_bytes(&keypair),
            keypair,
            inactive_warned_epoch: Mutex::new(None),
        }
    }

    /// The address the key attests for in `epoch_store`'s epoch. Fails when
    /// external attestation is disabled or the key is not in the epoch's
    /// attestor set; either is logged once per epoch.
    pub(crate) fn active_address(
        &self,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaResult<Address> {
        let epoch = epoch_store.epoch();
        let error = if !epoch_store.protocol_config().enable_external_attestation() {
            IotaError::UnsupportedFeature {
                error: "Explicit attestation not supported at current protocol version".into(),
            }
        } else if let Some((_, entry)) = epoch_store.attestor_set().by_pubkey(&self.pubkey) {
            return Ok(entry.attestor_address);
        } else {
            IotaError::AttestorKeyInactive { epoch }
        };
        if self.inactive_warned_epoch.lock().replace(epoch) != Some(epoch) {
            warn!(
                epoch,
                "attestor key configured but inactive, rejecting submissions: {error}"
            );
        }
        Err(error)
    }

    /// Dry-runs `transaction` as a validator would and wraps it with an
    /// explicit attestation by `attestor_address`.
    pub(crate) async fn attest(
        &self,
        state: Arc<AuthorityState>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        transaction: VerifiedTransaction,
        attestor_address: Address,
    ) -> IotaResult<AttestedTransaction> {
        let tx_digest = *transaction.digest();
        let join_result = tokio::task::spawn_blocking(move || {
            // Built inside the closure: `DenyRuleUnion` borrows, and
            // `spawn_blocking` requires `'static` captures.
            let local_deny_config = &state.config.transaction_deny_config;
            let governance_rules = epoch_store
                .protocol_config()
                .deny_rule_governance()
                .then(|| epoch_store.get_active_transaction_deny_rules());
            let combined_deny_config;
            let deny_config: &dyn DenyRuleConfig = match governance_rules.as_ref() {
                Some(rules) => {
                    combined_deny_config = DenyRuleUnion {
                        first: local_deny_config,
                        second: rules.as_ref(),
                    };
                    &combined_deny_config
                }
                None => local_deny_config,
            };
            let result = state.attest_transaction(&transaction, &epoch_store, deny_config);
            (result, transaction)
        })
        .await;
        let (result, transaction) = join_result.map_err(|join_err| {
            error!(?tx_digest, "attest_transaction task failed: {join_err}");
            IotaError::GenericAuthority {
                error: format!("attest_transaction task failed: {join_err}"),
            }
        })?;
        // The owned objects only matter to a validator's soft locks.
        let (payload, _owned_objects) = result?;
        let attestation =
            Attestation::new_explicit(&tx_digest, payload, attestor_address, &self.keypair);
        Ok(AttestedTransaction::new(
            transaction.into_inner(),
            attestation,
        ))
    }
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_crypto::ed25519::Ed25519PrivateKey;
    use iota_sdk_types::{ObjectId, Transaction};
    use iota_types::{
        base_types::dbg_addr,
        crypto::{AccountPrivateKey, get_key_pair, get_key_pair_from_rng},
        iota_system_state::attestor_registry::EpochStartAttestorInfoV1,
        object::Object,
        transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
        utils::to_sender_signed_transaction,
    };
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    use crate::{
        attestation_checks::verify_explicit_attestation,
        authority::test_authority_builder::TestAuthorityBuilder,
    };

    fn keypair_from_seed(seed: u8) -> SimpleKeypair {
        SimpleKeypair::from(
            get_key_pair_from_rng::<Ed25519PrivateKey, _>(&mut StdRng::from_seed([seed; 32])).1,
        )
    }

    /// Only a key in the epoch's attestor set is active, and its attestation
    /// verifies against that set.
    #[tokio::test]
    async fn attests_with_an_active_key() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_enable_validator_attestation_for_testing(true);
            config.set_enable_external_attestation_for_testing(true);
            config
        });
        let keypair = keypair_from_seed(7);
        let entry = EpochStartAttestorInfoV1 {
            attestor_address: Address::random(),
            attestor_pubkey: attestor_pubkey_bytes(&keypair),
        };
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let object_id = ObjectId::random();
        let gas_id = ObjectId::random();
        let state = TestAuthorityBuilder::new()
            .with_starting_objects(&[
                Object::with_id_owner_for_testing(object_id, sender),
                Object::with_id_owner_for_testing(gas_id, sender),
            ])
            .with_epoch_start_attestors(vec![entry.clone()])
            .build()
            .await;
        let epoch_store = state.load_epoch_store_one_call_per_task();

        assert!(matches!(
            FullnodeAttestor::new(keypair_from_seed(8)).active_address(&epoch_store),
            Err(IotaError::AttestorKeyInactive { .. })
        ));
        let attestor = FullnodeAttestor::new(keypair);
        assert_eq!(
            attestor.active_address(&epoch_store).unwrap(),
            entry.attestor_address
        );

        let rgp = state.reference_gas_price_for_testing().unwrap();
        let object = state.get_object(&object_id).unwrap();
        let gas = state.get_object(&gas_id).unwrap();
        let tx_data = Transaction::new_transfer(
            dbg_addr(2),
            object.object_ref(),
            sender,
            gas.object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        let tx = epoch_store
            .verify_transaction(to_sender_signed_transaction(tx_data, &sender_key))
            .unwrap();
        let attested = attestor
            .attest(
                state.clone(),
                epoch_store.clone(),
                tx,
                entry.attestor_address,
            )
            .await
            .unwrap();
        verify_explicit_attestation(&epoch_store, attested.digest(), &attested.attestation)
            .unwrap();
    }
}
