// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Abstract Account verifier tests
//!
//! The tests in this module are meant to test the
//! `account::create_auth_info_v1` native function ,`iota-execution/latest/
//! iota-verifier/src/account_auth_verifier.rs` verifier and the
//! `iota-execution/latest/iota-move-natives/src/raw_module_loader/mod.rs`in an
//! e2e environment.

use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{ObjectID, ObjectRef},
    effects::{TransactionEffects, TransactionEvents},
    transaction::CallArg,
};
use move_core_types::account_address::AccountAddress;
use test_cluster::{TestCluster, TestClusterBuilder};

macro_rules! use_crates {
    () => {
        use iota_macros::sim_test;
        use iota_types::effects::TransactionEffectsAPI;

        use super::TestEnvironment;
    };
}

/// Generate `simtest` tests for each `move module`.
///
/// To use the macro:
/// tests_in_module(<module_name>; pass::{<test_move_function>+}?
/// fail::{<test_move_function>+}?) pattern should be used.
/// Either the `pass` or `fail` section can be omitted, but not both.
///
/// This macro collection helps remove the need to individually write out the
/// simtests. The generated code will look likes so:
/// ```
/// <module_name> {
///     pass {
///         #[sim_test]
///         async fn <test_move_function> { ... }
///         ...
///     }
///     fail {
///         #[sim_test]
///         async fn <test_move_function> { ... }
///         ...
///     }
/// }
/// ```
/// With this module hierarchy we remove the possibility for test_name
/// collisions and can refer to each test case as one could find it in the
/// relevant source. E.x.:
/// ```
/// tests_in_module!(
///     object;
///         pass::{
///            immutable_ref
///        }
/// )
/// ```
/// The test can be found in the `object` move module and its name is
/// `immutable_ref`.
macro_rules! tests_in_module {
    ($module_name:ident; pass::{$($function_name_p:ident),+ $(,)?}) => {
        mod $module_name {
            use_crates!();

            gen_simtest_expect_pass![$($module_name::$function_name_p),+];
        }
    };
    ($module_name:ident; fail::{$($function_name_f:ident),+ $(,)?}) => {
        mod $module_name {
            use_crates!();

            gen_simtest_expect_fail![$($module_name::$function_name_f),+];
        }
    };
    ($module_name:ident; pass::{$($function_name_p:ident),+ $(,)?} fail::{$($function_name_f:ident),+ $(,)?}) => {
        mod $module_name {
            use_crates!();

            gen_simtest_expect_pass![$($module_name::$function_name_p),+];
            gen_simtest_expect_fail![$($module_name::$function_name_f),+];
        }
    };
}

macro_rules! gen_simtest_expect_pass {
    () => {};
    ($module_name: ident::$function_name: ident) => {
        #[sim_test]
        async fn $function_name() {
            let env = TestEnvironment::new().await;
            let (effects, _) = env
                .create_auth_info_using(stringify!($module_name), stringify!($function_name))
                .await
                .unwrap();
            assert!(effects.status().is_ok());
        }
    };
    ($module_name: ident::$function_name: ident, $($module_name_n: ident::$function_name_n: ident),+) => {
        gen_simtest_expect_pass![$module_name::$function_name];
        gen_simtest_expect_pass![$($module_name_n::$function_name_n),+];
    }
}

macro_rules! gen_simtest_expect_fail {
    () => {};
    ($module_name: ident::$function_name: ident) => {
        #[sim_test]
        async fn $function_name() {
            let env = TestEnvironment::new().await;
            let (effects, _) = env
                .create_auth_info_using(stringify!($module_name), stringify!($function_name))
                .await
                .unwrap();
            assert!(effects.status().is_err());
        }
    };
    ($module_name: ident::$function_name: ident, $($module_name_n: ident::$function_name_n: ident),+) => {
        gen_simtest_expect_fail![$module_name::$function_name];
        gen_simtest_expect_fail![$($module_name_n::$function_name_n),+];
    }
}

tests_in_module!(
object;
pass::{
    immutable_ref
}
fail::{
    by_value,
    by_mutable_ref
});

