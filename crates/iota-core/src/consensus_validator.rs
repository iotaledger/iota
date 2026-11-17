// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use eyre::WrapErr;
use fastcrypto_tbls::dkg_v1;
use iota_metrics::monitored_scope;
use iota_sdk_types::SenderSignedTransaction;
use iota_types::{
    base_types::ConciseableName,
    error::IotaError,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    transaction::transaction_identifiers_validity_check,
};
use prometheus_filtered::{IntCounter, Registry, register_int_counter_with_registry};
use starfish_core;
use tap::TapFallible;
use tracing::{info, instrument, warn};

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    checkpoints::CheckpointServiceNotify,
};

/// Allows verifying the validity of transactions
#[derive(Clone)]
pub struct IotaTxValidator {
    epoch_store: Arc<AuthorityPerEpochStore>,
    checkpoint_service: Arc<dyn CheckpointServiceNotify + Send + Sync>,
    metrics: Arc<IotaTxValidatorMetrics>,
}

impl IotaTxValidator {
    pub fn new(
        epoch_store: Arc<AuthorityPerEpochStore>,
        checkpoint_service: Arc<dyn CheckpointServiceNotify + Send + Sync>,
        metrics: Arc<IotaTxValidatorMetrics>,
    ) -> Self {
        info!(
            "IotaTxValidator constructed for epoch {}",
            epoch_store.epoch()
        );
        Self {
            epoch_store,
            checkpoint_service,
            metrics,
        }
    }

