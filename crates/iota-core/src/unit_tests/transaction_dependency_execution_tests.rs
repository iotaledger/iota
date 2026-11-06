// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Execution of a transaction with a transaction dependency input, once the
//! transaction it depends on has executed. The dependents here are certified
//! by the authority key alone, so the check before signing does not reject
//! them and execution decides.

use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, ObjectId, SharedObjectReference, TransactionDigest,
    TransactionEffects,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    executable_transaction::VerifiedExecutableTransaction,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TransactionEnvelope},
};

use crate::authority::{
    AuthorityState, ExecutionEnv,
    authority_test_utils::{init_certified_transaction, init_state_with_objects},
};

struct Setup {
    authority: std::sync::Arc<AuthorityState>,
    sender: Address,
    key: AccountPrivateKey,
    gas: Vec<Object>,
    shared: Object,
    /// Enables transaction dependencies for as long as the setup lives.
    _config_guard: OverrideGuard,
}

async fn setup() -> Setup {
    let config_guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_transaction_dependencies_for_testing(true);
        config
    });
    let (sender, key): (_, AccountPrivateKey) = get_key_pair();
    let gas: Vec<Object> = (0..2)
        .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
        .collect();
    let shared = Object::shared_for_testing();
    let authority = init_state_with_objects(gas.iter().cloned().chain([shared.clone()])).await;
    Setup {
        authority,
        sender,
        key,
        gas,
        shared,
        _config_guard: config_guard,
    }
}

impl Setup {
    /// A transfer of `amount` paid with gas coin `gas_index`. `u64::MAX` fails
    /// at execution. With `with_shared`, the shared object is a mutable input
    /// that no command uses.
    fn transfer(
        &self,
        gas_index: usize,
        amount: u64,
        dependency: Option<TransactionDigest>,
        with_shared: bool,
    ) -> TransactionEnvelope {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(Address::random(), Some(amount));
        if with_shared {
            builder
                .obj(CallArg::Shared(SharedObjectReference::new(
                    self.shared.id(),
                    self.shared.version(),
                    true,
                )))
                .unwrap();
        }
        let rgp = self
            .authority
            .epoch_store_for_testing()
            .reference_gas_price();
        let mut transaction =
            TestTransactionBuilder::new(self.sender, self.gas[gas_index].object_ref(), rgp)
                .programmable(builder.finish());
        if let Some(dependency) = dependency {
            transaction = transaction.with_transaction_dependency(dependency, 0);
        }
        transaction.build_and_sign(&self.key)
    }

    fn execute(&self, transaction: TransactionEnvelope, with_shared: bool) -> TransactionEffects {
        let certificate = init_certified_transaction(transaction, &self.authority);
        let assigned_versions = if with_shared {
            self.authority
                .epoch_store_for_testing()
                .assign_shared_object_versions_for_tests(
                    self.authority.get_object_cache_reader().as_ref(),
                    &[VerifiedExecutableTransaction::new_from_certificate(
                        certificate.clone(),
                    )],
                )
                .unwrap()
                .into_map()
                .get(&certificate.key())
                .cloned()
                .unwrap()
        } else {
            vec![]
        };
        let (effects, _) = self
            .authority
            .try_execute_for_test(
                &certificate,
                ExecutionEnv::new().with_assigned_versions(assigned_versions),
            )
            .unwrap();
        effects.inner().data().clone()
    }
}

#[tokio::test]
async fn dependent_of_a_failed_transaction_is_cancelled_at_execution() {
    let setup = setup().await;
    let failing = setup.transfer(0, u64::MAX, None, false);
    let failing_digest = *failing.digest();
    let failing_effects = setup.execute(failing, false);
    assert!(!failing_effects.status().is_success());

    let dependent = setup.transfer(1, 1_000, Some(failing_digest), true);
    let effects = setup.execute(dependent, true);

    let ExecutionStatus::Failure { error, command } = effects.status() else {
        panic!("the dependent of a failed transaction succeeded: {effects:?}");
    };
    assert_eq!(
        *error,
        ExecutionError::TransactionDependencyFailed {
            dependency: failing_digest
        }
    );
    assert_eq!(*command, None);
    assert!(effects.dependencies().contains(&failing_digest));
    // No command ran: nothing was created.
    assert!(effects.created().is_empty());
    // Charged like any failure, and the inputs move to their next versions
    // unchanged, the unused mutable shared input included.
    assert!(effects.gas_cost_summary().computation_cost > 0);
    assert!(effects.gas_object().reference().version > setup.gas[1].version());
    assert!(
        effects
            .mutated()
            .iter()
            .any(|object| object.reference().object_id == setup.shared.id())
    );
}

#[tokio::test]
async fn dependent_of_a_successful_transaction_executes() {
    let setup = setup().await;
    let dependency = setup.transfer(0, 1_000, None, false);
    let dependency_digest = *dependency.digest();
    assert!(setup.execute(dependency, false).status().is_success());

    let dependent = setup.transfer(1, 1_000, Some(dependency_digest), false);
    let effects = setup.execute(dependent, false);

    assert!(effects.status().is_success(), "{effects:?}");
    assert!(effects.dependencies().contains(&dependency_digest));
    assert!(!effects.created().is_empty());
}
