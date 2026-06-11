// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for every Move package in
//! `examples/move/abstract_iota_accounts/`.
//!
//! For each `max_auth_gas` budget in [`MaxAuthGas`], the test spins up a fresh
//! `TestCluster` with that budget enforced by the protocol config, then for
//! every example package: publishes it, creates an abstract-account instance,
//! and tries to authenticate a transaction signed by that account. The
//! observed outcomes are compared against the matrix in
//! [`expected_outcome`].
//!
//! Two test entry points are exposed:
//! - [`test_abstract_iota_accounts_examples_across_all_max_auth_gas_budgets`]
//!   exercises every variant of [`MaxAuthGas`] (full sweep).
//! - [`test_abstract_iota_accounts_examples_across_network_max_auth_gas_budgets`]
//!   exercises only the fixed budgets that match the `max_auth_gas` values
//!   currently configured for live IOTA networks ([`MaxAuthGas::NETWORKS`]).
//!
//! The dials a developer typically wants to tweak live near the top of the
//! file under "Tunable test parameters": the [`MaxAuthGas`] enum, the
//! [`NUM_OF_CYCLES`] slice for the benchmark scenario, the precomputed
//! `LEAN_IMT_*` fixture data, and the [`expected_outcome`] matrix.

use std::{path::PathBuf, str::FromStr};

use bip32::DerivationPath;
use fastcrypto::{
    ed25519::Ed25519Signature,
    encoding::{Encoding, Hex},
    hash::{HashFunction, Keccak256},
    traits::Authenticator,
};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Argument, Identifier, ObjectId, Owner, TypeTag, crypto::Intent};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_CLOCK_OBJECT_ID, IOTA_CLOCK_OBJECT_SHARED_VERSION, IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{IotaAddress, ObjectRef},
    crypto::SignatureScheme,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt},
    move_authenticator::MoveAuthenticator,
    move_package,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    storage::WriteKind,
    transaction::{
        CallArg, ProgrammableTransaction, SharedObjectRef,
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, Transaction, TransactionData,
        TransactionDataAPI, auth_digest_for_sig,
    },
};
use test_cluster::{TestCluster, TestClusterBuilder};

const EXAMPLES_SUBDIR: &str = "../../examples/move/abstract_iota_accounts";

/// Discrete `max_auth_gas` budgets the test sweeps over. Each variant gets
/// its own `TestCluster` because the `ProtocolConfig` override is global —
/// reusing a cluster across budgets would leak the previous override into
/// the next iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MaxAuthGas {
    /// 10_000 — extremely tight; only the cheapest authenticators fit.
    G10k,
    /// 20_000 — still very tight.
    G20k,
    /// 30_000 — fills the gap between the two tightest budgets and 50k.
    G30k,
    /// 40_000 — fills the gap between the two tightest budgets and 50k.
    G40k,
    /// 50_000 — small but more comfortable.
    G50k,
    /// 250_000 — fixed numeric budget.
    G250k,
    /// 1_000_000 — generous; heavy authenticators that don't rely on Groth16
    /// should fit here.
    G1m,
}

impl MaxAuthGas {
    /// Fixed budgets matching the `max_auth_gas` values currently configured
    /// for live IOTA networks.
    const NETWORKS: &'static [MaxAuthGas] = &[MaxAuthGas::G20k, MaxAuthGas::G250k];

    /// Every fixed numeric budget the test sweeps over.
    const ALL: &'static [MaxAuthGas] = &[
        MaxAuthGas::G10k,
        MaxAuthGas::G20k,
        MaxAuthGas::G30k,
        MaxAuthGas::G40k,
        MaxAuthGas::G50k,
        MaxAuthGas::G250k,
        MaxAuthGas::G1m,
    ];

    /// Resolve the numeric `max_auth_gas` budget for this variant.
    fn as_u64(self) -> u64 {
        match self {
            MaxAuthGas::G10k => 10_000,
            MaxAuthGas::G20k => 20_000,
            MaxAuthGas::G30k => 30_000,
            MaxAuthGas::G40k => 40_000,
            MaxAuthGas::G50k => 50_000,
            MaxAuthGas::G250k => 250_000,
            MaxAuthGas::G1m => 1_000_000,
        }
    }
}

// ---------------------------------------------------------------------------
// --- Tunable test parameters -----------------------------------------------
// ---------------------------------------------------------------------------

/// Loop counts exercised by the `account_for_benchmarks` scenario. The
/// scenario runs `authenticate_super_heavy` once per value, where the
/// Move function performs `N` ed25519 verifications. Use this to probe
/// how many verifications fit inside each `max_auth_gas` budget.
const NUM_OF_CYCLES: &[u64] = &[1, 20, 50, 100];

// Fixture data for the `lean_imt_account` example. The Groth16 verifying
// key, proof points and leaf are tied to a specific account address, and
// that address is in turn derived from a specific mnemonic at one of the
// first 500 BIP44 indices. The values are copied verbatim from the
// example's `README.md` — see that file for how to regenerate them.
const LEAN_IMT_MNEMONIC: &str = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";
const LEAN_IMT_TARGET_ADDRESS: &str =
    "0x6b72f63997aa75e2aff8e7cb119f5507f8b521dade51003fc07c8a4c70f79a70";
const LEAN_IMT_ROOT_HEX: &str = "4b61023a56e6b37edec2ba55c1b3f0cf0f4789431aafd6da10d32de09bb97402";
const LEAN_IMT_DOUBLE_HASHED_PUB_KEY_LEAF: &str =
    "3bdfd5246d42721d0a65eb7700be407b537b71f491cded4e1743b6253e353322";
const LEAN_IMT_VERIFYING_KEY: &str = "bbebdc7c4023eeb6a81fcbbc37613366f4dac6687cbe9abc5d09e4cef8899b173a2747c5442ddb898d34de2429a9b43f86b12aeea35d58ec1d97a009eb2a9c2d34d3f96ebdb7416fbedf83ff29abee30941a380166aac2b2557476f50ffe2094777610b217740ac57c573cbf6af8bce106f7772241dce3406f0b1f2b845570074dd670d78f1c9e0d29fa7113753e384f56775627c9c64dd899566f90b7813301b20482628c99f957ff584f940965bc6b711d377c76b12921e0816b421cae5c098432218eb209bb104559ddc0ad78173ebde47c918c540b82e7e6b658b52bb19e0300000000000000926536117de81e192a1c9bec13bbdfb102852c05911d2ebb306e09706547dfac4dfc834de8b175761425b0dd4080c5c3700a63a4da9548d08ab47ba968d8d09969cdb618703b45502a39c93ce22bbd739c946fba6fd4e10285806ed6de1acaa0";
const LEAN_IMT_PROOF_POINTS: &str = "641a8593665c9e415c6f7f2c57ad992566ee5af86d86f00812541e97ef1fa4182493694830c22645274d619dbdb95886a8cbf23c5f3683dd3b1a39a7953898243eab26e677c31f863201c3339c427d46ebc1390203917f39c8479135f275bf17456182c1f59a574d3390206bd51f8373384a56c407f94f677089d3d0ce9999a3";
// ---------------------------------------------------------------------------
// --- Result / expectation bookkeeping --------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Pass,
    Fail,
    Skipped,
}

struct PackageResult {
    /// Free-form result label. Most scenarios use the package name; the
    /// `account_for_benchmarks` scenario appends `#cycles=N` so each cycle
    /// count exercised gets its own row.
    name: String,
    publish_ok: bool,
    publish_err: Option<String>,
    create_outcome: Outcome,
    create_err: Option<String>,
    authenticate_outcome: Outcome,
    authenticate_err: Option<String>,
}

impl PackageResult {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            publish_ok: false,
            publish_err: None,
            create_outcome: Outcome::Skipped,
            create_err: None,
            authenticate_outcome: Outcome::Skipped,
            authenticate_err: None,
        }
    }
}

struct Expectation {
    authenticate: Outcome,
}