tests_in_module!(
option;
pass::{
    primitive_immutable_reference,
    primitive_by_value,
    // non_object_immutable_ref,
    object_immutable_ref,
    template_non_object_immutable_ref,
    templated_non_object_immutable_ref,
    template_object_immutable_reference,
    templated_object_immutable_ref
}
fail::{
    primitive_mutable_reference,
    non_object_mutable_ref,
    non_object_by_value,
    object_mutable_ref,
    object_by_value,
    template_non_object_mutable_ref,
    templated_non_object_by_value,
    templated_non_object_mutable_ref,
    template_object_by_value,
    template_object_mutable_reference,
    templated_object_by_value,
    templated_object_mutable_ref
});

tests_in_module!(
receiving;
fail::{
    immutable_ref,
    by_value,
    by_mutable_ref,
    vector_immutable_ref,
    vector_by_value,
    vector_by_mutable_ref,
    option_immutable_ref,
    option_by_value,
    option_by_mutable_ref
});

tests_in_module!(
signature;
pass::{
    minimally_viable_auth_function,
    arg_value,
    arg_mutable_value
}
fail::{
    has_to_be_public_auth_function,
    at_least_two_args,
    auth_context_cant_be_value,
    auth_context_cant_be_mutable_ref,
    tx_context_cant_be_value,
    tx_context_cant_be_mutable_ref,
    auth_context_isnt_struct,
    tx_context_isnt_struct,
    with_signer
});

tests_in_module!(
template;
pass::{
    primitive,
    templated_non_object_immutable_ref,
    object_immutable_ref,
    templated_object_immutable_ref,
}
fail::{
    templated_non_object_mutable_ref,
    templated_non_object_by_value,
    object_by_value,
    object_mutable_ref,
    templated_object_by_value,
    templated_object_mutable_ref
});

tests_in_module!(
vector;
pass::{
    primitive_immutable_reference,
    primitive_by_value,
    non_object_immutable_ref,
    object_immutable_ref,
    template_non_object_immutable_ref,
    templated_non_object_immutable_ref,
    template_object_immutable_reference,
    templated_object_immutable_ref,
}
fail::{
    primitive_mutable_reference,
    non_object_mutable_ref,
    non_object_by_value,
    object_mutable_ref,
    object_by_value,
    template_non_object_mutable_ref,
    templated_non_object_by_value,
    templated_non_object_mutable_ref,
    template_object_by_value,
    template_object_mutable_reference,
    templated_object_by_value,
    templated_object_mutable_ref
});

struct TestEnvironment {
    cluster: TestCluster,
    package_id: ObjectID,
}

impl TestEnvironment {
    async fn new() -> Self {
        let cluster = TestClusterBuilder::new().build().await;

        let package_id = publish_move_package(&cluster).await.0;

        Self {
            cluster,
            package_id,
        }
    }

    async fn create_auth_info_using(
        &self,
        module_name: &str,
        function_name: &str,
    ) -> anyhow::Result<(TransactionEffects, TransactionEvents)> {
        let arguments = vec![
            CallArg::Pure(bcs::to_bytes(&Into::<AccountAddress>::into(self.package_id)).unwrap()),
            CallArg::Pure(bcs::to_bytes(module_name.as_bytes()).unwrap()),
            CallArg::Pure(bcs::to_bytes(function_name.as_bytes()).unwrap()),
        ];

        let transaction_data = self
            .cluster
            .test_transaction_builder()
            .await
            .move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                "account",
                "create_auth_info_v1",
                arguments,
            )
            .build();

        let transaction = self.cluster.wallet.sign_transaction(&transaction_data);
        self.cluster
            .execute_transaction_return_raw_effects(transaction)
            .await
    }
}

async fn publish_move_package(test_cluster: &TestCluster) -> ObjectRef {
    let path = [
        env!("CARGO_MANIFEST_DIR"),
        "tests/abstract_account/authenticate",
    ]
    .iter()
    .collect();
    publish_package(&test_cluster.wallet, path).await
}
