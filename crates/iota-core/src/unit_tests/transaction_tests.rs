// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
};

use fastcrypto::traits::KeyPair;
use iota_macros::sim_test;
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::crypto::Intent;
use iota_types::{
    base_types::{Identifier, dbg_addr, random_object_ref},
    crypto::{AccountKeyPair, Signature, get_key_pair},
    error::{IotaError, UserInputError},
    messages_consensus::ConsensusDeterminedVersionAssignments,
    messages_grpc::HandleSoftBundleCertificatesRequestV1,
    transaction::{GenesisTransaction, TransactionDataAPI, TransactionKind},
    utils::to_sender_signed_transaction,
};
use starfish_core::{BlockRef, BlockStatus};

use crate::{
    authority::{
        authority_test_utils::send_batch_consensus_no_execution,
        authority_tests::{call_move_, create_gas_objects, publish_object_basics},
        test_authority_builder::TestAuthorityBuilder,
    },
    consensus_adapter::consensus_tests::make_consensus_adapter_for_test,
    mock_consensus::with_block_status,
};
macro_rules! assert_matches {
    ($expression:expr, $pattern:pat $(if $guard: expr)?) => {
        match $expression {
            $pattern $(if $guard)? => {}
            ref e => panic!(
                "assertion failed: `(left == right)` \
                 (left: `{:?}`, right: `{:?}`)",
                e,
                stringify!($pattern $(if $guard)?)
            ),
        }
    };
}

use fastcrypto::traits::AggregateAuthenticator;
use iota_types::{
    digests::ConsensusCommitDigest, messages_consensus::ConsensusCommitPrologueV1,
    messages_grpc::HandleCertificateRequestV1,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
};

use super::*;
pub use crate::authority::authority_test_utils::init_state_with_ids;
use crate::{
    authority_client::{AuthorityAPI, NetworkAuthorityClient},
    authority_server::AuthorityServer,
    stake_aggregator::{InsertResult, StakeAggregator},
};

#[sim_test]
async fn test_handle_transfer_transaction_bad_signature() {
    do_transaction_test(
        1,
        |_| {},
        |mut_tx| {
            let (_unknown_address, unknown_key): (_, AccountKeyPair) = get_key_pair();
            let data = mut_tx.data_mut_for_testing();
            *data.tx_signatures_mut_for_testing() =
                vec![Signature::new_secure(data.intent_message(), &unknown_key).into()];
        },
        |err| {
            assert_matches!(err, IotaError::SignerSignatureAbsent { .. });
        },
    )
    .await;
}

#[sim_test]
async fn test_handle_transfer_transaction_no_signature() {
    do_transaction_test(
        1,
        |_| {},
        |tx| {
            *tx.data_mut_for_testing().tx_signatures_mut_for_testing() = vec![];
        },
        |err| {
            assert_matches!(
                err,
                IotaError::SignerSignatureNumberMismatch {
                    expected: 1,
                    actual: 0
                }
            );
        },
    )
    .await;
}

#[sim_test]
async fn test_handle_transfer_transaction_extra_signature() {
    do_transaction_test(
        1,
        |_| {},
        |tx| {
            let sigs = tx.data_mut_for_testing().tx_signatures_mut_for_testing();
            sigs.push(sigs[0].clone());
        },
        |err| {
            assert_matches!(
                err,
                IotaError::SignerSignatureNumberMismatch {
                    expected: 1,
                    actual: 2
                }
            );
        },
    )
    .await;
}

#[sim_test]
async fn test_empty_gas_data() {
    do_transaction_test_skip_cert_checks(
        0,
        |tx| {
            tx.gas_data_mut().objects = vec![];
        },
        |_| {},
        |err| {
            assert_matches!(
                err,
                IotaError::UserInput {
                    error: UserInputError::MissingGasPayment
                }
            );
        },
    )
    .await;
}

#[sim_test]
async fn test_duplicate_gas_data() {
    do_transaction_test_skip_cert_checks(
        0,
        |tx| {
            let gas_data = tx.gas_data_mut();
            let new_gas = gas_data.objects[0];
            gas_data.objects.push(new_gas);
        },
        |_| {},
        |err| {
            assert_matches!(
                err,
                IotaError::UserInput {
                    error: UserInputError::MutableObjectUsedMoreThanOnce { .. }
                }
            );
        },
    )
    .await;
}

#[sim_test]
async fn test_gas_wrong_owner_matches_sender() {
    do_transaction_test(
        1,
        |tx| {
            let gas_data = tx.gas_data_mut();
            let (new_addr, _): (_, AccountKeyPair) = get_key_pair();
            gas_data.owner = new_addr;
            *tx.sender_mut_for_testing() = new_addr;
        },
        |_| {},
        |err| {
            assert_matches!(err, IotaError::SignerSignatureAbsent { .. });
        },
    )
    .await;
}