/// Expected `authenticate` outcome per `(package, max_auth_gas)` pair.
///
/// A few cells of the matrix depend on which test runner is used:
/// the same authenticator that aborts with `OUT_OF_GAS` under
/// `cargo simtest` may pass under plain `cargo nextest`, because the
/// two runners drive the validators through different code paths.
/// Those cells are gated on `cfg!(msim)` below.
///
/// To extend or correct this matrix:
/// 1. Add the new package's arm here (or adjust an existing one).
/// 2. Run the test in discovery mode (temporarily disable the `assert_eq!` in
///    `run_across_max_auth_gas_budgets`) and read the actual outcomes from the
///    printed per-budget table.
fn expected_outcome(name: &str, budget: MaxAuthGas) -> Expectation {
    use MaxAuthGas::*;
    use Outcome::{Fail, Pass};

    let simtest = cfg!(msim);

    let authenticate = match name {
        // Plain ed25519 authenticators — pass under every budget.
        "public_key_authentication"
        | "time_locked"
        | "spending_limit"
        | "function_call_keys"
        | "account_multi_auth"
        | "dynamic_multisig_account" => Pass,

        // Signature-driven gas sponsor. The sponsor authenticator builds a
        // ~140-byte message (tx digest + sender auth digest + bcs of the
        // sender's authenticator function info) and runs ed25519_verify over
        // it — the vector-append + bcs::to_bytes + per-byte verify cost
        // doesn't fit the tightest budget under simtest.
        "sponsorship_ed25519" => {
            if simtest && matches!(budget, G10k) {
                Fail
            } else {
                Pass
            }
        }

        // Policy-driven gas sponsor. The sponsor's authenticator reads
        // `expected_package_addr` from the account's cached `package_addr`
        // field (set once at `create` time via `type_name::get`) and compares
        // the module/function names against byte constants, so the per-call
        // PTB scan never calls `type_name::get` or `address::from_ascii_bytes`.
        // Even so, the PTB scan plus the upstream whitelist / allowance checks
        // overrun the tightest budget under simtest — only G10k is too tight.
        "whitelist_sponsorship" => {
            if simtest && matches!(budget, G10k) {
                Fail
            } else {
                Pass
            }
        }

        // ed25519 + keccak Merkle walk. Heavier than the plain variants;
        // under simtest it runs out of gas at the tighter budgets.
        "onesig" => {
            if simtest && matches!(budget, G10k | G20k | G30k) {
                Fail
            } else {
                Pass
            }
        }

        // ed25519 + BN254 Groth16. Under simtest the proof verification
        // overruns every tight budget — only G250k and above fit.
        "lean_imt_account" => {
            if simtest && matches!(budget, G10k | G20k | G30k | G40k | G50k) {
                Fail
            } else {
                Pass
            }
        }

        // Variable-cost benchmark scenario; outcome depends on both the
        // budget and the loop count parsed from the result name.
        n if n.starts_with("account_for_benchmarks#cycles=") => {
            let cycles: u64 = n
                .strip_prefix("account_for_benchmarks#cycles=")
                .and_then(|c| c.parse().ok())
                .unwrap_or_else(|| panic!("could not parse cycles from result name: {n}"));
            expected_super_heavy(cycles, budget)
        }

        _ => panic!("missing expectation for package: {name}"),
    };

    Expectation { authenticate }
}

/// Maximum number of authentication cycles each `max_auth_gas` budget
/// can absorb before `authenticate_super_heavy` aborts with
/// `OUT_OF_GAS`. The values are measured rather than computed because
/// the per-cycle cost is not strictly linear across budgets.
fn expected_super_heavy(cycles: u64, budget: MaxAuthGas) -> Outcome {
    use MaxAuthGas::*;
    use Outcome::{Fail, Pass};

    let max_cycles: u64 = match budget {
        G10k => 3,
        G20k => 7,
        G30k => 10,
        G40k => 14,
        G50k => 17,
        G250k => 89,
        G1m => 209,
    };
    if cycles <= max_cycles { Pass } else { Fail }
}

fn print_one_budget(budget: MaxAuthGas, results: &[PackageResult]) {
    eprintln!();
    eprintln!("--- max_auth_gas = {} ({:?}) ---", budget.as_u64(), budget);
    eprintln!(
        "{:>34}  {:>7}  {:>9}  {:>12}",
        "package", "publish", "create", "authenticate"
    );
    for r in results {
        eprintln!(
            "{:>34}  {:>7}  {:>9}  {:>12}",
            r.name,
            if r.publish_ok { "ok" } else { "FAIL" },
            outcome_str(r.create_outcome),
            outcome_str(r.authenticate_outcome),
        );
    }
    eprintln!();
}

fn print_all_results(groups: &[(MaxAuthGas, Vec<PackageResult>)]) {
    eprintln!();
    eprintln!("=== Abstract IOTA Accounts examples: per-package outcomes ===");
    for (budget, results) in groups {
        print_one_budget(*budget, results);
    }
}

fn outcome_str(o: Outcome) -> &'static str {
    match o {
        Outcome::Pass => "pass",
        Outcome::Fail => "fail",
        Outcome::Skipped => "-",
    }
}

// ---------------------------------------------------------------------------
// --- Test entry point ------------------------------------------------------
// ---------------------------------------------------------------------------

#[ignore]
#[sim_test]
async fn test_abstract_iota_accounts_examples_across_all_max_auth_gas_budgets()
-> Result<(), anyhow::Error> {
    run_across_max_auth_gas_budgets(MaxAuthGas::ALL).await
}

#[ignore]
#[sim_test]
async fn test_abstract_iota_accounts_examples_across_network_max_auth_gas_budgets()
-> Result<(), anyhow::Error> {
    run_across_max_auth_gas_budgets(MaxAuthGas::NETWORKS).await
}

/// Drive the publish-create-authenticate flow across each `max_auth_gas`
/// budget in `budgets`. For every budget the test spawns a fresh
/// `TestCluster` (the `ProtocolConfig` override is global), runs all
/// per-package scenarios, asserts publish/create/authenticate outcomes
/// against [`expected_outcome`], and prints a combined summary at the end.
async fn run_across_max_auth_gas_budgets(budgets: &[MaxAuthGas]) -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Per-budget result groups, kept for the final printed summary.
    let mut groups: Vec<(MaxAuthGas, Vec<PackageResult>)> = Vec::new();

    for &budget in budgets {
        let budget_value = budget.as_u64();

        // Apply the `max_auth_gas` override for this budget. The guard restores
        // the previous value when dropped at the end of this iteration.
        let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
            config.set_max_auth_gas_for_testing(budget_value);
            config
        });

        let test_cluster = TestClusterBuilder::new().build().await;
        let mut env = TestEnvironment::new(test_cluster).await;

        let mut results: Vec<PackageResult> = Vec::new();

        // The order matters only for readability — each scenario is independent.
        // `run_lean_imt_account` mutates the keystore (imports the
        // README's specific keypair), hence the `&mut`.
        results.push(run_public_key_authentication(&env).await);
        results.push(run_time_locked(&env).await);
        results.push(run_spending_limit(&env).await);
        results.push(run_function_call_keys(&env).await);
        results.push(run_dynamic_multisig_account(&env).await);
        results.push(run_onesig(&env).await);
        results.push(run_lean_imt_account(&mut env).await);
        results.push(run_account_multi_auth(&env).await);
        results.push(run_whitelist_sponsorship(&env).await);
        results.push(run_sponsorship_ed25519(&env).await);
        results.extend(run_account_for_benchmarks(&env, NUM_OF_CYCLES).await);

        // Every package must publish successfully — independent of the
        // `max_auth_gas` budget.
        for r in &results {
            assert!(
                r.publish_ok,
                "[budget={:?}] package {} failed to publish: {:?}",
                budget, r.name, r.publish_err
            );
        }

        // Every package's account-creation step MUST succeed — independent
        // of `max_auth_gas`, since account creation does not consume the
        // authenticator gas budget.
        for r in &results {
            assert_eq!(
                r.create_outcome,
                Outcome::Pass,
                "[budget={:?}/{}] account creation must pass. err: {:?}",
                budget,
                r.name,
                r.create_err
            );
        }

        // Authentication outcomes are budget-dependent.
        for r in &results {
            let expected = expected_outcome(&r.name, budget);
            assert_eq!(
                r.authenticate_outcome, expected.authenticate,
                "[budget={:?}/{}] authenticate outcome differs from expected. err: {:?}",
                budget, r.name, r.authenticate_err
            );
        }

        // Print the per-budget table eagerly so partial output is still
        // visible if the test is killed by a timeout in a later iteration.
        print_one_budget(budget, &results);

        groups.push((budget, results));
    }

    print_all_results(&groups);

    Ok(())
}

// ---------------------------------------------------------------------------
// --- Per-package scenarios --------------------------------------------------
// ---------------------------------------------------------------------------

/// `public_key_authentication::public_key_iotaccount::create(pub_key, ref)`
/// + ed25519_authenticator authenticating with `sign(ctx.digest())`.
async fn run_public_key_authentication(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("public_key_authentication");
    let (pkg_id, metadata_ref, _resp) = match env
        .publish_example_with_metadata("public_key_authentication")
        .await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let account_type = type_tag(&pkg_id, "iotaccount", "IOTAccount");

    // Create the account via `public_key_iotaccount::create(pk, ref)`.
    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "public_key_iotaccount",
            "ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        let pk_arg = match b.pure(env.owner_pk_bytes()) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("public_key_iotaccount"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    // Build a simple sender-from-AA transaction and authenticate it.
    let outcome =
        run_simple_auth_ed25519(env, account_ref, pkg_id, AuthCallArgs::ed25519_only()).await;
    match outcome {
        Ok((o, err)) => {
            r.authenticate_outcome = o;
            r.authenticate_err = err;
        }
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("{e:?}"));
        }
    }
    r
}

