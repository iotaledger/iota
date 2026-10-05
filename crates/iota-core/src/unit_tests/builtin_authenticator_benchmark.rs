// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Temporary benchmark for the built-in Ed25519 authenticator.

use std::time::{Duration, Instant};

use iota_sdk_crypto::Signer;
use iota_sdk_types::{
    Address, ClaimAccountTransaction, MoveAuthenticatorV1, ObjectId, ObjectReference, Owner,
    SenderSignedTransaction, SharedObjectReference, SimpleSignature, SmartAccountBuildKind,
    SmartAccountClaim, Transaction, TransactionKind, UserSignature,
    crypto::{Intent, IntentMessage},
};
use iota_transaction_checks::VerifierLimitsSource;
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
    utils::to_sender_signed_transaction,
};

use crate::authority::{
    AuthorityState, ExecutionEnv,
    authority_test_utils::{init_certified_transaction, send_consensus_no_execution},
    test_authority_builder::TestAuthorityBuilder,
};

const NUM_EXEC_TXS: usize = 2_000;
const NUM_SIGNING_ITERS: usize = 20_000;
const WARMUP: usize = 200;

fn gas_coins(owner: Address, count: usize) -> Vec<Object> {
    (0..count)
        .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), owner))
        .collect()
}

fn gas_ref(state: &AuthorityState, id: ObjectId) -> ObjectReference {
    state
        .get_object_store()
        .try_get_object(&id)
        .unwrap()
        .unwrap()
        .object_ref()
}

fn empty_pt_tx(sender: Address, gas: ObjectReference, rgp: u64) -> Transaction {
    // A single cheap command keeps the transaction realistic and identical
    // for the plain and the built-in sender.
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.pure(1u64).unwrap();
    Transaction::new(
        TransactionKind::Programmable(builder.finish()),
        sender,
        gas,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    )
}

fn builtin_signature(
    key: &AccountPrivateKey,
    tx: &Transaction,
    account: SharedObjectReference,
) -> UserSignature {
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx.clone());
    let sig: SimpleSignature = key.sign(&intent_msg.signing_digest());
    let wire = UserSignature::Simple(sig).to_bytes();
    UserSignature::MoveAuthenticator(
        MoveAuthenticatorV1::new_with_shared_account_object(
            vec![CallArg::Pure(bcs::to_bytes(&wire).unwrap())],
            vec![],
            account,
        )
        .into(),
    )
}

fn builtin_signed(
    key: &AccountPrivateKey,
    tx: Transaction,
    account: SharedObjectReference,
) -> TransactionEnvelope {
    let signature = builtin_signature(key, &tx, account);
    TransactionEnvelope::new(SenderSignedTransaction::new(tx, vec![signature]))
}