#[sim_test]
async fn test_gas_wrong_owner() {
    do_transaction_test(
        1,
        |tx| {
            let gas_data = tx.gas_data_mut();
            let (new_addr, _): (_, AccountKeyPair) = get_key_pair();
            gas_data.owner = new_addr;
        },
        |_| {},
        |err| {
            assert_matches!(
                err,
                IotaError::SignerSignatureNumberMismatch {
                    expected: 2,
                    actual: 1
                }
            );
        },
    )
    .await;
}

#[sim_test]
async fn test_user_sends_genesis_transaction() {
    test_user_sends_system_transaction_impl(TransactionKind::Genesis(GenesisTransaction {
        objects: vec![],
        events: vec![],
    }))
    .await;
}

#[tokio::test]
async fn test_user_sends_consensus_commit_prologue_v1() {
    test_user_sends_system_transaction_impl(TransactionKind::ConsensusCommitPrologueV1(
        ConsensusCommitPrologueV1 {
            epoch: 0,
            round: 0,
            sub_dag_index: None,
            commit_timestamp_ms: 42,
            consensus_commit_digest: ConsensusCommitDigest::default(),
            consensus_determined_version_assignments:
                ConsensusDeterminedVersionAssignments::CancelledTransactions {
                    cancelled_transactions: Vec::new(),
                },
        },
    ))
    .await;
}

#[tokio::test]
async fn test_user_sends_end_of_epoch_transaction() {
    test_user_sends_system_transaction_impl(TransactionKind::EndOfEpoch(vec![])).await;
}

async fn test_user_sends_system_transaction_impl(transaction_kind: TransactionKind) {
    do_transaction_test_skip_cert_checks(
        0,
        |tx| {
            *tx.kind_mut() = transaction_kind.clone();
        },
        |_| {},
        |err| {
            assert_matches!(
                err,
                IotaError::UserInput {
                    error: UserInputError::Unsupported { .. }
                }
            );
        },
    )
    .await;
}

pub fn init_transfer_transaction(
    pre_sign_mutations: impl Fn(&mut TransactionData),
    sender: IotaAddress,
    secret: &AccountKeyPair,
    recipient: IotaAddress,
    object_ref: ObjectRef,
    gas_object_ref: ObjectRef,
    gas_budget: u64,
    gas_price: u64,
) -> Transaction {
    let mut data = TransactionData::new_transfer(
        recipient,
        object_ref,
        sender,
        gas_object_ref,
        gas_budget,
        gas_price,
    );
    pre_sign_mutations(&mut data);
    to_sender_signed_transaction(data, secret)
}

pub fn init_move_call_transaction(
    pre_sign_mutations: impl Fn(&mut TransactionData),
    sender: IotaAddress,
    secret: &AccountKeyPair,
    gas_object_ref: ObjectRef,
    gas_budget: u64,
    gas_price: u64,
) -> Transaction {
    let mut data = TransactionData::new_move_call(
        sender,
        ObjectID::SYSTEM,
        Identifier::IOTA_SYSTEM_MODULE,
        Identifier::from_static("request_add_validator"),
        vec![],
        gas_object_ref,
        vec![CallArg::IOTA_SYSTEM_MUTABLE],
        gas_budget,
        gas_price,
    )
    .unwrap();
    pre_sign_mutations(&mut data);
    to_sender_signed_transaction(data, secret)
}

async fn do_transaction_test_skip_cert_checks(
    expected_sig_errors: u64,
    pre_sign_mutations: impl Fn(&mut TransactionData),
    post_sign_mutations: impl Fn(&mut Transaction),
    err_check: impl Fn(&IotaError),
) {
    do_transaction_test_impl(
        expected_sig_errors,
        false,
        pre_sign_mutations,
        post_sign_mutations,
        err_check,
    )
    .await
}

async fn do_transaction_test(
    expected_sig_errors: u64,
    pre_sign_mutations: impl Fn(&mut TransactionData),
    post_sign_mutations: impl Fn(&mut Transaction),
    err_check: impl Fn(&IotaError),
) {
    do_transaction_test_impl(
        expected_sig_errors,
        true,
        pre_sign_mutations,
        post_sign_mutations,
        err_check,
    )
    .await
}