    /// Rejects the block when a transaction carries an identifier that
    /// move-core rejects, once the protocol disallows them: a validator whose
    /// decoder follows the move-core rules cannot decode such a block, so every
    /// validator must refuse it the same way.
    fn identifiers_check(&self, signed_tx: &SenderSignedTransaction) -> Result<(), IotaError> {
        if self
            .epoch_store
            .protocol_config()
            .disallow_self_identifier_in_transactions()
        {
            transaction_identifiers_validity_check(signed_tx)?;
        }
        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    fn validate_transactions(&self, txs: Vec<ConsensusTransactionKind>) -> Result<(), IotaError> {
        let mut cert_batch = Vec::new();
        let mut ckpt_messages = Vec::new();
        let mut ckpt_batch = Vec::new();
        let mut authority_cap_batch = Vec::new();
        let mut user_tx_count: u64 = 0;

        for tx in txs.iter() {
            match tx {
                ConsensusTransactionKind::CertifiedTransaction(certificate) => {
                    self.identifiers_check(certificate.data())?;
                    // A certificate carries no attestation, so it would skip the
                    // judge, the units claim and the cutoff.
                    if self
                        .epoch_store
                        .protocol_config()
                        .enable_validator_attestation()
                    {
                        return Err(IotaError::UnsupportedFeature {
                            error: "CertifiedTransaction not allowed when validator attestation \
                                    is enabled"
                                .into(),
                        });
                    }
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

                ConsensusTransactionKind::UserTransactionV1(attested_tx) => {
                    if !self.epoch_store.protocol_config().enable_pcool_flow() {
                        return Err(IotaError::UnsupportedFeature {
                            error: "UserTransactionV1 not supported at current protocol version"
                                .into(),
                        });
                    }
                    self.identifiers_check(attested_tx.transaction.data())?;
                    // An attestation is authenticated by the block signature,
                    // so only its presence is checked here, against the flag;
                    // post-consensus validation checks its content.
                    let attested = attested_tx.attestation.is_some();
                    let attestation_enabled = self
                        .epoch_store
                        .protocol_config()
                        .enable_validator_attestation();
                    if attested != attestation_enabled {
                        return Err(IotaError::UnsupportedFeature {
                            error: if attested {
                                "attested UserTransactionV1 not supported at current protocol \
                                 version"
                            } else {
                                "UserTransactionV1 must carry an attestation when validator \
                                 attestation is enabled"
                            }
                            .into(),
                        });
                    }
                    // TODO: Batch signature verification for UserTransactionV1.
                    //  For now verify individually, but this should be batched for performance
                    //  similar to how certificates are batch-verified above.
                    self.epoch_store
                        .signature_verifier
                        .verify_tx(attested_tx.transaction.data())
                        .tap_err(|e| {
                            warn!("UserTransactionV1 signature verification failed: {}", e)
                        })?;
                    user_tx_count += 1;
                }

                ConsensusTransactionKind::EndOfPublish(_)
                | ConsensusTransactionKind::CapabilityNotificationV1(_) => {}

                ConsensusTransactionKind::OverloadNotificationV1(authority_name, _, percentage) => {
                    if !self.epoch_store.protocol_config().enable_pcool_flow() {
                        return Err(IotaError::UnsupportedFeature {
                            error:
                                "OverloadNotificationV1 not supported at current protocol version"
                                    .into(),
                        });
                    }
                    if *percentage > 100 {
                        return Err(IotaError::HandleConsensusTransactionFailure(format!(
                            "OverloadNotificationV1 with invalid percentage {percentage} from \
                                authority {}",
                            authority_name.concise(),
                        )));
                    }
                }

                // No explicit size cap on the proposed rules: the proposal is
                // bounded by `max_transaction_size_bytes` (256 KiB), enforced
                // on every received block by starfish's block verifier.
                ConsensusTransactionKind::TransactionDenyRuleProposal(_) => {
                    if !self.epoch_store.protocol_config().enable_pcool_flow() {
                        return Err(IotaError::UnsupportedFeature {
                            error: "TransactionDenyRuleProposal not supported at current protocol version"
                                .into(),
                        });
                    }
                }
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
        self.metrics
            .user_transaction_signatures_verified
            .inc_by(user_tx_count);
        Ok(())

        // todo - we should un-comment line below once we have a way to revert
        // those transactions at the end of epoch all certificates had
        // valid signatures, schedule them for execution prior to sequencing
        // which is unnecessary for owned object transactions.
        // It is unnecessary to write to pending_certificates table because the
        // certs will be written via consensus output.
        // self.execution_scheduler
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
    user_transaction_signatures_verified: IntCounter,
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
            user_transaction_signatures_verified: register_int_counter_with_registry!(
                "user_transaction_signatures_verified",
                "Number of UserTransactionV1 signatures verified in consensus validator",
                registry
            )
            .unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fastcrypto::traits::KeyPair as _;
    use iota_macros::sim_test;
    use iota_protocol_config::{Chain, OverrideGuard, ProtocolConfig, ProtocolVersion};
    use iota_sdk_types::{Identifier, ObjectId, UserSignature};
    use iota_types::{
        attestation::AttestedTransaction,
        crypto::AuthorityKeyPair,
        error::{IotaError, UserInputError},
        messages_consensus::{
            ConsensusTransaction, ConsensusTransactionKind, MisbehaviorObservationsV1,
            VersionedMisbehaviorReport,
        },
        object::Object,
        transaction::{CertifiedTransaction, SenderSignedTransactionAPI, TransactionEnvelope},
    };
    use starfish_core::TransactionVerifier as _;

    use crate::{
        authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder},
        checkpoints::CheckpointServiceNoop,
        consensus_adapter::consensus_tests::{test_certificates, test_gas_objects},
        consensus_validator::{IotaTxValidator, IotaTxValidatorMetrics},
        test_utils::consensus_transaction_feature_gate,
    };

    #[sim_test]
    async fn accept_valid_transaction() {
        // A certificate carries no attestation, so this test runs with
        // validator attestation off.
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_validator_attestation_for_testing(false);
            config
        });

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
                cert.tx_signatures_mut_for_testing()[0] =
                    UserSignature::Simple(iota_types::crypto::zero_ed25519_signature());
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

    #[sim_test]
    async fn verify_batch_checks_user_transaction_v1_signature() {
        use iota_types::crypto::{AccountPrivateKey, deterministic_random_account_private_key};

        use crate::test_utils::make_transfer_iota_transaction;

        let (sender, sender_key): (_, AccountPrivateKey) =
            deterministic_random_account_private_key();
        let gas_object_id = ObjectId::random();
        let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);
        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(vec![gas_object])
                .build();
        let _guard = validator_attestation_off();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;
        let epoch_store = state.epoch_store_for_testing().clone();
        assert!(epoch_store.protocol_config().enable_pcool_flow());

        let rgp = epoch_store.reference_gas_price();
        let gas_ref = state.get_object(&gas_object_id).unwrap().object_ref();
        let valid_tx = make_transfer_iota_transaction(
            gas_ref,
            iota_sdk_types::Address::random(),
            None,
            sender,
            &sender_key,
            rgp,
        );
        // The sender's signature over other data, so that the signer check
        // passes and only the signature itself is wrong.
        let mut other_tx = make_transfer_iota_transaction(
            gas_ref,
            iota_sdk_types::Address::random(),
            None,
            sender,
            &sender_key,
            rgp,
        );
        let mut bad_tx = valid_tx.clone();
        bad_tx.tx_signatures_mut_for_testing()[0] =
            other_tx.tx_signatures_mut_for_testing()[0].clone();

        let expected_error = epoch_store
            .signature_verifier
            .verify_tx(bad_tx.data())
            .unwrap_err();
        assert!(
            matches!(expected_error, IotaError::InvalidSignature { .. }),
            "{expected_error:?}"
        );

        let to_bytes = |tx: &iota_types::transaction::TransactionEnvelope| {
            bcs::to_bytes(&ConsensusTransaction::new_user_transaction(tx.clone())).unwrap()
        };
        let valid_bytes = to_bytes(&valid_tx);
        let bad_bytes = to_bytes(&bad_tx);

        let metrics = IotaTxValidatorMetrics::new(&Default::default());
        let validator = IotaTxValidator::new(
            epoch_store,
            Arc::new(CheckpointServiceNoop {}),
            metrics.clone(),
        );

        let res = validator.verify_batch(&[&valid_bytes]);
        assert!(res.is_ok(), "{res:?}");
        assert_eq!(metrics.user_transaction_signatures_verified.get(), 1);

        for batch in [vec![&bad_bytes[..]], vec![&valid_bytes[..], &bad_bytes[..]]] {
            let res = validator.verify_batch(&batch);
            assert!(
                matches!(
                    &res,
                    Err(starfish_core::ValidationError::InvalidTransaction(msg))
                        if *msg == expected_error.to_string()
                ),
                "{res:?}"
            );
        }
        assert_eq!(metrics.user_transaction_signatures_verified.get(), 1);
    }

    /// Verifies that `validate_transactions` correctly gates every
    /// `ConsensusTransactionKind` variant against the current protocol config's
    /// feature flags.
    ///
    /// The exhaustive match forces a compile error when new variants are added,
    /// so the developer must explicitly map each variant to its gating flag.
    ///
    /// Every gating flag is set explicitly and the test runs with them all off
    /// and all on, so each gated variant is seen rejected and accepted
    /// regardless of the chain defaults.
    #[sim_test]
    async fn validate_transactions_feature_gating_with_gated_features_enabled() {
        validate_transactions_feature_gating(true).await;
    }

    #[sim_test]
    async fn validate_transactions_feature_gating_with_gated_features_disabled() {
        validate_transactions_feature_gating(false).await;
    }

    async fn validate_transactions_feature_gating(gated_features_enabled: bool) {
        use iota_types::crypto::{
            AccountPrivateKey, AuthorityPublicKeyBytes, deterministic_random_account_private_key,
        };

        use crate::test_utils::make_transfer_iota_transaction;

        let (sender, sender_key): (_, AccountPrivateKey) =
            deterministic_random_account_private_key();
        let gas_object_id = ObjectId::random();
        let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(vec![gas_object.clone()])
                .build();

        let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
            config.set_enable_pcool_flow_for_testing(gated_features_enabled);
            config.set_calculate_validator_scores_for_testing(gated_features_enabled);
            // The user transaction below carries no attestation.
            config.set_enable_validator_attestation_for_testing(false);
            config
        });
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;

        let rgp = state.epoch_store_for_testing().reference_gas_price();
        let gas_ref = state.get_object(&gas_object_id).unwrap().object_ref();
        let recipient = iota_sdk_types::Address::random();
        let signed_tx =
            make_transfer_iota_transaction(gas_ref, recipient, None, sender, &sender_key, rgp);

        let metrics = IotaTxValidatorMetrics::new(&Default::default());
        let validator = IotaTxValidator::new(
            state.epoch_store_for_testing().clone(),
            Arc::new(CheckpointServiceNoop {}),
            metrics,
        );

        let protocol_config = validator.epoch_store.protocol_config();
        assert_eq!(protocol_config.enable_pcool_flow(), gated_features_enabled);
        assert_eq!(
            protocol_config.calculate_validator_scores(),
            gated_features_enabled
        );
        let authority = AuthorityPublicKeyBytes::default();

        // Variants that can be validated without signature verification setup
        // (or that carry a valid signature, like UserTransactionV1).
        // CertifiedTransaction, CheckpointSignature, and
        // SignedCapabilityNotificationV1 are excluded because they require valid
        // cryptographic signatures and would fail before reaching the feature
        // gate check; their gating is recorded in
        // `consensus_transaction_feature_gate`.
        #[allow(deprecated)]
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
                ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
                    signed_tx, None,
                ))),
            ),
            (
                "OverloadNotificationV1",
                ConsensusTransactionKind::OverloadNotificationV1(authority, 0, 50),
            ),
            (
                "TransactionDenyRuleProposal",
                ConsensusTransactionKind::TransactionDenyRuleProposal(
                    iota_types::messages_consensus::TransactionDenyRuleProposal {
                        authority,
                        generation: 0,
                        proposed_rules: Default::default(),
                    },
                ),
            ),
        ];

        for (name, kind) in testable_variants {
            let gated = consensus_transaction_feature_gate(&kind, protocol_config);
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
    /// Validator attestation off, which the latest version has on, for tests
    /// that send plain user transactions and certificates through block
    /// verification.
    fn validator_attestation_off() -> OverrideGuard {
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_validator_attestation_for_testing(false);
            config
        })
    }

    /// A single-validator authority at `protocol_version`, or at the latest
    /// version when `None`, with its validator keypair and a block verifier
    /// over its epoch store.
    async fn self_identifier_fixture(
        protocol_version: Option<ProtocolVersion>,
    ) -> (Arc<AuthorityState>, AuthorityKeyPair, IotaTxValidator) {
        let mut network_config_builder =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir();
        if let Some(version) = protocol_version {
            network_config_builder = network_config_builder.with_protocol_version(version);
        }
        let network_config = network_config_builder.build();
        let keypair = network_config.validator_configs()[0]
            .authority_key_pair()
            .copy();
        let mut builder = TestAuthorityBuilder::new().with_network_config(&network_config, 0);
        if let Some(version) = protocol_version {
            builder = builder
                .with_protocol_config(ProtocolConfig::get_for_version(version, Chain::Unknown));
        }
        let state = builder.build().await;
        let validator = IotaTxValidator::new(
            state.epoch_store_for_testing().clone(),
            Arc::new(CheckpointServiceNoop {}),
            IotaTxValidatorMetrics::new(&Default::default()),
        );
        (state, keypair, validator)
    }

    /// A signed Move call of `0x2::coin::zero<0x2::iota::<type_name>>`.
    fn move_call_with_type_name(
        state: &AuthorityState,
        type_name: Identifier,
    ) -> TransactionEnvelope {
        use iota_sdk_types::{Address, StructTag, Transaction, TypeTag};
        use iota_types::{
            base_types::random_object_ref,
            crypto::{AccountPrivateKey, deterministic_random_account_private_key},
            programmable_transaction_builder::ProgrammableTransactionBuilder,
            transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
            utils::to_sender_signed_transaction,
        };

        let (sender, sender_key): (_, AccountPrivateKey) =
            deterministic_random_account_private_key();
        let gas_price = state.epoch_store_for_testing().reference_gas_price();
        let type_arg = TypeTag::Struct(Box::new(StructTag::new(
            "0x2".parse::<Address>().unwrap(),
            Identifier::from_static("iota"),
            type_name,
            vec![],
        )));
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.programmable_move_call(
            ObjectId::from("0x2".parse::<Address>().unwrap()),
            Identifier::from_static("coin"),
            Identifier::from_static("zero"),
            vec![type_arg],
            vec![],
        );
        let transaction = Transaction::new_programmable(
            sender,
            vec![random_object_ref()],
            builder.finish(),
            gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            gas_price,
        );
        to_sender_signed_transaction(transaction, &sender_key)
    }

    fn user_transaction(signed_tx: TransactionEnvelope) -> ConsensusTransactionKind {
        ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
            signed_tx, None,
        )))
    }

    /// `signed_tx` certified by the single validator of `state`.
    fn certificate(
        signed_tx: TransactionEnvelope,
        state: &AuthorityState,
        keypair: &AuthorityKeyPair,
    ) -> ConsensusTransactionKind {
        ConsensusTransactionKind::CertifiedTransaction(Box::new(
            CertifiedTransaction::new_from_keypairs_for_testing(
                signed_tx.into_data(),
                std::slice::from_ref(keypair),
                &state.clone_committee_for_testing(),
            ),
        ))
    }

    fn assert_self_identifier_rejected(result: &Result<(), IotaError>) {
        assert!(
            matches!(
                result,
                Err(IotaError::UserInput {
                    error: UserInputError::InvalidIdentifier { error }
                }) if error == "<SELF>"
            ),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn reject_identifier_move_core_rejects_in_user_transaction() {
        let _guard = validator_attestation_off();
        let (state, _, validator) = self_identifier_fixture(None).await;
        assert!(
            validator
                .epoch_store
                .protocol_config()
                .disallow_self_identifier_in_transactions()
        );

        let result = validator.validate_transactions(vec![user_transaction(
            move_call_with_type_name(&state, Identifier::new_unchecked("<SELF>")),
        )]);
        assert_self_identifier_rejected(&result);

        validator
            .validate_transactions(vec![user_transaction(move_call_with_type_name(
                &state,
                Identifier::from_static("IOTA"),
            ))])
            .unwrap();
    }

    #[tokio::test]
    async fn reject_identifier_move_core_rejects_in_certificate() {
        let _guard = validator_attestation_off();
        let (state, keypair, validator) = self_identifier_fixture(None).await;
        assert!(
            validator
                .epoch_store
                .protocol_config()
                .disallow_self_identifier_in_transactions()
        );

        let result = validator.validate_transactions(vec![certificate(
            move_call_with_type_name(&state, Identifier::new_unchecked("<SELF>")),
            &state,
            &keypair,
        )]);
        assert_self_identifier_rejected(&result);

        validator
            .validate_transactions(vec![certificate(
                move_call_with_type_name(&state, Identifier::from_static("IOTA")),
                &state,
                &keypair,
            )])
            .unwrap();
    }

    #[tokio::test]
    async fn reject_system_transaction_in_user_transaction() {
        use iota_sdk_types::{
            Address, Event, GenesisTransaction, SenderSignedTransaction, StructTag, Transaction,
            TransactionKind,
        };
        use iota_types::transaction::TransactionAPI;

        let (_, _, validator) = self_identifier_fixture(None).await;
        assert!(
            validator
                .epoch_store
                .protocol_config()
                .disallow_self_identifier_in_transactions()
        );

        // A system transaction needs no signature and can carry identifiers in
        // genesis events.
        let event = Event {
            package_id: ObjectId::random(),
            module: Identifier::new_unchecked("<SELF>"),
            sender: Address::random(),
            struct_tag: StructTag::new(
                "0x2".parse::<Address>().unwrap(),
                Identifier::from_static("iota"),
                Identifier::from_static("IOTA"),
                vec![],
            ),
            contents: vec![],
        };
        let signed_tx = TransactionEnvelope::new(SenderSignedTransaction::new(
            Transaction::new_system_transaction(TransactionKind::Genesis(GenesisTransaction {
                objects: vec![],
                events: vec![event],
            })),
            vec![],
        ));
        let result = validator.validate_transactions(vec![user_transaction(signed_tx)]);
        assert!(
            matches!(
                &result,
                Err(IotaError::UserInput {
                    error: UserInputError::Unsupported(_)
                })
            ),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn accept_identifier_move_core_rejects_before_the_flag() {
        let (state, keypair, validator) =
            self_identifier_fixture(Some(ProtocolVersion::new(39))).await;
        assert!(
            !validator
                .epoch_store
                .protocol_config()
                .disallow_self_identifier_in_transactions()
        );

        validator
            .validate_transactions(vec![
                user_transaction(move_call_with_type_name(
                    &state,
                    Identifier::new_unchecked("<SELF>"),
                )),
                certificate(
                    move_call_with_type_name(&state, Identifier::new_unchecked("<SELF>")),
                    &state,
                    &keypair,
                ),
            ])
            .unwrap();
    }

    /// A certificate carries no attestation, so block verification refuses it
    /// while validator attestation is on, whatever it holds.
    #[tokio::test]
    async fn reject_certificate_with_validator_attestation_on() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_enable_validator_attestation_for_testing(true);
            config.set_checkpoint_summary_version_specific_data_for_testing(2);
            config
        });
        let (state, keypair, validator) = self_identifier_fixture(None).await;
        assert!(
            validator
                .epoch_store
                .protocol_config()
                .enable_validator_attestation()
        );

        let result = validator.validate_transactions(vec![certificate(
            move_call_with_type_name(&state, Identifier::from_static("IOTA")),
            &state,
            &keypair,
        )]);
        assert!(
            matches!(&result, Err(IotaError::UnsupportedFeature { .. })),
            "{result:?}"
        );
    }
}