/// `time_locked::time_locked_iotaccount::create(pk, none, unlock_time, ref)`
/// + `unlock_time_clock_ed25519_authenticator(account, clock, signature, ...)`.
async fn run_time_locked(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("time_locked");
    let (pkg_id, metadata_ref, _resp) = match env.publish_example_with_metadata("time_locked").await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let account_type = type_tag(&pkg_id, "iotaccount", "IOTAccount");

    // unlock_time = 1 → past timestamp, so the clock check will not abort.
    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "time_locked_iotaccount",
            "unlock_time_clock_ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        // create(public_key, none<address>, unlock_time, authenticator, ctx)
        let pk_arg = b.pure(env.owner_pk_bytes()).unwrap();
        let admin_arg = b.pure::<Option<IotaAddress>>(None).unwrap();
        let unlock_time_arg = b.pure(1u64).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("time_locked_iotaccount"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, admin_arg, unlock_time_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let outcome =
        run_simple_auth_ed25519(env, account_ref, pkg_id, AuthCallArgs::clock_then_ed25519()).await;
    match outcome {
        Ok((o, err)) => {
            r.authenticate_outcome = o;
            r.authenticate_err = err;
        }
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("{e:?}"));
        }
    }
    r
}

/// `spending_limit::spending_limit_account::create(pk, limit, ref)`
/// + `ed25519_authenticator(account, signature, ...)`. We exercise a PTB that
///   does NOT call `withdraw_from_balance_reserve`, so the spending-limit
///   branch is exercised with a zero total.
async fn run_spending_limit(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("spending_limit");
    let (pkg_id, metadata_ref, _resp) =
        match env.publish_example_with_metadata("spending_limit").await {
            Ok(v) => {
                r.publish_ok = true;
                v
            }
            Err(e) => {
                r.publish_err = Some(format!("{e:?}"));
                return r;
            }
        };

    let account_type = type_tag(&pkg_id, "spending_limit_account", "SpendingLimitAccount");
    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "spending_limit_account",
            "ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        let pk_arg = b.pure(env.owner_pk_bytes()).unwrap();
        let limit_arg = b.pure(1_000u64).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("spending_limit_account"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, limit_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let outcome =
        run_simple_auth_ed25519(env, account_ref, pkg_id, AuthCallArgs::ed25519_only()).await;
    match outcome {
        Ok((o, err)) => {
            r.authenticate_outcome = o;
            r.authenticate_err = err;
        }
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("{e:?}"));
        }
    }
    r
}

/// `function_call_keys::function_call_keys::create(pk, none, ref)`
/// + `ed25519_authenticator(account, owner_pk, signature, ...)` in OWNER FLOW.
async fn run_function_call_keys(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("function_call_keys");
    let (pkg_id, metadata_ref, _resp) = match env
        .publish_example_with_metadata("function_call_keys")
        .await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let account_type = type_tag(&pkg_id, "iotaccount", "IOTAccount");
    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "function_call_keys",
            "ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        let pk_arg = b.pure(env.owner_pk_bytes()).unwrap();
        let admin_arg = b.pure::<Option<IotaAddress>>(None).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("function_call_keys"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, admin_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let outcome = run_simple_auth_ed25519(
        env,
        account_ref,
        pkg_id,
        AuthCallArgs::pubkey_then_ed25519(env.owner_pk_bytes()),
    )
    .await;
    match outcome {
        Ok((o, err)) => {
            r.authenticate_outcome = o;
            r.authenticate_err = err;
        }
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("{e:?}"));
        }
    }
    r
}

/// `dynamic_multisig_account` — multisig with on-chain approvals.
///
/// The scenario registers a single-member account (threshold 1, weight 1),
/// then submits a separate transaction from the member that pre-proposes
/// (and implicitly approves) the AA transaction's digest. With one
/// approval recorded, the on-chain `approval_authenticator` accepts the
/// AA transaction.
async fn run_dynamic_multisig_account(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("dynamic_multisig_account");
    let (pkg_id, metadata_ref, _resp) = match env
        .publish_example_with_metadata("dynamic_multisig_account")
        .await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let account_type = type_tag(
        &pkg_id,
        "dynamic_multisig_account",
        "DynamicMultisigAccount",
    );

    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "dynamic_multisig_account",
            "approval_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };

        // members = [owner], weights = [1], threshold = 1
        let members_arg = b.pure::<Vec<IotaAddress>>(vec![env.owner]).unwrap();
        let weights_arg = b.pure::<Vec<u64>>(vec![1]).unwrap();
        let threshold_arg = b.pure(1u64).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("dynamic_multisig_account"),
            Identifier::from_static("create"),
            vec![],
            vec![members_arg, weights_arg, threshold_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    // Build the AA transaction we intend to authenticate, so we can compute
    // its digest BEFORE submitting it.
    let aa_sender: IotaAddress = account_ref.object_id.into();
    let rgp = env.test_cluster.get_reference_gas_price().await;
    let gas = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let aa_pt = simple_sender_clock_ptb();
    let aa_tx_data = tx_data_from_pt(env, aa_pt, aa_sender, gas).await;
    let aa_tx_digest = aa_tx_data.digest().into_inner();

    // From the owner (the only member), propose the transaction. The proposer
    // is automatically recorded as the first approver in `Transactions::add`,
    // so this single call satisfies the threshold = 1.
    let propose_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let acc_arg = b
            .obj(CallArg::Shared(SharedObjectRef {
                object_id: account_ref.object_id,
                initial_shared_version: account_ref.version,
                mutable: true,
            }))
            .unwrap();
        let digest_arg = b.pure(aa_tx_digest.to_vec()).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("dynamic_multisig_account"),
            Identifier::from_static("propose_transaction"),
            vec![],
            vec![acc_arg, digest_arg],
        );
        b.finish()
    };

    let propose_tx_data = env
        .test_cluster
        .test_transaction_builder()
        .await
        .programmable(propose_pt)
        .build();
    let propose_tx = env.test_cluster.wallet.sign_transaction(&propose_tx_data);
    match env
        .test_cluster
        .execute_transaction_return_raw_effects(propose_tx)
        .await
    {
        Ok((effects, _)) if effects.status().is_success() => {}
        Ok((effects, _)) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!(
                "propose_transaction failed: {:?}",
                effects.status()
            ));
            return r;
        }
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("propose_transaction submit: {e:?}"));
            return r;
        }
    }

    // Submit the AA tx now that the approval is recorded. The
    // `approval_authenticator` reads `total_approves(ctx.digest()) >=
    // threshold` and passes.
    let auth = match make_move_authenticator(account_ref, vec![]) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("{e:?}"));
            return r;
        }
    };
    let tx = Transaction::from_generic_sig_data(aa_tx_data, vec![auth]);
    let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
    r.authenticate_outcome = outcome;
    r.authenticate_err = err;
    r
}

/// `onesig::account::create(pk, ref)` + `onesig_authenticator(account,
/// merkle_root, proof, signature, ...)`.
async fn run_onesig(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("onesig");
    let (pkg_id, metadata_ref, _resp) = match env.publish_example_with_metadata("onesig").await {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let account_type = type_tag(&pkg_id, "account", "OneSigAccount");
    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "account",
            "onesig_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        let pk_arg = b.pure(env.owner_pk_bytes()).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("account"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    // Mirror the example's README flow: pre-compute three distinct AA
    // transactions (the example funds the AA with three coins and uses each
    // as gas for a different timestamp_ms PTB), build a sorted-pair keccak
    // Merkle tree over their digests, sign the resulting root once, then
    // submit ONE of the three transactions with its corresponding proof.
    let aa_sender: IotaAddress = account_ref.object_id.into();
    let rgp = env.test_cluster.get_reference_gas_price().await;
    let gas1 = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let gas2 = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let gas3 = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let tx1 = tx_data_from_pt(env, simple_sender_clock_ptb(), aa_sender, gas1).await;
    let tx2 = tx_data_from_pt(env, simple_sender_clock_ptb(), aa_sender, gas2).await;
    let tx3 = tx_data_from_pt(env, simple_sender_clock_ptb(), aa_sender, gas3).await;

    let leaves: Vec<Vec<u8>> = vec![
        tx1.digest().into_inner().to_vec(),
        tx2.digest().into_inner().to_vec(),
        tx3.digest().into_inner().to_vec(),
    ];

    let (merkle_root, proofs) = build_sorted_keccak_merkle_tree(&leaves);

    // The authenticator checks `ed25519_verify(sig, pk, merkle_root)`.
    // `sign_hashed` signs the raw 32-byte message verbatim.
    let root_arr: [u8; 32] = merkle_root
        .as_slice()
        .try_into()
        .expect("Keccak256 always produces 32 bytes");
    let signature = env.sign_digest_raw(&root_arr);

    // Authenticate `tx1` with `proofs[0]`. The on-chain authenticator will
    // recompute keccak256(tx1.digest()) and walk the proof up to the root.
    let auth = match make_move_authenticator(
        account_ref,
        vec![
            CallArg::Pure(bcs::to_bytes(&merkle_root).unwrap()),
            CallArg::Pure(bcs::to_bytes(&proofs[0]).unwrap()),
            CallArg::Pure(bcs::to_bytes(&signature).unwrap()),
        ],
    ) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_err = Some(format!("{e:?}"));
            r.authenticate_outcome = Outcome::Fail;
            return r;
        }
    };

    let tx = Transaction::from_generic_sig_data(tx1, vec![auth]);
    let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
    r.authenticate_outcome = outcome;
    r.authenticate_err = err;
    r
}