async fn do_transaction_test_impl(
    _expected_sig_errors: u64,
    check_forged_cert: bool,
    pre_sign_mutations: impl Fn(&mut TransactionData),
    post_sign_mutations: impl Fn(&mut Transaction),
    err_check: impl Fn(&IotaError),
) {
    telemetry_subscribers::init_for_testing();
    let (sender1, sender_key1): (_, AccountKeyPair) = get_key_pair();
    let (sender2, sender_key2): (_, AccountKeyPair) = get_key_pair();
    let recipient = dbg_addr(2);
    let object_id = ObjectID::random();
    let gas_object_id1 = ObjectID::random();
    let gas_object_id2 = ObjectID::random();
    let authority_state = init_state_with_ids(vec![
        (sender1, object_id),
        (sender1, gas_object_id1),
        (sender2, gas_object_id2),
    ])
    .await;
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas_object1 = authority_state.get_object(&gas_object_id1).await.unwrap();
    let gas_object2 = authority_state.get_object(&gas_object_id2).await.unwrap();

    // Execute the test with two transactions, one transfer and one move call.
    // The move call contains access to a shared object.
    // We test both txs and expect the same error.
    let mut transfer_transaction = init_transfer_transaction(
        &pre_sign_mutations,
        sender1,
        &sender_key1,
        recipient,
        object.compute_object_reference(),
        gas_object1.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );

    let mut move_call_transaction = init_move_call_transaction(
        &pre_sign_mutations,
        sender2,
        &sender_key2,
        gas_object2.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );

    let server = AuthorityServer::new_for_test(authority_state.clone());

    let server_handle = server.spawn_for_test().await.unwrap();

    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        Some(
            authority_state
                .config
                .network_key_pair()
                .public()
                .to_owned(),
        ),
    )
    .await
    .unwrap();

    post_sign_mutations(&mut transfer_transaction);
    post_sign_mutations(&mut move_call_transaction);
    let socket_addr = make_socket_addr();

    let transactions = vec![transfer_transaction, move_call_transaction];
    for transaction in &transactions {
        let err = client
            .handle_transaction(transaction.clone(), Some(socket_addr))
            .await
            .unwrap_err();
        err_check(&err);
    }

    check_locks(authority_state.clone(), vec![object_id]).await;

    // now verify that the same transactions are rejected if false certificates are
    // somehow formed and sent
    if check_forged_cert {
        let epoch_store = authority_state.epoch_store_for_testing();
        for transaction in transactions {
            let signed_transaction = VerifiedSignedTransaction::new(
                epoch_store.epoch(),
                VerifiedTransaction::new_unchecked(transaction),
                authority_state.name,
                &*authority_state.secret,
            );
            let mut agg = StakeAggregator::new(epoch_store.committee().clone());

            let InsertResult::QuorumReached(cert_sig) =
                agg.insert(signed_transaction.clone().into())
            else {
                panic!("quorum expected");
            };

            let plain_tx = signed_transaction.into_inner();

            let ct = CertifiedTransaction::new_from_data_and_sig(plain_tx.into_data(), cert_sig);

            let err = client
                .handle_certificate_v1(
                    HandleCertificateRequestV1::new(ct.clone()),
                    Some(socket_addr),
                )
                .await
                .unwrap_err();
            err_check(&err);
            epoch_store.clear_signature_cache();
            let err = client
                .handle_certificate_v1(
                    HandleCertificateRequestV1::new(ct.clone()),
                    Some(socket_addr),
                )
                .await
                .unwrap_err();
            err_check(&err);

            // Additionally, if the tx contains access to shared objects, check if Soft
            // Bundle handler returns the same error.
            if ct.contains_shared_object() {
                epoch_store.clear_signature_cache();
                let err = client
                    .handle_soft_bundle_certificates_v1(
                        HandleSoftBundleCertificatesRequestV1 {
                            certificates: vec![ct.clone()],
                            wait_for_effects: true,
                            include_events: false,
                            include_auxiliary_data: false,
                            include_input_objects: false,
                            include_output_objects: false,
                        },
                        Some(socket_addr),
                    )
                    .await
                    .unwrap_err();
                err_check(&err);
            }
        }
    }
}

async fn check_locks(authority_state: Arc<AuthorityState>, object_ids: Vec<ObjectID>) {
    for object_id in object_ids {
        let object = authority_state.get_object(&object_id).await.unwrap();
        assert!(
            authority_state
                .get_transaction_lock(
                    &object.compute_object_reference(),
                    &authority_state.epoch_store_for_testing()
                )
                .await
                .unwrap()
                .is_none()
        );
    }
}

fn make_socket_addr() -> std::net::SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0)
}

#[tokio::test]
async fn test_oversized_txn() {
    telemetry_subscribers::init_for_testing();
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = dbg_addr(2);
    let object_id = ObjectID::random();
    let authority_state = init_state_with_ids(vec![(sender, object_id)]).await;
    let max_txn_size = authority_state
        .epoch_store_for_testing()
        .protocol_config()
        .max_tx_size_bytes() as usize;
    let object = authority_state.get_object(&object_id).await.unwrap();
    let obj_ref = object.compute_object_reference();

    // Construct an oversized txn.
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        // Put a lot of commands in the txn so it's large.
        for _ in 0..(1024 * 16) {
            builder.transfer_object(recipient, obj_ref).unwrap();
        }
        builder.finish()
    };

    let txn_data = TransactionData::new_programmable(sender, vec![obj_ref], pt, 0, 0);

    let txn = to_sender_signed_transaction(txn_data, &sender_key);
    let tx_size = bcs::serialized_size(&txn).unwrap();

    // Making sure the txn is larger than the max txn size.
    assert!(tx_size > max_txn_size);

    let server = AuthorityServer::new_for_test(authority_state.clone());

    let server_handle = server.spawn_for_test().await.unwrap();

    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        Some(
            authority_state
                .config
                .network_key_pair()
                .public()
                .to_owned(),
        ),
    )
    .await
    .unwrap();

    let res = client
        .handle_transaction(txn, Some(make_socket_addr()))
        .await;
    // The txn should be rejected due to its size.
    assert!(
        res.err()
            .unwrap()
            .to_string()
            .contains("serialized transaction size exceeded maximum")
    );
}

