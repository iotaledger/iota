// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The attestor role of a fullnode: dry-runs the transactions it submits and
//! attaches an external attestation signed with its configured key.

use std::sync::Arc;

use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::{Address, Argument, Command, Input, Transaction, TransactionKind};
use iota_types::{
    IOTA_SYSTEM_PACKAGE_ID,
    attestation::{Attestation, AttestedTransaction},
    committee::EpochId,
    deny_rule_governance::DenyRuleConfig,
    error::{IotaError, IotaResult},
    iota_system_state::attestor_registry::attestor_pubkey_bytes,
    transaction::{TransactionAPI, VerifiedTransaction},
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
                error: "External attestation not supported at current protocol version".into(),
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

    /// Whether `transaction` is the one call the key cannot attest because it
    /// activates it: a lone `iota_system::register_attestor` or
    /// `rotate_attestor_key` naming the configured public key.
    pub(crate) fn registers_this_key(&self, transaction: &Transaction) -> bool {
        let TransactionKind::Programmable(ptb) = transaction.kind() else {
            return false;
        };
        let [Command::MoveCall(call)] = ptb.commands.as_slice() else {
            return false;
        };
        if call.package != IOTA_SYSTEM_PACKAGE_ID || call.module.as_str() != "iota_system" {
            return false;
        }
        let key_argument = match call.function.as_str() {
            "register_attestor" => 2,
            "rotate_attestor_key" => 1,
            _ => return false,
        };
        let Some(Argument::Input(index)) = call.arguments.get(key_argument) else {
            return false;
        };
        let Some(Input::Pure(bytes)) = ptb.inputs.get(usize::from(*index)) else {
            return false;
        };
        bcs::from_bytes::<Vec<u8>>(bytes).is_ok_and(|key| key == self.pubkey)
    }

    /// Dry-runs `transaction` as a validator would and wraps it with an
    /// external attestation by `attestor_address`.
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
            Attestation::new_external(&tx_digest, payload, attestor_address, &self.keypair);
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
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::{dbg_addr, random_object_ref},
        crypto::{AccountPrivateKey, get_key_pair, get_key_pair_from_rng},
        iota_system_state::attestor_registry::EpochStartAttestorInfoV1,
        object::Object,
        transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
        utils::to_sender_signed_transaction,
    };
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    use crate::{
        attestation_checks::verify_attestor,
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
        verify_attestor(&epoch_store, attested.digest(), &attested.attestation).unwrap();
    }

    /// Only a lone system call registering the configured key, or rotating
    /// to it, is exempt from attestation.
    #[test]
    fn registers_this_key_matches_only_the_configured_key() {
        let keypair = keypair_from_seed(7);
        let attestor = FullnodeAttestor::new(keypair.clone());
        let sender = dbg_addr(1);
        let gas = random_object_ref();
        let system_call = |function: &str, key: &SimpleKeypair| {
            let key_args = [
                CallArg::pure(&attestor_pubkey_bytes(key)),
                CallArg::pure(&vec![0u8; 64]),
            ];
            let args: Vec<CallArg> = match function {
                "register_attestor" => [
                    CallArg::IOTA_SYSTEM_MUTABLE,
                    CallArg::ImmutableOrOwned(random_object_ref()),
                ]
                .into_iter()
                .chain(key_args)
                .chain((0..4).map(|_| CallArg::pure(&b"x".to_vec())))
                .collect(),
                _ => [CallArg::IOTA_SYSTEM_MUTABLE]
                    .into_iter()
                    .chain(key_args)
                    .collect(),
            };
            TestTransactionBuilder::new(sender, gas, 1_000)
                .move_call(IOTA_SYSTEM_PACKAGE_ID, "iota_system", function, args)
                .build()
        };
        assert!(attestor.registers_this_key(&system_call("register_attestor", &keypair)));
        assert!(attestor.registers_this_key(&system_call("rotate_attestor_key", &keypair)));
        assert!(
            !attestor.registers_this_key(&system_call("register_attestor", &keypair_from_seed(8)))
        );
        assert!(!attestor.registers_this_key(&system_call("deregister_attestor", &keypair)));
        let transfer = TestTransactionBuilder::new(sender, gas, 1_000)
            .transfer_iota(None, dbg_addr(2))
            .build();
        assert!(!attestor.registers_this_key(&transfer));
    }
}