/// `lean_imt_account` — account guarded by a Groth16 (BN254) proof that
/// the signer's public key is a leaf in a precomputed LeanIMT.
///
/// The proof is tied to a specific public key; this scenario derives
/// that exact keypair from [`LEAN_IMT_MNEMONIC`] (scanning BIP44 indices
/// to find the address that matches [`LEAN_IMT_TARGET_ADDRESS`]),
/// creates the account with the precomputed Merkle root, then
/// authenticates using the proof bytes copied from the example README.
async fn run_lean_imt_account(env: &mut TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("lean_imt_account");
    let (pkg_id, metadata_ref, _resp) =
        match env.publish_example_with_metadata("lean_imt_account").await {
            Ok(v) => {
                r.publish_ok = true;
                v
            }
            Err(e) => {
                r.publish_err = Some(format!("{e:?}"));
                return r;
            }
        };

    // Find — and import into the keystore — the keypair derived from
    // `LEAN_IMT_MNEMONIC` whose IOTA address equals `LEAN_IMT_TARGET_ADDRESS`.
    // `generate_addresses.rs` generates the first 500 addresses; we scan
    // those indices and remove the ones that don't match to keep the
    // keystore clean.
    let target_addr = IotaAddress::from_str(LEAN_IMT_TARGET_ADDRESS).unwrap();
    let mut signer: Option<IotaAddress> = None;
    for i in 0..500u32 {
        let path = DerivationPath::from_str(&format!("m/44'/4218'/0'/0'/{i}'")).unwrap();
        let keystore = env.test_cluster.wallet.config_mut().keystore_mut();
        let addr = match keystore.import_from_mnemonic(
            LEAN_IMT_MNEMONIC,
            SignatureScheme::ED25519,
            Some(path),
            None,
        ) {
            Ok(a) => a,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("mnemonic import at index {i}: {e:?}"));
                return r;
            }
        };
        if addr == target_addr {
            signer = Some(addr);
            break;
        }
        let _ = keystore.remove_key(&addr);
    }
    let signer = match signer {
        Some(a) => a,
        None => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!(
                "could not derive {LEAN_IMT_TARGET_ADDRESS} from mnemonic within 500 indices"
            ));
            return r;
        }
    };
    let signer_pk = env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&signer)
        .expect("signer key must exist in keystore")
        .public()
        .as_ref()
        .to_vec();

    // Create the AA with the README's pre-computed LeanIMT root.
    let account_type = type_tag(&pkg_id, "lean_imt_account", "LeanIMTAccount");
    let root = Hex::decode(LEAN_IMT_ROOT_HEX).expect("README root hex is valid");
    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "lean_imt_account",
            "secret_ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("{e:?}"));
                return r;
            }
        };
        let root_arg = b.pure(root).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("lean_imt_account"),
            Identifier::from_static("create"),
            vec![],
            vec![root_arg, auth_ref],
        );
        b.finish()
    };

    let account_ref = match create_account_with_pt(env, pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("{e:?}"));
            return r;
        }
    };

    // Build the AA tx and sign its digest with the matching key.
    let aa_sender: IotaAddress = account_ref.object_id.into();
    let rgp = env.test_cluster.get_reference_gas_price().await;
    let gas = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let pt = simple_sender_clock_ptb();
    let tx_data = tx_data_from_pt(env, pt, aa_sender, gas).await;
    let tx_digest = tx_data.digest().into_inner();
    let signature_full = env
        .test_cluster
        .wallet
        .config()
        .keystore()
        .sign_hashed(&signer, &tx_digest)
        .expect("ed25519 sign should not fail");
    // `Signature::as_ref()` returns `flag || sig || pk` — keep only the
    // raw 64-byte signature, which is what the Move authenticator expects.
    let sig_bytes = signature_full.as_ref();
    assert!(sig_bytes.len() > Ed25519Signature::LENGTH);
    let signature: Vec<u8> = sig_bytes[1..1 + Ed25519Signature::LENGTH].to_vec();

    let leaf = Hex::decode(LEAN_IMT_DOUBLE_HASHED_PUB_KEY_LEAF).unwrap();
    let pvk = Hex::decode(LEAN_IMT_VERIFYING_KEY).unwrap();
    let proof_points = Hex::decode(LEAN_IMT_PROOF_POINTS).unwrap();

    let auth = match make_move_authenticator(
        account_ref,
        vec![
            CallArg::Pure(bcs::to_bytes(&signature).unwrap()),
            CallArg::Pure(bcs::to_bytes(&signer_pk).unwrap()),
            CallArg::Pure(bcs::to_bytes(&leaf).unwrap()),
            CallArg::Pure(bcs::to_bytes(&pvk).unwrap()),
            CallArg::Pure(bcs::to_bytes(&proof_points).unwrap()),
        ],
    ) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_err = Some(format!("{e:?}"));
            r.authenticate_outcome = Outcome::Fail;
            return r;
        }
    };

    let tx = Transaction::from_generic_sig_data(tx_data, vec![auth]);
    let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
    r.authenticate_outcome = outcome;
    r.authenticate_err = err;
    r
}

/// `account_multi_auth` — the package's `init` shares an empty `Account`
/// at publish time, then `link_auth` is called separately to attach an
/// authenticator function to it. The example's authenticator takes a
/// mix of typed call args (u64, vector<u8>, nested vector, String,
/// Option, &Clock) and asserts each against a fixed value; the
/// scenario constructs those args verbatim so the assertions succeed.
async fn run_account_multi_auth(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("account_multi_auth");
    let (pkg_id, metadata_ref, resp) = match env
        .publish_example_with_metadata("account_multi_auth")
        .await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    // Locate the shared `Account` object created by `init` at publish time.
    let account_type = type_tag(&pkg_id, "account", "Account");
    let account_ref = match find_created_shared_in_response(&resp, &account_type) {
        Some(r) => r,
        None => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!(
                "no Created shared object of type {account_type} in publish response"
            ));
            return r;
        }
    };

    // Wire the authenticator with `link_auth`. This is the "create" step.
    let link_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let acc_arg = b
            .obj(CallArg::Shared(SharedObjectRef {
                object_id: account_ref.object_id,
                initial_shared_version: account_ref.version,
                mutable: true,
            }))
            .unwrap();
        let pkg_arg = b.obj(CallArg::ImmutableOrOwned(metadata_ref)).unwrap();
        let module_arg = b.pure("account").unwrap();
        let fn_arg = b.pure("authenticate").unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("account"),
            Identifier::from_static("link_auth"),
            vec![],
            vec![acc_arg, pkg_arg, module_arg, fn_arg],
        );
        b.finish()
    };

    let tx_data = env
        .test_cluster
        .test_transaction_builder()
        .await
        .programmable(link_pt)
        .build();
    let tx = env.test_cluster.wallet.sign_transaction(&tx_data);
    match env
        .test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await
    {
        Ok((effects, _)) if effects.status().is_success() => {
            r.create_outcome = Outcome::Pass;
        }
        Ok((effects, _)) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("link_auth failed: {:?}", effects.status()));
            return r;
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("link_auth submit failed: {e:?}"));
            return r;
        }
    }

    // NOTE: For shared objects, `initial_shared_version` is the version at
    // which the object FIRST became shared (i.e. publish-time for this
    // example). We therefore keep using the original `account_ref` even
    // though `link_auth` bumped the object's current version.

    // Build a trivial PTB sent FROM the AA and authenticate it with the five
    // magic auth-call-args plus the shared Clock.
    let aa_sender: IotaAddress = account_ref.object_id.into();
    let rgp = env.test_cluster.get_reference_gas_price().await;
    let gas = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;
    let pt = simple_sender_clock_ptb();
    let tx_data = tx_data_from_pt(env, pt, aa_sender, gas).await;

    let extra_args = vec![
        CallArg::Pure(bcs::to_bytes(&42u64).unwrap()),
        CallArg::Pure(bcs::to_bytes(&vec![0xCA_u8, 0xFE]).unwrap()),
        CallArg::Pure(bcs::to_bytes(&vec![vec![0xAA_u8], vec![0xBB_u8, 0xCC]]).unwrap()),
        CallArg::Pure(bcs::to_bytes(&"test".to_string()).unwrap()),
        CallArg::Pure(bcs::to_bytes(&Some(vec![0xDE_u8, 0xAD])).unwrap()),
        CallArg::Shared(SharedObjectRef {
            object_id: IOTA_CLOCK_OBJECT_ID,
            initial_shared_version: IOTA_CLOCK_OBJECT_SHARED_VERSION,
            mutable: false,
        }),
    ];

    let auth = match make_move_authenticator(account_ref, extra_args) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("{e:?}"));
            return r;
        }
    };
    let tx = Transaction::from_generic_sig_data(tx_data, vec![auth]);
    let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
    r.authenticate_outcome = outcome;
    r.authenticate_err = err;
    r
}