#[tokio::test]
async fn test_very_large_certificate() {
    telemetry_subscribers::init_for_testing();
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = dbg_addr(2);
    let object_id = ObjectID::random();
    let gas_object_id = ObjectID::random();
    let authority_state =
        init_state_with_ids(vec![(sender, object_id), (sender, gas_object_id)]).await;
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas_object = authority_state.get_object(&gas_object_id).await.unwrap();

    let transfer_transaction = init_transfer_transaction(
        |_| {},
        sender,
        &sender_key,
        recipient,
        object.compute_object_reference(),
        gas_object.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );

    let server = AuthorityServer::new_for_test(authority_state.clone());

    let server_handle = server.spawn_for_test().await.unwrap();

    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        Some(
            authority_state
                .config
                .network_key_pair()
                .public()
                .to_owned(),
        ),
    )
    .await
    .unwrap();
    let socket_addr = make_socket_addr();

    let auth_sig = client
        .handle_transaction(transfer_transaction.clone(), Some(socket_addr))
        .await
        .unwrap()
        .status
        .into_signed_for_testing();

    let signatures: BTreeMap<_, _> = vec![auth_sig]
        .into_iter()
        .map(|a| (a.authority, a.signature))
        .collect();

    // Insert a lot into the bitmap so the cert is very large, while the txn inside
    // is reasonably sized.
    let mut signers_map = roaring::bitmap::RoaringBitmap::new();
    // Insert every even number up to 52,108,864 (~52 million).
    // Avoiding inserting contiguous ranges to skip range compression.
    for i in (0..52_108_864).step_by(2) {
        signers_map.insert(i);
    }

    let sigs: Vec<AuthoritySignature> = signatures.into_values().collect();

    let quorum_signature = iota_types::crypto::AuthorityQuorumSignInfo {
        epoch: 0,
        signature: iota_types::crypto::AggregateAuthoritySignature::aggregate(&sigs)
            .map_err(|e| IotaError::InvalidSignature {
                error: e.to_string(),
            })
            .expect("Validator returned invalid signature"),
        signers_map,
    };
    let cert = iota_types::message_envelope::Envelope::new_from_data_and_sig(
        transfer_transaction.into_data(),
        quorum_signature,
    );

    let res = client
        .handle_certificate_v1(HandleCertificateRequestV1::new(cert), Some(socket_addr))
        .await;
    assert!(res.is_err());
    let err = res.err().unwrap();
    println!("ERROR: {err:?}");
    // The resulting error should be a RpcError with a message length too large.
    assert!(
        matches!(err, IotaError::Rpc(..)) && err.to_string().contains("message length too large")
    );
}

