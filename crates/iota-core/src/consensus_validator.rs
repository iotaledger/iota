// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use eyre::WrapErr;
use fastcrypto_tbls::dkg_v1;
use iota_metrics::monitored_scope;
use iota_types::{
    error::IotaError,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
};
use prometheus::{IntCounter, Registry, register_int_counter_with_registry};
use starfish_core;
use tap::TapFallible;
use tracing::{info, instrument, warn};

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    checkpoints::CheckpointServiceNotify, transaction_manager::TransactionManager,
};

/// Allows verifying the validity of transactions
#[derive(Clone)]
pub struct IotaTxValidator {
    epoch_store: Arc<AuthorityPerEpochStore>,
    checkpoint_service: Arc<dyn CheckpointServiceNotify + Send + Sync>,
    _transaction_manager: Arc<TransactionManager>,
    metrics: Arc<IotaTxValidatorMetrics>,
}

impl IotaTxValidator {
    pub fn new(
        epoch_store: Arc<AuthorityPerEpochStore>,
        checkpoint_service: Arc<dyn CheckpointServiceNotify + Send + Sync>,
        transaction_manager: Arc<TransactionManager>,
        metrics: Arc<IotaTxValidatorMetrics>,
    ) -> Self {
        info!(
            "IotaTxValidator constructed for epoch {}",
            epoch_store.epoch()
        );
        Self {
            epoch_store,
            checkpoint_service,
            _transaction_manager: transaction_manager,
            metrics,
        }
    }

    #[instrument(level = "trace", skip_all)]
    fn validate_transactions(&self, txs: Vec<ConsensusTransactionKind>) -> Result<(), IotaError> {
        let mut cert_batch = Vec::new();
        let mut ckpt_messages = Vec::new();
        let mut ckpt_batch = Vec::new();
        let mut authority_cap_batch = Vec::new();

        for tx in txs.iter() {
            match tx {
                ConsensusTransactionKind::CertifiedTransaction(certificate) => {
                    cert_batch.push(certificate.as_ref());
                }
                ConsensusTransactionKind::CheckpointSignature(signature) => {
                    ckpt_messages.push(signature.as_ref());
                    ckpt_batch.push(&signature.summary);
                }
                ConsensusTransactionKind::RandomnessDkgMessage(_, bytes) => {
                    if bytes.len() > dkg_v1::DKG_MESSAGES_MAX_SIZE {
                        warn!("batch verification error: DKG Message too large");
                        return Err(IotaError::InvalidDkgMessageSize);
                    }
                }
                ConsensusTransactionKind::RandomnessDkgConfirmation(_, bytes) => {
                    if bytes.len() > dkg_v1::DKG_MESSAGES_MAX_SIZE {
                        warn!("batch verification error: DKG Confirmation too large");
                        return Err(IotaError::InvalidDkgMessageSize);
                    }
                }
                ConsensusTransactionKind::SignedCapabilityNotificationV1(signed_cap) => {
                    authority_cap_batch.push(signed_cap);
                }

                ConsensusTransactionKind::MisbehaviorReport(_) => {
                    if !self
                        .epoch_store
                        .protocol_config()
                        .calculate_validator_scores()
                    {
                        return Err(IotaError::UnsupportedFeature {
                            error: "MisbehaviorReport not supported at current protocol version"
                                .into(),
                        });
                    }
                }
                #[allow(deprecated)]
                ConsensusTransactionKind::NewJWKFetchedDeprecated => {
                    return Err(IotaError::UnsupportedFeature {
                        error: "NewJWKFetched (zkLogin) is deprecated and not supported".into(),
                    });
                }

                ConsensusTransactionKind::UserTransactionV1(transaction) => {
                    if !self.epoch_store.protocol_config().enable_white_flag_flow() {
                        return Err(IotaError::UnsupportedFeature {
                            error: "UserTransactionV1 not supported at current protocol version"
                                .into(),
                        });
                    }
                    // TODO: Batch signature verification for UserTransactionV1.
                    //  For now verify individually, but this should be batched for performance
                    //  similar to how certificates are batch-verified above.
                    self.epoch_store
                        .signature_verifier
                        .verify_tx(transaction.data())
                        .tap_err(|e| {
                            warn!("UserTransactionV1 signature verification failed: {}", e)
                        })?;
                }

                ConsensusTransactionKind::EndOfPublish(_)
                | ConsensusTransactionKind::CapabilityNotificationV1(_) => {}
            }
        }

        // verify the certificate signatures as a batch
        let cert_count = cert_batch.len();
        let ckpt_count = ckpt_batch.len();
        let authority_cap_count = authority_cap_batch.len();

        self.epoch_store
            .signature_verifier
            .verify_certs_and_checkpoints(cert_batch, ckpt_batch, authority_cap_batch)
            .tap_err(|e| warn!("batch verification error: {}", e))?;

        // All checkpoint sigs have been verified, forward them to the checkpoint
        // service
        for ckpt in ckpt_messages {
            self.checkpoint_service
                .notify_checkpoint_signature(&self.epoch_store, ckpt)?;
        }

        self.metrics
            .certificate_signatures_verified
            .inc_by(cert_count as u64);
        self.metrics
            .checkpoint_signatures_verified
            .inc_by(ckpt_count as u64);
        self.metrics
            .authority_capabilities_verified
            .inc_by(authority_cap_count as u64);
        Ok(())

        // todo - we should un-comment line below once we have a way to revert
        // those transactions at the end of epoch all certificates had
        // valid signatures, schedule them for execution prior to sequencing
        // which is unnecessary for owned object transactions.
        // It is unnecessary to write to pending_certificates table because the
        // certs will be written via consensus output.
        // self.transaction_manager
        //     .enqueue_certificates(owned_tx_certs, &self.epoch_store)
        //     .wrap_err("Failed to schedule certificates for execution")
    }
}