fn stats(label: &str, mut samples: Vec<Duration>) {
    samples.sort();
    let n = samples.len();
    let total: Duration = samples.iter().sum();
    let mean = total / n as u32;
    let p = |q: f64| samples[((n as f64 - 1.0) * q) as usize];
    println!(
        "BENCH {label}: n={n} mean={:.1}us p50={:.1}us p90={:.1}us p99={:.1}us",
        mean.as_secs_f64() * 1e6,
        p(0.5).as_secs_f64() * 1e6,
        p(0.9).as_secs_f64() * 1e6,
        p(0.99).as_secs_f64() * 1e6,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn builtin_authenticator_benchmark() {
    let (aa_address, aa_key): (Address, AccountPrivateKey) = get_key_pair();
    let (plain_address, plain_key): (Address, AccountPrivateKey) = get_key_pair();

    let mut objects = gas_coins(aa_address, NUM_EXEC_TXS + WARMUP + 2);
    let aa_gas_ids: Vec<ObjectId> = objects.iter().map(|o| o.id()).collect();
    let plain_objects = gas_coins(plain_address, NUM_EXEC_TXS + WARMUP + 1);
    let plain_gas_ids: Vec<ObjectId> = plain_objects.iter().map(|o| o.id()).collect();
    objects.extend(plain_objects);

    let state = TestAuthorityBuilder::new()
        .with_starting_objects(&objects)
        .build()
        .await;
    let rgp = state.reference_gas_price_for_testing().unwrap();
    let epoch_store = state.epoch_store_for_testing();

    // Claim a built-in Ed25519 account whose id is `aa_address`.
    let claim = SmartAccountClaim {
        public_key_scheme: 0,
        public_key_raw_bytes: AsRef::<[u8]>::as_ref(&aa_key.public_key()).to_vec(),
        build_kind: SmartAccountBuildKind::Mutable,
    };
    let claim_tx = Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
        aa_address,
        gas_ref(&state, aa_gas_ids[0]),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let claim_cert =
        init_certified_transaction(to_sender_signed_transaction(claim_tx, &aa_key), &state);
    let (claim_effects, err) = state
        .try_execute_for_test(&claim_cert, ExecutionEnv::new())
        .unwrap();
    assert!(err.is_none(), "claim failed: {err:?}");
    let account_version = claim_effects
        .data()
        .created()
        .into_iter()
        .find_map(|created| match created.owner() {
            Owner::Shared(v) if created.reference().object_id == ObjectId::from(aa_address) => {
                Some(*v)
            }
            _ => None,
        })
        .expect("claimed account must be created as a shared object");
    let account = SharedObjectReference::new(aa_address.into(), account_version, false);

    // --- Signing path: validation checks, including Move authentication. ---
    let signing_tx = builtin_signed(
        &aa_key,
        empty_pt_tx(aa_address, gas_ref(&state, aa_gas_ids[1]), rgp),
        account,
    );
    let signing_tx = epoch_store.verify_transaction(signing_tx).unwrap();
    let plain_signing_tx = epoch_store
        .verify_transaction(to_sender_signed_transaction(
            empty_pt_tx(plain_address, gas_ref(&state, plain_gas_ids[0]), rgp),
            &plain_key,
        ))
        .unwrap();

    // Sanity check: a signature of another transaction must be rejected, so the
    // timed calls below really verify the signature.
    let other_tx = empty_pt_tx(aa_address, gas_ref(&state, aa_gas_ids[2]), rgp);
    let wrong_signing_tx = epoch_store
        .verify_transaction(TransactionEnvelope::new(SenderSignedTransaction::new(
            empty_pt_tx(aa_address, gas_ref(&state, aa_gas_ids[1]), rgp),
            vec![builtin_signature(&aa_key, &other_tx, account)],
        )))
        .unwrap();
    assert!(
        state
            .handle_transaction_validation_checks(
                &wrong_signing_tx,
                &epoch_store,
                &state.config.transaction_deny_config,
                false,
                VerifierLimitsSource::NodeConfig(&state.config.verifier_signing_config),
            )
            .await
            .is_err(),
        "a signature of another transaction must be rejected"
    );

    for (label, tx) in [
        ("signing/builtin", &signing_tx),
        ("signing/plain", &plain_signing_tx),
    ] {
        let mut samples = Vec::with_capacity(NUM_SIGNING_ITERS);
        for i in 0..(NUM_SIGNING_ITERS + WARMUP) {
            let start = Instant::now();
            state
                .handle_transaction_validation_checks(
                    tx,
                    &epoch_store,
                    &state.config.transaction_deny_config,
                    false,
                    VerifierLimitsSource::NodeConfig(&state.config.verifier_signing_config),
                )
                .await
                .unwrap();
            if i >= WARMUP {
                samples.push(start.elapsed());
            }
        }
        stats(label, samples);
    }

    // --- Execution path: execute distinct certificates. ---
    let mut builtin_samples = Vec::with_capacity(NUM_EXEC_TXS);
    let mut builtin_gas = 0u64;
    for i in 0..(NUM_EXEC_TXS + WARMUP) {
        let tx = builtin_signed(
            &aa_key,
            empty_pt_tx(aa_address, gas_ref(&state, aa_gas_ids[i + 2]), rgp),
            account,
        );
        let cert = init_certified_transaction(tx, &state);
        let assigned_versions = send_consensus_no_execution(&state, &cert).await;
        let env = ExecutionEnv::new().with_assigned_versions(assigned_versions);
        let start = Instant::now();
        let (effects, err) = state.try_execute_for_test(&cert, env).unwrap();
        let elapsed = start.elapsed();
        assert!(err.is_none(), "builtin tx failed: {err:?}");
        if i >= WARMUP {
            builtin_samples.push(elapsed);
            builtin_gas = effects.data().gas_cost_summary().computation_cost;
        }
    }
    stats("execution/builtin", builtin_samples);

    let mut plain_samples = Vec::with_capacity(NUM_EXEC_TXS);
    let mut plain_gas = 0u64;
    for i in 0..(NUM_EXEC_TXS + WARMUP) {
        let tx = to_sender_signed_transaction(
            empty_pt_tx(plain_address, gas_ref(&state, plain_gas_ids[i + 1]), rgp),
            &plain_key,
        );
        let cert = init_certified_transaction(tx, &state);
        let start = Instant::now();
        let (effects, err) = state
            .try_execute_for_test(&cert, ExecutionEnv::new())
            .unwrap();
        let elapsed = start.elapsed();
        assert!(err.is_none(), "plain tx failed: {err:?}");
        if i >= WARMUP {
            plain_samples.push(elapsed);
            plain_gas = effects.data().gas_cost_summary().computation_cost;
        }
    }
    stats("execution/plain", plain_samples);
    println!("BENCH gas computation_cost builtin={builtin_gas} plain={plain_gas}");
}