#[tokio::test]
async fn test_handle_certificate_errors() {
    telemetry_subscribers::init_for_testing();
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = dbg_addr(2);
    let object_id = ObjectID::random();
    let gas_object_id = ObjectID::random();
    let authority_state =
        init_state_with_ids(vec![(sender, object_id), (sender, gas_object_id)]).await;
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas_object = authority_state.get_object(&gas_object_id).await.unwrap();

    let transfer_transaction = init_transfer_transaction(
        |_| {},
        sender,
        &sender_key,
        recipient,
        object.compute_object_reference(),
        gas_object.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );

    let server = AuthorityServer::new_for_test(authority_state.clone());

    let server_handle = server.spawn_for_test().await.unwrap();

    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        Some(
            authority_state
                .config
                .network_key_pair()
                .public()
                .to_owned(),
        ),
    )
    .await
    .unwrap();

    // Test handle certificate from the wrong epoch
    let epoch_store = authority_state.epoch_store_for_testing();
    let next_epoch = epoch_store.epoch() + 1;
    let signed_transaction = VerifiedSignedTransaction::new(
        next_epoch,
        VerifiedTransaction::new_unchecked(transfer_transaction.clone()),
        authority_state.name,
        &*authority_state.secret,
    );

    let mut committee_1 = epoch_store.committee().deref().clone();
    committee_1.epoch = next_epoch;
    let ct = CertifiedTransaction::new(
        transfer_transaction.data().clone(),
        vec![signed_transaction.auth_sig().clone()],
        &committee_1,
    )
    .unwrap();
    let socket_addr = make_socket_addr();

    let err = client
        .handle_certificate_v1(
            HandleCertificateRequestV1::new(ct.clone()),
            Some(socket_addr),
        )
        .await
        .unwrap_err();
    assert_matches!(
        err,
        IotaError::WrongEpoch {
            expected_epoch: 0,
            actual_epoch: 1
        }
    );

    // Test handle certificate with invalid user input
    let signed_transaction = VerifiedSignedTransaction::new(
        epoch_store.epoch(),
        VerifiedTransaction::new_unchecked(transfer_transaction.clone()),
        authority_state.name,
        &*authority_state.secret,
    );

    let committee = epoch_store.committee().deref().clone();

    let tx = VerifiedTransaction::new_consensus_commit_prologue_v1(
        0,
        0,
        42,
        ConsensusCommitDigest::default(),
        Vec::new(),
    );
    let ct = CertifiedTransaction::new(
        tx.data().clone(),
        vec![signed_transaction.auth_sig().clone()],
        &committee,
    )
    .unwrap();

    let err = client
        .handle_certificate_v1(
            HandleCertificateRequestV1::new(ct.clone()),
            Some(socket_addr),
        )
        .await
        .unwrap_err();

    assert_matches!(
        err,
        IotaError::UserInput {
            error: UserInputError::Unsupported(message)
        } if message == "SenderSignedData must not contain system transaction"
    );

    let mut invalid_sig_count_tx = transfer_transaction.clone();
    let data = invalid_sig_count_tx.data_mut_for_testing();
    data.tx_signatures_mut_for_testing().clear();
    let ct = CertifiedTransaction::new(
        data.clone(),
        vec![signed_transaction.auth_sig().clone()],
        &committee,
    )
    .unwrap();
    let err = client
        .handle_certificate_v1(
            HandleCertificateRequestV1::new(ct.clone()),
            Some(socket_addr),
        )
        .await
        .unwrap_err();

    assert_matches!(
        err,
        IotaError::SignerSignatureNumberMismatch {
            expected: 1,
            actual: 0
        }
    );

    let mut absent_sig_tx = transfer_transaction.clone();
    let (_unknown_address, unknown_key): (_, AccountKeyPair) = get_key_pair();
    let data = absent_sig_tx.data_mut_for_testing();
    *data.tx_signatures_mut_for_testing() =
        vec![Signature::new_secure(data.intent_message(), &unknown_key).into()];
    let ct = CertifiedTransaction::new(
        data.clone(),
        vec![signed_transaction.auth_sig().clone()],
        &committee,
    )
    .unwrap();

    let err = client
        .handle_certificate_v1(
            HandleCertificateRequestV1::new(ct.clone()),
            Some(socket_addr),
        )
        .await
        .unwrap_err();

    assert_matches!(err, IotaError::SignerSignatureAbsent { .. });
}

#[sim_test]
async fn test_handle_soft_bundle_certificates() {
    telemetry_subscribers::init_for_testing();

    let mut protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
    protocol_config.set_max_soft_bundle_size_for_testing(10);

    let authority = TestAuthorityBuilder::new()
        .with_reference_gas_price(1000)
        .with_protocol_config(protocol_config)
        .build()
        .await;

    let mut senders = Vec::new();
    let mut gas_object_ids = Vec::new();
    for _i in 0..4 {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let gas_object_id = ObjectID::random();

        let obj = Object::with_id_owner_for_testing(gas_object_id, address);
        authority.insert_genesis_object(obj).await;

        senders.push((address, keypair));
        gas_object_ids.push(gas_object_id);
    }

    let (authority, package) = publish_object_basics(authority).await;

    let shared_object = {
        let effects = call_move_(
            &authority,
            None,
            &gas_object_ids[0],
            &senders[0].0,
            &senders[0].1,
            &package.object_id,
            "object_basics",
            "share",
            vec![],
            vec![],
            true,
        )
        .await
        .unwrap();
        effects.status().unwrap();
        let shared_object_id = effects.created()[0].0.object_id;
        authority.get_object(&shared_object_id).await.unwrap()
    };
    let initial_shared_version = shared_object.version();

    // Create a server with mocked consensus.
    // This ensures transactions submitted to consensus will get processed.
    let adapter = make_consensus_adapter_for_test(
        authority.clone(),
        HashSet::new(),
        true,
        vec![with_block_status(BlockStatus::Sequenced(
            starfish_core::GenericTransactionRef::BlockRef(BlockRef::MIN),
        ))],
    );
    let server = AuthorityServer::new_for_test_with_consensus_adapter(authority.clone(), adapter);
    let _metrics = server.metrics.clone();
    let server_handle = server.spawn_for_test().await.unwrap();
    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        Some(authority.config.network_key_pair().public().to_owned()),
    )
    .await
    .unwrap();

    let signed_tx_into_certificate = |transaction: Transaction| async {
        let epoch_store = authority.load_epoch_store_one_call_per_task();
        let committee = authority.clone_committee_for_testing();
        let mut sigs = vec![];

        let transaction = epoch_store.verify_transaction(transaction).unwrap();
        let response = authority
            .handle_transaction(&epoch_store, transaction.clone())
            .await
            .unwrap();
        let vote = response.status.into_signed_for_testing();
        sigs.push(vote);
        if let Ok(cert) =
            CertifiedTransaction::new(transaction.clone().into_message(), sigs.clone(), &committee)
        {
            return cert
                .try_into_verified_for_testing(&committee, &Default::default())
                .unwrap();
        }
        panic!("Failed to create certificate");
    };

    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let mut certificates: Vec<CertifiedTransaction> = Vec::new();
    for i in 0..4 {
        let cert = {
            let gas_object_ref = authority
                .get_object(&gas_object_ids[i])
                .await
                .unwrap()
                .compute_object_reference();
            let data = TransactionData::new_move_call(
                senders[i].0,
                package.object_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("set_value"),
                // type_args
                vec![],
                gas_object_ref,
                // args
                vec![
                    CallArg::Shared(SharedObjectRef::new(
                        shared_object.id(),
                        initial_shared_version,
                        true,
                    )),
                    CallArg::Pure((i as u64).to_le_bytes().to_vec()),
                ],
                TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
                rgp,
            )
            .unwrap();
            let signed = to_sender_signed_transaction(data, &senders[i].1);
            signed_tx_into_certificate(signed).await
        };
        certificates.push(cert.into());
    }
    let responses = client
        .handle_soft_bundle_certificates_v1(
            HandleSoftBundleCertificatesRequestV1 {
                certificates,
                wait_for_effects: true,
                include_events: false,
                include_auxiliary_data: false,
                include_input_objects: true,
                include_output_objects: true,
            },
            None,
        )
        .await
        .unwrap()
        .responses;

    // Verify if transactions have been executed in the correct order.
    // This is done by checking if each tx's input object version matches the
    // previous tx's output object version.
    assert_eq!(responses.len(), 4);
    let mut expected_object_version = initial_shared_version;
    for response in responses {
        let input_objects = response.input_objects.unwrap();
        assert!(
            input_objects
                .iter()
                .any(|obj| obj.id() == shared_object.id()
                    && obj.version() == expected_object_version)
        );

        let output_objects = response.output_objects.unwrap();
        let output_object = output_objects
            .iter()
            .find(|obj| obj.id() == shared_object.id())
            .unwrap();
        expected_object_version = output_object.version();
    }
}