fn tx_from_bytes(tx: &[u8]) -> Result<ConsensusTransaction, eyre::Report> {
    bcs::from_bytes::<ConsensusTransaction>(tx)
        .wrap_err("Malformed transaction (failed to deserialize)")
}

impl starfish_core::TransactionVerifier for IotaTxValidator {
    #[instrument(level = "trace", skip_all)]
    fn verify_batch(
        &self,
        batch: &[&[u8]],
    ) -> core::result::Result<(), starfish_core::ValidationError> {
        let _scope = monitored_scope("ValidateBatch");

        let txs = batch
            .iter()
            .map(|tx| {
                tx_from_bytes(tx)
                    .map(|tx| tx.kind)
                    .map_err(|e| starfish_core::ValidationError::InvalidTransaction(e.to_string()))
            })
            .collect::<core::result::Result<Vec<_>, _>>()?;

        self.validate_transactions(txs)
            .map_err(|e| starfish_core::ValidationError::InvalidTransaction(e.to_string()))
    }
}

pub struct IotaTxValidatorMetrics {
    certificate_signatures_verified: IntCounter,
    checkpoint_signatures_verified: IntCounter,
    authority_capabilities_verified: IntCounter,
}

impl IotaTxValidatorMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            certificate_signatures_verified: register_int_counter_with_registry!(
                "certificate_signatures_verified",
                "Number of certificates verified in consensus batch verifier",
                registry
            )
            .unwrap(),
            checkpoint_signatures_verified: register_int_counter_with_registry!(
                "checkpoint_signatures_verified",
                "Number of checkpoint verified in consensus batch verifier",
                registry
            )
            .unwrap(),
            authority_capabilities_verified: register_int_counter_with_registry!(
                "authority_capabilities_verified",
                "Number of signed authority capabilities verified in consensus batch verifier",
                registry
            )
            .unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iota_macros::sim_test;
    use iota_protocol_config::Chain;
    use iota_types::{
        crypto::Ed25519IotaSignature,
        error::IotaError,
        messages_consensus::{
            ConsensusTransaction, ConsensusTransactionKind, MisbehaviorObservationsV1,
            VersionedMisbehaviorReport,
        },
        object::Object,
        signature::GenericSignature,
    };
    use starfish_core::TransactionVerifier as _;

    use crate::{
        authority::test_authority_builder::TestAuthorityBuilder,
        checkpoints::CheckpointServiceNoop,
        consensus_adapter::consensus_tests::{test_certificates, test_gas_objects},
        consensus_validator::{IotaTxValidator, IotaTxValidatorMetrics},
    };

    #[sim_test]
    async fn accept_valid_transaction() {
        // Initialize an authority with a (owned) gas object and a shared object; then
        // make a test certificate.
        let mut objects = test_gas_objects();
        let shared_object = Object::shared_for_testing();
        objects.push(shared_object.clone());

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(objects.clone())
                .build();

        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;
        let name1 = state.name;
        let certificates = test_certificates(&state, shared_object).await;

        let first_transaction = certificates[0].clone();
        let first_transaction_bytes: Vec<u8> = bcs::to_bytes(
            &ConsensusTransaction::new_certificate_message(&name1, first_transaction),
        )
        .unwrap();

        let metrics = IotaTxValidatorMetrics::new(&Default::default());
        let validator = IotaTxValidator::new(
            state.epoch_store_for_testing().clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.transaction_manager().clone(),
            metrics,
        );
        let res = validator.verify_batch(&[&first_transaction_bytes]);
        assert!(res.is_ok(), "{res:?}");

        let transaction_bytes: Vec<_> = certificates
            .clone()
            .into_iter()
            .map(|cert| {
                bcs::to_bytes(&ConsensusTransaction::new_certificate_message(&name1, cert)).unwrap()
            })
            .collect();

        let batch: Vec<_> = transaction_bytes.iter().map(|t| t.as_slice()).collect();
        let res_batch = validator.verify_batch(&batch);
        assert!(res_batch.is_ok(), "{res_batch:?}");

        let bogus_transaction_bytes: Vec<_> = certificates
            .into_iter()
            .map(|mut cert| {
                // set it to an all-zero user signature
                cert.tx_signatures_mut_for_testing()[0] = GenericSignature::Signature(
                    iota_types::crypto::Signature::Ed25519IotaSignature(
                        Ed25519IotaSignature::default(),
                    ),
                );
                bcs::to_bytes(&ConsensusTransaction::new_certificate_message(&name1, cert)).unwrap()
            })
            .collect();

        let batch: Vec<_> = bogus_transaction_bytes
            .iter()
            .map(|t| t.as_slice())
            .collect();
        let res_batch = validator.verify_batch(&batch);
        assert!(res_batch.is_err());
    }

    /// Verifies that `validate_transactions` correctly gates every
    /// `ConsensusTransactionKind` variant against the current protocol config's
    /// feature flags.
    ///
    /// The exhaustive match forces a compile error when new variants are added,
    /// so the developer must explicitly map each variant to its gating flag.
    ///
    /// NOTE: This test is primarily useful for variants that are under
    /// development or not yet fully rolled out on all networks. Once all
    /// feature-gated variants are enabled on every network, this test can be
    /// ignored — its value lies in catching missing gates during the upgrade
    /// window between code deployment and protocol activation.
    #[sim_test]
    async fn validate_transactions_feature_gating() {
        use iota_protocol_config::ProtocolConfig;
        use iota_types::{
            base_types::ObjectID,
            crypto::{AccountKeyPair, AuthorityPublicKeyBytes, deterministic_random_account_key},
        };

        use crate::test_utils::make_transfer_iota_transaction;

        let (sender, sender_key): (_, AccountKeyPair) = deterministic_random_account_key();
        let gas_object_id = ObjectID::random();
        let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(vec![gas_object.clone()])
                .build();

        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .with_chain_override(Chain::Mainnet)
            .build()
            .await;

        let rgp = state.epoch_store_for_testing().reference_gas_price();
        let gas_ref = state
            .get_object(&gas_object_id)
            .await
            .unwrap()
            .compute_object_reference();
        let recipient = iota_types::crypto::get_key_pair::<AccountKeyPair>().0;
        let signed_tx =
            make_transfer_iota_transaction(gas_ref, recipient, None, sender, &sender_key, rgp);

        let metrics = IotaTxValidatorMetrics::new(&Default::default());
        let validator = IotaTxValidator::new(
            state.epoch_store_for_testing().clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.transaction_manager().clone(),
            metrics,
        );

        let protocol_config = validator.epoch_store.protocol_config();
        let authority = AuthorityPublicKeyBytes::default();

        // Returns the feature flag value that gates a variant, or `None` if the
        // variant is always allowed. The exhaustive match ensures this function
        // must be updated when new variants are added to ConsensusTransactionKind.
        #[allow(deprecated)]
        fn is_feature_gated(
            kind: &ConsensusTransactionKind,
            config: &ProtocolConfig,
        ) -> Option<bool> {
            match kind {
                // Always allowed (no feature flag gating).
                ConsensusTransactionKind::CertifiedTransaction(_)
                | ConsensusTransactionKind::CheckpointSignature(_)
                | ConsensusTransactionKind::EndOfPublish(_)
                | ConsensusTransactionKind::CapabilityNotificationV1(_)
                | ConsensusTransactionKind::SignedCapabilityNotificationV1(_)
                | ConsensusTransactionKind::RandomnessDkgMessage(_, _)
                | ConsensusTransactionKind::RandomnessDkgConfirmation(_, _) => None,

                // Gated behind `enable_white_flag_flow`.
                ConsensusTransactionKind::UserTransactionV1(_) => {
                    Some(config.enable_white_flag_flow())
                }

                // Gated behind `calculate_validator_scores`.
                ConsensusTransactionKind::MisbehaviorReport(_) => {
                    Some(config.calculate_validator_scores())
                }

                // Always rejected: zkLogin JWK support was never enabled on
                // IOTA and the variant is retained only for serialization
                // compatibility.
                ConsensusTransactionKind::NewJWKFetchedDeprecated => Some(false),
            }
        }

        // Variants that can be validated without signature verification setup
        // (or that carry a valid signature, like UserTransactionV1).
        // CertifiedTransaction, CheckpointSignature, and
        // SignedCapabilityNotificationV1 are excluded because they require valid
        // cryptographic signatures and would fail before reaching the feature
        // gate check; their gating is verified by the exhaustive match above.
        let testable_variants: Vec<(&str, ConsensusTransactionKind)> = vec![
            (
                "EndOfPublish",
                ConsensusTransactionKind::EndOfPublish(authority),
            ),
            (
                "NewJWKFetchedDeprecated",
                ConsensusTransactionKind::NewJWKFetchedDeprecated,
            ),
            (
                "CapabilityNotificationV1",
                ConsensusTransactionKind::CapabilityNotificationV1(
                    iota_types::messages_consensus::AuthorityCapabilitiesV1::new(
                        authority,
                        Chain::Mainnet,
                        iota_types::supported_protocol_versions::SupportedProtocolVersions::SYSTEM_DEFAULT,
                        vec![],
                    ),
                ),
            ),
            (
                "RandomnessDkgMessage",
                ConsensusTransactionKind::RandomnessDkgMessage(authority, vec![]),
            ),
            (
                "RandomnessDkgConfirmation",
                ConsensusTransactionKind::RandomnessDkgConfirmation(authority, vec![]),
            ),
            (
                "MisbehaviorReport",
                ConsensusTransactionKind::MisbehaviorReport(VersionedMisbehaviorReport::new_v1(
                    authority,
                    0,
                    MisbehaviorObservationsV1 {
                        faulty_blocks_provable: vec![],
                        faulty_blocks_unprovable: vec![],
                        missing_proposals: vec![],
                        equivocations: vec![],
                    },
                )),
            ),
            (
                "UserTransactionV1",
                ConsensusTransactionKind::UserTransactionV1(Box::new(signed_tx)),
            ),
        ];

        for (name, kind) in testable_variants {
            let gated = is_feature_gated(&kind, protocol_config);
            let result = validator.validate_transactions(vec![kind]);

            match gated {
                Some(false) => {
                    // Feature flag is disabled: must be rejected.
                    assert!(
                        matches!(&result, Err(IotaError::UnsupportedFeature { .. })),
                        "{name}: feature flag is disabled, expected UnsupportedFeature, \
                         got {result:?}",
                    );
                }
                Some(true) | None => {
                    // Feature flag is enabled or variant is ungated: must not
                    // be rejected as unsupported.
                    assert!(
                        !matches!(&result, Err(IotaError::UnsupportedFeature { .. })),
                        "{name}: should not be rejected as UnsupportedFeature, got {result:?}",
                    );
                }
            }
        }
    }
}