/// `whitelist_sponsorship` — policy-driven gas sponsor.
///
/// Publishes the WLS package and, separately, `public_key_authentication`
/// so we have a real sender authenticator function to whitelist. Creates a
/// sender `IOTAccount` (ed25519-authenticated) and a sponsor
/// `WhitelistSponsorshipAccount`. The owner (= admin) then whitelists the
/// sender's authenticator function and grants the sender a gas allowance.
/// Finally we issue a sponsored transaction:
///
/// - **sender** = the `IOTAccount`, authenticating with `ed25519(sig over
///   tx_digest)`;
/// - **sponsor** = the `WhitelistSponsorshipAccount`, authenticating with an
///   empty-call-args `MoveAuthenticator` (the sponsor's `authenticator` takes
///   only `&AuthContext` and `&TxContext`);
/// - the PTB contains a `deduct_user_gas_allowance` move call so the sponsor
///   authenticator's PTB scan accepts it.
///
/// The scenario expects every step — including the validator-side run of the
/// sponsor authenticator — to succeed under all `max_auth_gas` budgets.
async fn run_whitelist_sponsorship(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("whitelist_sponsorship");

    // Publish the WLS sponsor package.
    let (wls_pkg_id, wls_metadata_ref, _resp) = match env
        .publish_example_with_metadata("whitelist_sponsorship")
        .await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    // Publish the sender package — we use `public_key_authentication`'s
    // `IOTAccount` + ed25519 authenticator as a concrete sender.
    let (sender_pkg_id, sender_metadata_ref, _) = match env
        .publish_example_with_metadata("public_key_authentication")
        .await
    {
        Ok(v) => v,
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("publish public_key_authentication: {e:?}"));
            return r;
        }
    };

    let sender_account_type = type_tag(&sender_pkg_id, "iotaccount", "IOTAccount");

    // 1. Create the sender IOTAccount via `public_key_iotaccount::create(pk, ref)`.
    let sender_create_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let sender_auth_ref = match build_auth_function_ref_v1(
            &mut b,
            sender_account_type.clone(),
            sender_metadata_ref,
            "public_key_iotaccount",
            "ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("sender auth ref: {e:?}"));
                return r;
            }
        };
        let pk_arg = b.pure(env.owner_pk_bytes()).unwrap();
        b.programmable_move_call(
            sender_pkg_id,
            Identifier::from_static("public_key_iotaccount"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, sender_auth_ref],
        );
        b.finish()
    };
    let sender_account_ref = match create_account_with_pt(env, sender_create_pt).await {
        Ok(v) => v,
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("create sender: {e:?}"));
            return r;
        }
    };

    // 2. Create the WLS sponsor account with the owner as admin.
    let sponsor_create_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let sponsor_type = type_tag(
            &wls_pkg_id,
            "whitelist_sponsorship_account",
            "WhitelistSponsorshipAccount",
        );
        let sponsor_auth_ref = match build_auth_function_ref_v1(
            &mut b,
            sponsor_type,
            wls_metadata_ref,
            "whitelist_sponsorship_authentication",
            "authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("sponsor auth ref: {e:?}"));
                return r;
            }
        };
        let admin_arg = b.pure(env.owner).unwrap();
        b.programmable_move_call(
            wls_pkg_id,
            Identifier::from_static("whitelist_sponsorship_account"),
            Identifier::from_static("create"),
            vec![],
            vec![admin_arg, sponsor_auth_ref],
        );
        b.finish()
    };
    let sponsor_account_ref = match create_account_with_pt(env, sponsor_create_pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("create sponsor: {e:?}"));
            return r;
        }
    };

    let sender_addr: IotaAddress = sender_account_ref.object_id.into();
    let sponsor_addr: IotaAddress = sponsor_account_ref.object_id.into();
    let rgp = env.test_cluster.get_reference_gas_price().await;
    let gas_budget = rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE;
    let allowance: u64 = gas_budget.saturating_mul(2);

    // 3. Admin (= owner) whitelists the sender's `ed25519_authenticator` and grants
    //    the sender a gas allowance. Both calls take `&mut sponsor`.
    let admin_setup_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let sponsor_arg = b
            .obj(CallArg::Shared(SharedObjectRef {
                object_id: sponsor_account_ref.object_id,
                initial_shared_version: sponsor_account_ref.version,
                mutable: true,
            }))
            .unwrap();
        let sender_auth_ref_for_whitelist = match build_auth_function_ref_v1(
            &mut b,
            sender_account_type.clone(),
            sender_metadata_ref,
            "public_key_iotaccount",
            "ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.authenticate_outcome = Outcome::Fail;
                r.authenticate_err = Some(format!("admin: build sender ref: {e:?}"));
                return r;
            }
        };
        b.programmable_move_call(
            wls_pkg_id,
            Identifier::from_static("whitelist_sponsorship_account"),
            Identifier::from_static("add_authenticator_function"),
            vec![sender_account_type.clone()],
            vec![sponsor_arg, sender_auth_ref_for_whitelist],
        );
        let user_arg = b.pure(sender_addr).unwrap();
        let allowance_arg = b.pure(allowance).unwrap();
        b.programmable_move_call(
            wls_pkg_id,
            Identifier::from_static("whitelist_sponsorship_account"),
            Identifier::from_static("add_user_gas_allowance"),
            vec![],
            vec![sponsor_arg, user_arg, allowance_arg],
        );
        b.finish()
    };
    let admin_setup_outcome = async {
        let tx_data = env
            .test_cluster
            .test_transaction_builder()
            .await
            .programmable(admin_setup_pt)
            .build();
        let tx = env.test_cluster.wallet.sign_transaction(&tx_data);
        let (effects, _) = env
            .test_cluster
            .execute_transaction_return_raw_effects(tx)
            .await?;
        if !effects.status().is_success() {
            anyhow::bail!("admin setup failed: {:?}", effects.status());
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    if let Err(e) = admin_setup_outcome {
        r.authenticate_outcome = Outcome::Fail;
        r.authenticate_err = Some(format!("admin setup: {e:?}"));
        return r;
    }

    // 4. Build the sponsored PTB: a trivial sender action + the required
    //    `deduct_user_gas_allowance(sponsor)` move call (it reads the sender and
    //    gas budget from the `TxContext`) so the sponsor authenticator's PTB scan
    //    finds and accepts it.
    let sponsor_gas = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;

    let sponsored_pt = {
        let mut b = ProgrammableTransactionBuilder::new();

        // Put the sponsor's `deduct_user_gas_allowance` call FIRST so the sponsor
        // authenticator's PTB scan matches on the first command and the
        // subsequent commands are cheap byte-compares.
        let sponsor_arg = b
            .obj(CallArg::Shared(SharedObjectRef {
                object_id: sponsor_account_ref.object_id,
                initial_shared_version: sponsor_account_ref.version,
                mutable: true,
            }))
            .unwrap();
        b.programmable_move_call(
            wls_pkg_id,
            Identifier::from_static("whitelist_sponsorship_account"),
            Identifier::from_static("deduct_user_gas_allowance"),
            vec![],
            vec![sponsor_arg],
        );

        let clock = b
            .obj(CallArg::Shared(SharedObjectRef {
                object_id: IOTA_CLOCK_OBJECT_ID,
                initial_shared_version: IOTA_CLOCK_OBJECT_SHARED_VERSION,
                mutable: false,
            }))
            .unwrap();
        b.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::from_static("clock"),
            Identifier::from_static("timestamp_ms"),
            vec![],
            vec![clock],
        );

        b.finish()
    };

    let tx_data = TransactionData::new_programmable_allow_sponsor(
        sender_addr,
        vec![sponsor_gas],
        sponsored_pt,
        gas_budget,
        rgp,
        sponsor_addr,
    );
    let tx_digest = tx_data.digest().into_inner();
    let signature = env.sign_digest_raw(&tx_digest);

    let sender_auth = match make_move_authenticator(
        sender_account_ref,
        vec![CallArg::Pure(bcs::to_bytes(&signature).unwrap())],
    ) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("sender authenticator: {e:?}"));
            return r;
        }
    };
    // The sponsor authenticator takes no user-facing inputs; pass an empty
    // extra-args list. This is the path enabled by the `signing.rs` refactor:
    // an AA whose authenticator has no user inputs is signed via a
    // `MoveAuthenticator` with empty call args.
    let sponsor_auth = match make_move_authenticator(sponsor_account_ref, vec![]) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("sponsor authenticator: {e:?}"));
            return r;
        }
    };

    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_auth, sponsor_auth]);
    let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
    r.authenticate_outcome = outcome;
    r.authenticate_err = err;
    r
}