#[tokio::test]
async fn test_handle_soft_bundle_certificates_errors() {
    let mut senders = Vec::new();
    let mut gas_objects = Vec::new();
    let mut owned_objects = Vec::new();
    for _i in 0..15 {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let mut objects = create_gas_objects(2, sender);
        senders.push((sender, keypair));
        gas_objects.push(objects.remove(0));
        owned_objects.push(objects.remove(0));
    }

    let mut protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
    protocol_config.set_max_soft_bundle_size_for_testing(3);
    protocol_config.set_consensus_max_transactions_in_block_bytes_for_testing(10_000);
    let authority = TestAuthorityBuilder::new()
        .with_reference_gas_price(1000)
        .with_protocol_config(protocol_config)
        .build()
        .await;

    authority.insert_genesis_objects(&gas_objects).await;
    authority.insert_genesis_objects(&owned_objects).await;

    let (authority, package) = publish_object_basics(authority).await;

    let shared_object = {
        let effects = call_move_(
            &authority,
            None,
            &gas_objects[3].id(),
            &senders[3].0,
            &senders[3].1,
            &package.object_id,
            "object_basics",
            "share",
            vec![],
            vec![],
            true,
        )
        .await
        .unwrap();
        effects.status().unwrap();
        let shared_object_id = effects.created()[0].0.object_id;
        authority.get_object(&shared_object_id).await.unwrap()
    };
    let initial_shared_version = shared_object.version();

    // Create a single validator cluster.
    let server = AuthorityServer::new_for_test(authority);
    let authority = server.state.clone();
    let _metrics = server.metrics.clone();
    let server_handle = server.spawn_for_test().await.unwrap();
    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        Some(authority.config.network_key_pair().public().to_owned()),
    )
    .await
    .unwrap();

    let signed_tx_into_certificate = |transaction: Transaction| async {
        let epoch_store = authority.load_epoch_store_one_call_per_task();
        let committee = authority.clone_committee_for_testing();
        let mut sigs = vec![];

        let transaction = epoch_store.verify_transaction(transaction).unwrap();
        let response = authority
            .handle_transaction(&epoch_store, transaction.clone())
            .await
            .unwrap();
        let vote = response.status.into_signed_for_testing();
        sigs.push(vote);
        if let Ok(cert) =
            CertifiedTransaction::new(transaction.clone().into_message(), sigs.clone(), &committee)
        {
            return cert
                .try_into_verified_for_testing(&committee, &Default::default())
                .unwrap();
        }
        panic!("Failed to create certificate");
    };

    let rgp = authority.reference_gas_price_for_testing().unwrap();

    // Case 0: submit an empty soft bundle.
    println!("Case 0: submit an empty soft bundle.");
    {
        let response = client
            .handle_soft_bundle_certificates_v1(
                HandleSoftBundleCertificatesRequestV1 {
                    certificates: vec![],
                    wait_for_effects: true,
                    include_events: false,
                    include_auxiliary_data: false,
                    include_input_objects: false,
                    include_output_objects: false,
                },
                None,
            )
            .await;
        assert!(response.is_err());
        assert_matches!(response.unwrap_err(), IotaError::NoCertificateProvided);
    }

    // Case 1: submit a soft bundle with more txs than the limit.
    // The bundle should be rejected.
    println!("Case 1: submit a soft bundle with more txs than the limit.");
    {
        let mut certificates: Vec<CertifiedTransaction> = vec![];
        for i in 0..5 {
            let owned_object_ref = authority
                .get_object(&owned_objects[i].id())
                .await
                .unwrap()
                .compute_object_reference();
            let gas_object_ref = authority
                .get_object(&gas_objects[i].id())
                .await
                .unwrap()
                .compute_object_reference();
            let data = TransactionData::new_transfer(
                senders[i + 1].0,
                owned_object_ref,
                senders[i].0,
                gas_object_ref,
                rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                rgp,
            );
            let signed = to_sender_signed_transaction(data, &senders[i].1);
            certificates.push(signed_tx_into_certificate(signed).await.into());
        }
        let response = client
            .handle_soft_bundle_certificates_v1(
                HandleSoftBundleCertificatesRequestV1 {
                    certificates,
                    wait_for_effects: true,
                    include_events: false,
                    include_auxiliary_data: false,
                    include_input_objects: false,
                    include_output_objects: false,
                },
                None,
            )
            .await;
        assert!(response.is_err());
        assert_matches!(
            response.unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::TooManyTransactionsInSoftBundle { .. },
            }
        );
    }

    // Case 2: submit a soft bundle with tx containing no shared object.
    // The bundle should be rejected.
    println!("Case 2: submit a soft bundle with tx containing no shared object.");
    {
        let owned_object_ref = authority
            .get_object(&owned_objects[5].id())
            .await
            .unwrap()
            .compute_object_reference();
        let gas_object_ref = authority
            .get_object(&gas_objects[5].id())
            .await
            .unwrap()
            .compute_object_reference();
        let data = TransactionData::new_transfer(
            senders[6].0,
            owned_object_ref,
            senders[5].0,
            gas_object_ref,
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        let signed = to_sender_signed_transaction(data, &senders[5].1);
        let response = client
            .handle_soft_bundle_certificates_v1(
                HandleSoftBundleCertificatesRequestV1 {
                    certificates: vec![signed_tx_into_certificate(signed).await.into()],
                    wait_for_effects: true,
                    include_events: false,
                    include_auxiliary_data: false,
                    include_input_objects: false,
                    include_output_objects: false,
                },
                None,
            )
            .await;
        assert!(response.is_err());
        assert_matches!(
            response.unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::NoSharedObject { .. },
            }
        );
    }

    // Case 3: submit a soft bundle with txs of different gas prices.
    // The bundle should be rejected.
    println!("Case 3: submit a soft bundle with txs of different gas prices.");
    {
        let cert0 = {
            let gas_object_ref = authority
                .get_object(&gas_objects[6].id())
                .await
                .unwrap()
                .compute_object_reference();
            let data = TransactionData::new_move_call(
                senders[6].0,
                package.object_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("set_value"),
                // type_args
                vec![],
                gas_object_ref,
                // args
                vec![
                    CallArg::Shared(SharedObjectRef::new(
                        shared_object.id(),
                        initial_shared_version,
                        true,
                    )),
                    CallArg::Pure(11u64.to_le_bytes().to_vec()),
                ],
                TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
                rgp,
            )
            .unwrap();
            let signed = to_sender_signed_transaction(data, &senders[6].1);
            signed_tx_into_certificate(signed).await
        };
        let cert1 = {
            let gas_object_ref = authority
                .get_object(&gas_objects[7].id())
                .await
                .unwrap()
                .compute_object_reference();
            let data = TransactionData::new_move_call(
                senders[7].0,
                package.object_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("set_value"),
                // type_args
                vec![],
                gas_object_ref,
                // args
                vec![
                    CallArg::Shared(SharedObjectRef::new(
                        shared_object.id(),
                        initial_shared_version,
                        true,
                    )),
                    CallArg::Pure(12u64.to_le_bytes().to_vec()),
                ],
                TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
                rgp + 1,
            )
            .unwrap();
            let signed = to_sender_signed_transaction(data, &senders[7].1);
            signed_tx_into_certificate(signed).await
        };
        let response = client
            .handle_soft_bundle_certificates_v1(
                HandleSoftBundleCertificatesRequestV1 {
                    certificates: vec![cert0.into(), cert1.into()],
                    wait_for_effects: true,
                    include_events: false,
                    include_auxiliary_data: false,
                    include_input_objects: false,
                    include_output_objects: false,
                },
                None,
            )
            .await;
        assert!(response.is_err());
        assert_matches!(
            response.unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::GasPriceMismatch { .. },
            }
        );
    }

    // Case 4: submit a soft bundle with txs whose consensus message has been
    // processed. The bundle should be rejected.
    println!("Case 4: submit a soft bundle with txs whose consensus message has been processed.");
    {
        let cert0 = {
            let gas_object_ref = authority
                .get_object(&gas_objects[8].id())
                .await
                .unwrap()
                .compute_object_reference();
            let data = TransactionData::new_move_call(
                senders[8].0,
                package.object_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("set_value"),
                // type_args
                vec![],
                gas_object_ref,
                // args
                vec![
                    CallArg::Shared(SharedObjectRef::new(
                        shared_object.id(),
                        initial_shared_version,
                        true,
                    )),
                    CallArg::Pure(11u64.to_le_bytes().to_vec()),
                ],
                TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
                rgp,
            )
            .unwrap();
            let signed = to_sender_signed_transaction(data, &senders[8].1);
            signed_tx_into_certificate(signed).await
        };
        let cert1 = {
            let gas_object_ref = authority
                .get_object(&gas_objects[9].id())
                .await
                .unwrap()
                .compute_object_reference();
            let data = TransactionData::new_move_call(
                senders[9].0,
                package.object_id,
                Identifier::from_static("object_basics"),
                Identifier::from_static("set_value"),
                // type_args
                vec![],
                gas_object_ref,
                // args
                vec![
                    CallArg::Shared(SharedObjectRef::new(
                        shared_object.id(),
                        initial_shared_version,
                        true,
                    )),
                    CallArg::Pure(12u64.to_le_bytes().to_vec()),
                ],
                TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
                rgp,
            )
            .unwrap();
            let signed = to_sender_signed_transaction(data, &senders[9].1);
            signed_tx_into_certificate(signed).await
        };
        send_batch_consensus_no_execution(&authority, &[cert0.clone(), cert1.clone()], true).await;
        let response = client
            .handle_soft_bundle_certificates_v1(
                HandleSoftBundleCertificatesRequestV1 {
                    certificates: vec![cert0.into(), cert1.into()],
                    wait_for_effects: true,
                    include_events: false,
                    include_auxiliary_data: false,
                    include_input_objects: false,
                    include_output_objects: false,
                },
                None,
            )
            .await;
        assert!(response.is_err());
        assert_matches!(
            response.unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::CertificateAlreadyProcessed,
            }
        );
    }

    // Case 5: submit a soft bundle with total tx size exceeding the block size
    // limit. The bundle should be rejected.
    println!("Case 5: submit a soft bundle with total tx size exceeding the block size limit.");
    {
        let mut certificates: Vec<CertifiedTransaction> = vec![];

        for i in 11..14 {
            let owned_object_ref = authority
                .get_object(&owned_objects[i].id())
                .await
                .unwrap()
                .compute_object_reference();
            let gas_object_ref = authority
                .get_object(&gas_objects[i].id())
                .await
                .unwrap()
                .compute_object_reference();
            let sender = &senders[i];
            let recipient = &senders[i + 1].0;

            // Construct an oversized txn.
            let pt = {
                let mut builder = ProgrammableTransactionBuilder::new();
                // Put a lot of commands in the txn so it's large.
                for _ in 0..1000 {
                    builder
                        .transfer_object(*recipient, owned_object_ref)
                        .unwrap();
                }
                builder.finish()
            };

            let data = TransactionData::new_programmable(
                sender.0,
                vec![gas_object_ref],
                pt,
                rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                rgp,
            );

            let signed = to_sender_signed_transaction(data, &sender.1);
            certificates.push(signed_tx_into_certificate(signed).await.into());
        }

        let response = client
            .handle_soft_bundle_certificates_v1(
                HandleSoftBundleCertificatesRequestV1 {
                    certificates,
                    wait_for_effects: true,
                    include_events: false,
                    include_auxiliary_data: false,
                    include_input_objects: false,
                    include_output_objects: false,
                },
                None,
            )
            .await;
        assert!(response.is_err());
        assert_matches!(
            response.unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::SoftBundleTooLarge {
                    size: 25116,
                    limit: 5000
                },
            }
        );
    }
}

#[test]
fn sender_signed_data_serialized_intent() {
    let mut txn = SenderSignedData::new(
        TransactionData::new_transfer(
            IotaAddress::ZERO,
            random_object_ref(),
            IotaAddress::ZERO,
            random_object_ref(),
            0,
            0,
        ),
        vec![],
    );

    assert_eq!(txn.intent_message().intent, Intent::iota_transaction());

    // deser fails when intent is wrong
    let mut bytes = bcs::to_bytes(txn.inner()).unwrap();
    bytes[0] = 1; // set invalid intent
    let e = bcs::from_bytes::<SenderSignedTransaction>(&bytes).unwrap_err();
    assert!(e.to_string().contains("invalid Intent for Transaction"));

    // ser fails when intent is wrong
    txn.inner_mut().intent_message.intent.scope = IntentScope::TransactionEffects;
    let e = bcs::to_bytes(txn.inner()).unwrap_err();
    assert!(e.to_string().contains("invalid Intent for Transaction"));
}