/// `public_key_authentication::public_key_iotaccount::sponsorship_ed25519_authenticator`
/// — signature-driven gas sponsor.
///
/// Publishes the `public_key_authentication` package once and creates two
/// `IOTAccount`s on top of it:
///
/// - **sender** — bound to `ed25519_authenticator`. Signs over `ctx.digest()`.
/// - **sponsor** — bound to `sponsorship_ed25519_authenticator`. Signs over
///   `ctx.digest() || auth_ctx.sender_auth_digest() ||
///   bcs(auth_ctx.sender_authenticator_function_info_v1())`, matching the
///   helper [`public_key_authentication::authenticate_ed25519_for_sponsorship`].
///
/// The test reconstructs that exact byte sequence off-chain (the sender
/// `MoveAuthenticator`'s digest, plus the BCS encoding of the sender's
/// authenticator function info) and signs it with the owner's ed25519 key,
/// then submits the sponsored transaction with both `MoveAuthenticator`s.
async fn run_sponsorship_ed25519(env: &TestEnvironment) -> PackageResult {
    let mut r = PackageResult::new("sponsorship_ed25519");
    let (pkg_id, metadata_ref, _resp) = match env
        .publish_example_with_metadata("public_key_authentication")
        .await
    {
        Ok(v) => {
            r.publish_ok = true;
            v
        }
        Err(e) => {
            r.publish_err = Some(format!("{e:?}"));
            return r;
        }
    };

    let account_type = type_tag(&pkg_id, "iotaccount", "IOTAccount");

    // Only the sponsor is an abstract account in this scenario — the sender
    // is a regular keypair-backed address (`env.owner`) that signs the
    // transaction with a standard `GenericSignature::Signature`.
    let sponsor_create_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let auth_ref = match build_auth_function_ref_v1(
            &mut b,
            account_type.clone(),
            metadata_ref,
            "public_key_iotaccount",
            "sponsorship_ed25519_authenticator",
        ) {
            Ok(v) => v,
            Err(e) => {
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(format!("sponsor create pt: {e:?}"));
                return r;
            }
        };
        let pk_arg = b.pure(env.owner_pk_bytes()).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("public_key_iotaccount"),
            Identifier::from_static("create"),
            vec![],
            vec![pk_arg, auth_ref],
        );
        b.finish()
    };
    let sponsor_account_ref = match create_account_with_pt(env, sponsor_create_pt).await {
        Ok(v) => {
            r.create_outcome = Outcome::Pass;
            v
        }
        Err(e) => {
            r.create_outcome = Outcome::Fail;
            r.create_err = Some(format!("create sponsor: {e:?}"));
            return r;
        }
    };

    // Build the sponsored transaction. The PTB body itself is trivial — only
    // the two authenticators (regular sender signature + AA sponsor) matter
    // for this scenario.
    let sender_addr: IotaAddress = env.owner;
    let sponsor_addr: IotaAddress = sponsor_account_ref.object_id.into();
    let rgp = env.test_cluster.get_reference_gas_price().await;
    let sponsor_gas = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor_addr)
        .await;
    let gas_budget = rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE;

    let pt = simple_sender_clock_ptb();
    let tx_data = TransactionData::new_programmable_allow_sponsor(
        sender_addr,
        vec![sponsor_gas],
        pt,
        gas_budget,
        rgp,
        sponsor_addr,
    );
    let tx_digest = tx_data.digest().into_inner();

    // Sender: standard `GenericSignature::Signature` (ed25519 over the
    // intent-wrapped TransactionData) — NOT a `MoveAuthenticator`. So
    // `auth_ctx.sender_authenticator_function_info_v1()` is `None` on-chain.
    let sender_auth = GenericSignature::Signature(
        env.test_cluster
            .wallet
            .config()
            .keystore()
            .sign_secure(&env.owner, &tx_data, Intent::iota_transaction())
            .expect("sender ed25519 sign should not fail"),
    );

    // Reconstruct the byte sequence `authenticate_ed25519_for_sponsorship`
    // verifies against. For a non-AA sender,
    // `sender_authenticator_function_info_v1()` is `None`, so the Move helper
    // skips the third segment entirely:
    //
    //   msg = ctx.digest()                  // 32 bytes
    //      || auth_ctx.sender_auth_digest() // 32 bytes — Blake2b256(sender_sig.as_ref())
    let sender_auth_digest = match auth_digest_for_sig(&sender_auth) {
        Ok(d) => d,
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("sender auth digest: {e:?}"));
            return r;
        }
    };
    let mut sponsor_msg = Vec::with_capacity(32 + 32);
    sponsor_msg.extend_from_slice(&tx_digest);
    sponsor_msg.extend_from_slice(sender_auth_digest.as_bytes());

    // Sign the constructed message with the owner's ed25519 key. `sign_hashed`
    // here just performs standard `Ed25519::sign(msg)` over the raw bytes — no
    // intent wrapping — which is exactly what
    // `ed25519::ed25519_verify(sig, pk, &msg)` checks on-chain.
    let sponsor_signature: Vec<u8> = {
        let raw = env
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_hashed(&env.owner, &sponsor_msg)
            .expect("ed25519 sign should not fail");
        let bytes = raw.as_ref();
        bytes[1..1 + Ed25519Signature::LENGTH].to_vec()
    };
    let sponsor_auth = match make_move_authenticator(
        sponsor_account_ref,
        vec![CallArg::Pure(bcs::to_bytes(&sponsor_signature).unwrap())],
    ) {
        Ok(v) => v,
        Err(e) => {
            r.authenticate_outcome = Outcome::Fail;
            r.authenticate_err = Some(format!("sponsor authenticator: {e:?}"));
            return r;
        }
    };

    let tx = Transaction::from_generic_sig_data(tx_data, vec![sender_auth, sponsor_auth]);
    let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
    r.authenticate_outcome = outcome;
    r.authenticate_err = err;
    r
}

/// `account_for_benchmarks` — variable-cost authenticator used to probe
/// where each `max_auth_gas` budget runs out of gas.
///
/// The Move `authenticate_super_heavy` function takes 122 immutable
/// `BenchObject` references and loops `num_of_cycles` ed25519 verifies.
/// The scenario publishes the package, creates the account, mints the
/// 122 frozen `BenchObject`s once, then runs the authenticator once per
/// value in `cycle_counts` — emitting one [`PackageResult`] per cycle
/// count (labelled `account_for_benchmarks#cycles=N`).
async fn run_account_for_benchmarks(
    env: &TestEnvironment,
    cycle_counts: &[u64],
) -> Vec<PackageResult> {
    let make_label = |cycles: u64| format!("account_for_benchmarks#cycles={cycles}");

    let placeholder = |cycles: u64, error: String, where_: &str| -> PackageResult {
        let mut r = PackageResult::new(make_label(cycles));
        match where_ {
            "publish" => r.publish_err = Some(error),
            "create" => {
                r.publish_ok = true;
                r.create_outcome = Outcome::Fail;
                r.create_err = Some(error);
            }
            "auth" => {
                r.publish_ok = true;
                r.create_outcome = Outcome::Pass;
                r.authenticate_outcome = Outcome::Fail;
                r.authenticate_err = Some(error);
            }
            _ => unreachable!("unknown placeholder slot"),
        }
        r
    };

    // 1. Publish the package and capture the metadata ref.
    let (pkg_id, metadata_ref, _resp) = match env
        .publish_example_with_metadata("account_for_benchmarks")
        .await
    {
        Ok(v) => v,
        Err(e) => {
            let err = format!("{e:?}");
            return cycle_counts
                .iter()
                .map(|&c| placeholder(c, err.clone(), "publish"))
                .collect();
        }
    };

    // 2. Create the account: `create(metadata, "abstract_account",
    //    "authenticate_super_heavy", pk)`.
    let pk_bytes = env.owner_pk_bytes();
    let create_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let meta = b.obj(CallArg::ImmutableOrOwned(metadata_ref)).unwrap();
        let module = b.pure("abstract_account").unwrap();
        let fn_name = b.pure("authenticate_super_heavy").unwrap();
        let pk = b.pure(pk_bytes.clone()).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("abstract_account"),
            Identifier::from_static("create"),
            vec![],
            vec![meta, module, fn_name, pk],
        );
        b.finish()
    };
    let account_ref = match create_account_with_pt(env, create_pt).await {
        Ok(v) => v,
        Err(e) => {
            let err = format!("{e:?}");
            return cycle_counts
                .iter()
                .map(|&c| placeholder(c, err.clone(), "create"))
                .collect();
        }
    };

    // 3. Mint the 122 frozen `BenchObject`s that the authenticator takes as inputs.
    //    Freezing them (rather than sharing) lets the auth call pass each as
    //    `CallArg::ImmutableOrOwned`.
    let bench_pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let amount = b.pure(122u64).unwrap();
        let is_shared = b.pure(false).unwrap();
        b.programmable_move_call(
            pkg_id,
            Identifier::from_static("abstract_account"),
            Identifier::from_static("create_bench_objects"),
            vec![],
            vec![amount, is_shared],
        );
        b.finish()
    };
    let bench_tx_data = env
        .test_cluster
        .test_transaction_builder()
        .await
        .programmable(bench_pt)
        .build();
    let bench_tx = env.test_cluster.wallet.sign_transaction(&bench_tx_data);
    let (bench_effects, _) = match env
        .test_cluster
        .execute_transaction_return_raw_effects(bench_tx)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            let err = format!("create_bench_objects submit: {e:?}");
            return cycle_counts
                .iter()
                .map(|&c| placeholder(c, err.clone(), "create"))
                .collect();
        }
    };
    if !bench_effects.status().is_success() {
        let err = format!("create_bench_objects status: {:?}", bench_effects.status());
        return cycle_counts
            .iter()
            .map(|&c| placeholder(c, err.clone(), "create"))
            .collect();
    }
    let bench_refs: Vec<ObjectRef> = bench_effects
        .all_changed_objects()
        .into_iter()
        .filter_map(|(oref, owner, kind)| {
            (matches!(kind, WriteKind::Create) && matches!(owner, Owner::Immutable)).then_some(oref)
        })
        .collect();
    assert_eq!(
        bench_refs.len(),
        122,
        "expected exactly 122 frozen BenchObjects, got {}",
        bench_refs.len()
    );

    // 4. Run `authenticate_super_heavy` for each requested cycle count. The auth
    //    call args are: `[u64(cycles), signature, 122 × &BenchObject]`.
    let mut results = Vec::with_capacity(cycle_counts.len());
    for &cycles in cycle_counts {
        let mut r = PackageResult::new(make_label(cycles));
        r.publish_ok = true;
        r.create_outcome = Outcome::Pass;

        let aa_sender: IotaAddress = account_ref.object_id.into();
        let rgp = env.test_cluster.get_reference_gas_price().await;
        let gas = env
            .test_cluster
            .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
            .await;
        let pt = simple_sender_clock_ptb();
        let tx_data = tx_data_from_pt(env, pt, aa_sender, gas).await;
        let tx_digest = tx_data.digest().into_inner();
        // The on-chain authenticator passes `signature` straight to
        // `ed25519_verify`, so we send the raw 64-byte ed25519 signature.
        let signature = env.sign_digest_raw(&tx_digest);

        let mut extra_args: Vec<CallArg> = Vec::with_capacity(2 + bench_refs.len());
        extra_args.push(CallArg::Pure(bcs::to_bytes(&cycles).unwrap()));
        extra_args.push(CallArg::Pure(bcs::to_bytes(&signature).unwrap()));
        for &bench_ref in &bench_refs {
            extra_args.push(CallArg::ImmutableOrOwned(bench_ref));
        }

        let auth = match make_move_authenticator(account_ref, extra_args) {
            Ok(v) => v,
            Err(e) => {
                r.authenticate_outcome = Outcome::Fail;
                r.authenticate_err = Some(format!("{e:?}"));
                results.push(r);
                continue;
            }
        };
        let tx = Transaction::from_generic_sig_data(tx_data, vec![auth]);
        let (outcome, err) = execute_aa_tx_outcome(env, tx).await;
        r.authenticate_outcome = outcome;
        r.authenticate_err = err;
        results.push(r);
    }

    results
}

// ---------------------------------------------------------------------------
// --- Shared test environment ------------------------------------------------
// ---------------------------------------------------------------------------

struct TestEnvironment {
    test_cluster: TestCluster,
    /// Address of the first keystore key — used as the sender for publish /
    /// create-account transactions.
    owner: IotaAddress,
}

impl TestEnvironment {
    async fn new(test_cluster: TestCluster) -> Self {
        let owner = test_cluster
            .wallet
            .config()
            .keystore()
            .addresses()
            .first()
            .copied()
            .expect("wallet must have at least one account");
        Self {
            test_cluster,
            owner,
        }
    }

    /// Path to an example package inside the repo.
    fn example_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push(EXAMPLES_SUBDIR);
        p.push(name);
        p
    }

    /// Publishes the example package at
    /// `examples/move/abstract_iota_accounts/{name}`, bundling any
    /// unpublished local dependencies into the same package.
    ///
    /// Returns the published package id, its derived `PackageMetadataV1`
    /// object reference, and the full publish response so callers can mine it
    /// for additional `init`-time objects. Errors if the package has no
    /// `PackageMetadataV1` object — i.e. no `#[authenticator]` function —
    /// since every package the tests exercise here must expose at least one.
    async fn publish_example_with_metadata(
        &self,
        name: &str,
    ) -> anyhow::Result<(
        ObjectId,
        ObjectRef,
        iota_json_rpc_types::IotaTransactionBlockResponse,
    )> {
        let path = Self::example_path(name);
        let (sender, gas) = self
            .test_cluster
            .wallet
            .get_one_gas_object()
            .await?
            .ok_or_else(|| anyhow::anyhow!("no gas object available for publish"))?;
        let rgp = self.test_cluster.get_reference_gas_price().await;
        let tx = self.test_cluster.wallet.sign_transaction(
            &TestTransactionBuilder::new(sender, gas, rgp)
                .publish_with_deps(path)
                .build(),
        );
        let resp = self
            .test_cluster
            .wallet
            .execute_transaction_must_succeed(tx)
            .await;

        let pkg_id = iota_json_rpc_types::get_new_package_obj_from_response(&resp)
            .ok_or_else(|| anyhow::anyhow!("no Published object change in response"))?
            .object_id;
        let metadata_id = move_package::derive_package_metadata_id(pkg_id);
        let metadata_ref = self
            .test_cluster
            .get_object_from_fullnode_store(&metadata_id)
            .await
            .map(|obj| obj.object_ref())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "package {name} has no PackageMetadataV1 — missing #[authenticator]?"
                )
            })?;
        Ok((pkg_id, metadata_ref, resp))
    }

    /// Fetch the owner's raw ed25519 public-key bytes (32 bytes — no flag).
    fn owner_pk_bytes(&self) -> Vec<u8> {
        let key = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .get_key(&self.owner)
            .expect("owner key must exist in keystore");
        key.public().as_ref().to_vec()
    }

    /// Sign an arbitrary 32-byte digest with the owner's ed25519 key and return
    /// the raw 64-byte signature (no flag, no public key).
    fn sign_digest_raw(&self, digest: &[u8; 32]) -> Vec<u8> {
        let sig = self
            .test_cluster
            .wallet
            .config()
            .keystore()
            .sign_hashed(&self.owner, digest)
            .expect("ed25519 sign should not fail");
        // `Signature::as_ref()` yields `flag || sig || pk`.
        let bytes = sig.as_ref();
        assert!(bytes.len() > Ed25519Signature::LENGTH);
        bytes[1..1 + Ed25519Signature::LENGTH].to_vec()
    }
}

// ---------------------------------------------------------------------------
// --- Small helpers used across packages -------------------------------------
// ---------------------------------------------------------------------------

/// Build a `0x2::authenticator_function::create_auth_function_ref_v1<T>(...)`
/// PTB call and return the resulting `Argument::Result(...)`.
fn build_auth_function_ref_v1(
    builder: &mut ProgrammableTransactionBuilder,
    account_type: TypeTag,
    package_metadata_ref: ObjectRef,
    module_name: &str,
    function_name: &str,
) -> anyhow::Result<Argument> {
    let args = vec![
        builder.obj(CallArg::ImmutableOrOwned(package_metadata_ref))?,
        builder.pure(module_name)?,
        builder.pure(function_name)?,
    ];
    let r = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("authenticator_function"),
        Identifier::from_static("create_auth_function_ref_v1"),
        vec![account_type],
        args,
    );
    Ok(r)
}

/// Extract the single newly created shared object reference from a set of
/// effects. We use this for account creation transactions which always create
/// exactly one shared account object.
fn first_created_shared(effects: &TransactionEffects) -> anyhow::Result<ObjectRef> {
    effects
        .all_changed_objects()
        .into_iter()
        .find_map(|(oref, owner, kind)| {
            matches!(kind, WriteKind::Create)
                .then(|| matches!(owner, Owner::Shared(_)).then_some(oref))
                .flatten()
        })
        .ok_or_else(|| anyhow::anyhow!("no created shared object in effects"))
}

/// Format `pkg::module::Type` as a `TypeTag`.
fn type_tag(package: &ObjectId, module: &str, type_name: &str) -> TypeTag {
    TypeTag::from_str(&format!("{package}::{module}::{type_name}")).unwrap()
}

/// Build TransactionData with the owner as sender (and sponsor by default).
async fn tx_data_from_pt(
    env: &TestEnvironment,
    pt: ProgrammableTransaction,
    sender: IotaAddress,
    gas: ObjectRef,
) -> TransactionData {
    let gas_price = env.test_cluster.get_reference_gas_price().await;
    TransactionData::new_programmable_allow_sponsor(
        sender,
        vec![gas],
        pt,
        gas_price * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        gas_price,
        sender,
    )
}

/// Execute a transaction whose primary signature is a `MoveAuthenticator`
/// and return whether the validators accepted it.
///
/// Goes through the direct validator path
/// ([`TestCluster::execute_transaction_return_raw_effects`]) rather than
/// the wallet's JSON-RPC path; that way a signing-time auth rejection
/// (e.g. `OUT_OF_GAS` while running the authenticator under
/// `max_auth_gas`) surfaces as `Err` here rather than being smoothed over.
async fn execute_aa_tx_outcome(
    env: &TestEnvironment,
    tx: Transaction,
) -> (Outcome, Option<String>) {
    match env
        .test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await
    {
        Ok((effects, _events)) => {
            if effects.status().is_success() {
                (Outcome::Pass, None)
            } else {
                (
                    Outcome::Fail,
                    Some(format!("effects.status = {:?}", effects.status())),
                )
            }
        }
        Err(e) => (Outcome::Fail, Some(format!("submit: {e:?}"))),
    }
}

// ---------------------------------------------------------------------------
// --- Shared sub-routines used by multiple scenarios -------------------------
// ---------------------------------------------------------------------------

/// Common path: build a `TransactionData` from `pt` (sender = owner), execute,
/// and return the unique created shared object as the account ref.
async fn create_account_with_pt(
    env: &TestEnvironment,
    pt: ProgrammableTransaction,
) -> anyhow::Result<ObjectRef> {
    let tx_data = env
        .test_cluster
        .test_transaction_builder()
        .await
        .programmable(pt)
        .build();
    let tx = env.test_cluster.wallet.sign_transaction(&tx_data);
    let (effects, _) = env
        .test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await?;
    if !effects.status().is_success() {
        anyhow::bail!("create account tx failed: {:?}", effects.status());
    }
    first_created_shared(&effects)
}

/// Variants of the per-authenticator extra `CallArg` list (before signature).
enum AuthCallArgs {
    /// `[signature]`
    Ed25519Only,
    /// `[clock, signature]`
    ClockThenEd25519,
    /// `[pub_key, signature]` — function_call_keys owner flow.
    PubKeyThenEd25519(Vec<u8>),
}

impl AuthCallArgs {
    fn ed25519_only() -> Self {
        Self::Ed25519Only
    }
    fn clock_then_ed25519() -> Self {
        Self::ClockThenEd25519
    }
    fn pubkey_then_ed25519(pk: Vec<u8>) -> Self {
        Self::PubKeyThenEd25519(pk)
    }
}

async fn run_simple_auth_ed25519(
    env: &TestEnvironment,
    account_ref: ObjectRef,
    _pkg_id: ObjectId,
    args: AuthCallArgs,
) -> anyhow::Result<(Outcome, Option<String>)> {
    let aa_sender: IotaAddress = account_ref.object_id.into();

    let rgp = env.test_cluster.get_reference_gas_price().await;
    let gas = env
        .test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), aa_sender)
        .await;

    let pt = simple_sender_clock_ptb();
    let tx_data = tx_data_from_pt(env, pt, aa_sender, gas).await;
    let tx_digest = tx_data.digest().into_inner();
    let signature = env.sign_digest_raw(&tx_digest);

    let extra_args = match args {
        AuthCallArgs::Ed25519Only => vec![CallArg::Pure(bcs::to_bytes(&signature)?)],
        AuthCallArgs::ClockThenEd25519 => vec![
            CallArg::Shared(SharedObjectRef {
                object_id: IOTA_CLOCK_OBJECT_ID,
                initial_shared_version: IOTA_CLOCK_OBJECT_SHARED_VERSION,
                mutable: false,
            }),
            CallArg::Pure(bcs::to_bytes(&signature)?),
        ],
        AuthCallArgs::PubKeyThenEd25519(pk) => vec![
            CallArg::Pure(bcs::to_bytes(&pk)?),
            CallArg::Pure(bcs::to_bytes(&signature)?),
        ],
    };

    let auth = make_move_authenticator(account_ref, extra_args)?;
    let tx = Transaction::from_generic_sig_data(tx_data, vec![auth]);
    Ok(execute_aa_tx_outcome(env, tx).await)
}

/// A trivial PTB whose sender is the AA: read `Clock::timestamp_ms`. The PTB
/// itself does not need authentication beyond the AA — only the
/// `MoveAuthenticator` matters for these tests.
fn simple_sender_clock_ptb() -> ProgrammableTransaction {
    let mut b = ProgrammableTransactionBuilder::new();
    let clock = b
        .obj(CallArg::Shared(SharedObjectRef {
            object_id: IOTA_CLOCK_OBJECT_ID,
            initial_shared_version: IOTA_CLOCK_OBJECT_SHARED_VERSION,
            mutable: false,
        }))
        .unwrap();
    b.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("clock"),
        Identifier::from_static("timestamp_ms"),
        vec![],
        vec![clock],
    );
    b.finish()
}

/// Rust mirror of `onesig::merkle::build_merkle_tree_with_proofs` — a
/// sorted-pair keccak Merkle tree. Each leaf is `keccak256(leaf_bytes)`,
/// internal nodes are `keccak256(min(l,r) || max(l,r))`, and odd unpaired
/// nodes at any level are carried up unchanged.
///
/// Returns `(root, proofs)` where `proofs[i]` is the list of sibling hashes
/// for the i-th input leaf, walking bottom-up.
fn build_sorted_keccak_merkle_tree(leaves: &[Vec<u8>]) -> (Vec<u8>, Vec<Vec<Vec<u8>>>) {
    fn keccak(bytes: &[u8]) -> Vec<u8> {
        let mut h = Keccak256::default();
        h.update(bytes);
        h.finalize().to_vec()
    }

    fn hash_pair_sorted(left: &[u8], right: &[u8]) -> Vec<u8> {
        let mut h = Keccak256::default();
        if left < right {
            h.update(left);
            h.update(right);
        } else {
            h.update(right);
            h.update(left);
        }
        h.finalize().to_vec()
    }

    let n = leaves.len();
    assert!(n > 0, "merkle tree needs at least one leaf");

    let mut current_level: Vec<Vec<u8>> = leaves.iter().map(|l| keccak(l)).collect();
    let mut proofs: Vec<Vec<Vec<u8>>> = vec![Vec::new(); n];
    let mut leaf_pos: Vec<usize> = (0..n).collect();

    while current_level.len() > 1 {
        let level_len = current_level.len();
        let mut next_level: Vec<Vec<u8>> = Vec::new();

        let mut j = 0;
        while j + 1 < level_len {
            next_level.push(hash_pair_sorted(&current_level[j], &current_level[j + 1]));
            j += 2;
        }
        if level_len % 2 == 1 {
            next_level.push(current_level[level_len - 1].clone());
        }

        for i in 0..n {
            let pos = leaf_pos[i];
            if pos.is_multiple_of(2) {
                if pos + 1 < level_len {
                    proofs[i].push(current_level[pos + 1].clone());
                }
                // else: unpaired last node — no sibling to record.
            } else {
                proofs[i].push(current_level[pos - 1].clone());
            }
            leaf_pos[i] = pos / 2;
        }

        current_level = next_level;
    }

    (current_level.remove(0), proofs)
}

/// Build a `MoveAuthenticator` v1 from extra args and the account ref.
fn make_move_authenticator(
    account_ref: ObjectRef,
    extra_args: Vec<CallArg>,
) -> anyhow::Result<GenericSignature> {
    let self_call_arg = CallArg::Shared(SharedObjectRef {
        object_id: account_ref.object_id,
        initial_shared_version: account_ref.version,
        mutable: false,
    });
    Ok(GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_v1(extra_args, vec![], self_call_arg),
    ))
}

/// Scan a publish response for a `Created` object whose owner is `Shared`
/// and whose `object_type` matches `expected`. Returns the object reference,
/// or `None` if no match exists.
fn find_created_shared_in_response(
    resp: &iota_json_rpc_types::IotaTransactionBlockResponse,
    expected: &TypeTag,
) -> Option<ObjectRef> {
    let expected_struct = match expected {
        TypeTag::Struct(s) => s.as_ref(),
        _ => return None,
    };
    resp.object_changes.as_ref()?.iter().find_map(|c| match c {
        iota_json_rpc_types::ObjectChange::Created {
            owner: Owner::Shared(_),
            object_type,
            object_id,
            version,
            digest,
            ..
        } if object_type == expected_struct => Some(ObjectRef::new(*object_id, *version, *digest)),
        _ => None,
    })
}
