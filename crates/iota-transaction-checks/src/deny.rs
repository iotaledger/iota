// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Command, ObjectId, ObjectReference, Transaction, UserSignature};
use iota_types::{
    account_abstraction::authenticator_function::AuthenticatorFunctionRef,
    deny_rule_governance::DenyRuleConfig,
    error::{IotaError, IotaResult, UserInputError},
    storage::BackingPackageStore,
    transaction::{InputObjectKind, TransactionAPI, TransactionKindExt},
};
use tracing::instrument;
macro_rules! deny_if_true {
    ($cond:expr, $msg:expr) => {
        if ($cond) {
            return Err(IotaError::UserInput {
                error: UserInputError::TransactionDenied {
                    error: $msg.to_string(),
                },
            });
        }
    };
}

/// Check that the provided transaction is allowed to be signed according to the
/// deny config.
#[instrument(level = "trace", skip_all, fields(tx_digest = ?tx.digest()))]
pub fn check_transaction_for_validation(
    tx: &Transaction,
    tx_signatures: &[UserSignature],
    input_object_kinds: &[InputObjectKind],
    receiving_objects: &[ObjectReference],
    filter_config: &dyn DenyRuleConfig,
    package_store: &dyn BackingPackageStore,
) -> IotaResult {
    check_disabled_features(filter_config, tx, tx_signatures)?;

    check_signers(filter_config, tx)?;

    check_input_objects(filter_config, input_object_kinds)?;

    check_package_dependencies(filter_config, tx, package_store)?;

    check_receiving_objects(filter_config, receiving_objects)?;

    Ok(())
}

#[instrument(level = "trace", skip_all)]
fn check_receiving_objects(
    filter_config: &dyn DenyRuleConfig,
    receiving_objects: &[ObjectReference],
) -> IotaResult {
    deny_if_true!(
        filter_config.receiving_objects_disabled() && !receiving_objects.is_empty(),
        "Receiving objects is temporarily disabled".to_string()
    );
    if !filter_config.has_denied_objects() {
        return Ok(());
    }
    for receiving_object in receiving_objects {
        deny_if_true!(
            filter_config.is_object_denied(&receiving_object.object_id),
            format!(
                "Access to object {:?} is temporarily disabled",
                receiving_object.object_id
            )
        );
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
fn check_disabled_features(
    filter_config: &dyn DenyRuleConfig,
    tx: &Transaction,
    tx_signatures: &[UserSignature],
) -> IotaResult {
    deny_if_true!(
        filter_config.user_transaction_disabled(),
        "Transaction signing is temporarily disabled"
    );

    tx_signatures.iter().try_for_each(|s| {
        if let UserSignature::MoveAuthenticator(_) = s {
            deny_if_true!(
                filter_config.move_authenticator_disabled(),
                "MoveAuthenticator is temporarily disabled"
            );
        }
        Ok(())
    })?;

    if !filter_config.package_publish_disabled() && !filter_config.package_upgrade_disabled() {
        return Ok(());
    }

    for command in tx.kind().iter_commands() {
        deny_if_true!(
            filter_config.package_publish_disabled() && matches!(command, Command::Publish(..)),
            "Package publish is temporarily disabled"
        );
        deny_if_true!(
            filter_config.package_upgrade_disabled() && matches!(command, Command::Upgrade(..)),
            "Package upgrade is temporarily disabled"
        );
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
fn check_signers(filter_config: &dyn DenyRuleConfig, tx: &Transaction) -> IotaResult {
    if !filter_config.has_denied_addresses() {
        return Ok(());
    }
    for signer in tx.signers() {
        deny_if_true!(
            filter_config.is_address_denied(&signer),
            format!(
                "Access to account address {:?} is temporarily disabled",
                signer
            )
        );
    }
    Ok(())
}

#[instrument(level = "trace", skip_all)]
fn check_input_objects(
    filter_config: &dyn DenyRuleConfig,
    input_object_kinds: &[InputObjectKind],
) -> IotaResult {
    let shared_object_disabled = filter_config.shared_object_disabled();
    if !filter_config.has_denied_objects() && !shared_object_disabled {
        // No need to iterate through the input objects if no relevant policy is set.
        return Ok(());
    }
    for input_object_kind in input_object_kinds {
        let id = input_object_kind.object_id();
        deny_if_true!(
            filter_config.is_object_denied(&id),
            format!("Access to input object {id} is temporarily disabled")
        );
        deny_if_true!(
            shared_object_disabled && input_object_kind.is_shared_object(),
            "Usage of shared object in transactions is temporarily disabled"
        );
    }
    Ok(())
}

/// Check that no `MoveAuthenticator` authenticates through a denied package.
///
/// The package holding the authenticate function is named only by the account's
/// `AuthenticatorFunctionRef`: the call is assembled during execution, and the
/// reference is a runtime one, so the package appears in neither the
/// transaction's commands nor any linkage table. It is therefore checked here
/// rather than alongside a transaction's commands in
/// [`check_transaction_for_validation`].
///
/// Call it once the references have been loaded, over every authenticator the
/// transaction carries, and before any authenticator runs.
#[instrument(level = "trace", skip_all)]
pub fn check_authenticator_packages<'a>(
    filter_config: &dyn DenyRuleConfig,
    authenticator_function_refs: impl IntoIterator<Item = &'a AuthenticatorFunctionRef>,
    package_store: &dyn BackingPackageStore,
) -> IotaResult {
    if !filter_config.has_denied_packages() {
        return Ok(());
    }
    for authenticator_function_ref in authenticator_function_refs {
        let package_id = match authenticator_function_ref {
            AuthenticatorFunctionRef::V1(v1) => v1.package,
        };
        for dep in package_and_dependency_ids(package_id, package_store)? {
            deny_if_true!(
                filter_config.is_package_denied(&dep),
                format!(
                    "Access to package {dep} from a Move authenticator is temporarily disabled"
                )
            );
        }
    }
    Ok(())
}

/// The ids the deny list has to see for a call into `package_id`: the package
/// itself, and the upgraded id of each of its dependencies.
///
/// `linkage_table` maps the original id of a dependency to the id actually
/// loaded, and only the latter is returned. So this establishes that a denied
/// package is not in current use, and denying one version of a package still
/// permits a newer one.
fn package_and_dependency_ids(
    package_id: ObjectId,
    package_store: &dyn BackingPackageStore,
) -> IotaResult<Vec<ObjectId>> {
    let package = package_store
        .get_package_object(&package_id)?
        .ok_or(IotaError::UserInput {
            error: UserInputError::ObjectNotFound {
                object_id: package_id,
                version: None,
            },
        })?;
    let package = package.move_package();
    Ok(package
        .linkage_table()
        .values()
        .map(|upgrade_info| upgrade_info.upgraded_id)
        .chain(std::iter::once(package.id()))
        .collect())
}

#[instrument(level = "trace", skip_all)]
fn check_package_dependencies(
    filter_config: &dyn DenyRuleConfig,
    tx: &Transaction,
    package_store: &dyn BackingPackageStore,
) -> IotaResult {
    if !filter_config.has_denied_packages() {
        return Ok(());
    }
    let mut dependencies = vec![];
    for command in tx.kind().iter_commands() {
        match command {
            Command::Publish(cmd) => {
                // It is possible that the deps list is inaccurate since it's provided
                // by the user. But that's OK because this publish transaction will fail
                // to execute in the end. Similar reasoning for Upgrade.
                dependencies.extend(cmd.dependencies.iter().copied());
            }
            Command::Upgrade(cmd) => {
                dependencies.extend(cmd.dependencies.iter().copied());
                // It's crucial that we don't allow upgrading a package in the deny list,
                // otherwise one can bypass the deny list by upgrading a package.
                dependencies.push(cmd.package);
            }
            Command::MoveCall(cmd) => {
                dependencies.extend(package_and_dependency_ids(cmd.package, package_store)?);
            }
            Command::TransferObjects(..)
            | Command::SplitCoins(..)
            | Command::MergeCoins(..)
            | Command::MakeMoveVector(..) => {}
            _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
        }
    }
    for dep in dependencies {
        deny_if_true!(
            filter_config.is_package_denied(&dep),
            format!("Access to package {dep} is temporarily disabled")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use iota_sdk_types::{
        DenyRuleSet, MovePackage, ObjectId, TransactionDigest, UpgradeInfo, Version,
    };
    use iota_types::{
        account_abstraction::authenticator_function::{
            AuthenticatorFunctionRef, AuthenticatorFunctionRefV1,
        },
        object::Object,
        storage::PackageObject,
    };

    use super::*;

    const MAX_PACKAGE_SIZE: u64 = 100 * 1024;

    /// A package store holding only the packages a test puts in it; anything
    /// else reads back as missing.
    #[derive(Default)]
    struct TestPackageStore(HashMap<ObjectId, PackageObject>);

    impl BackingPackageStore for TestPackageStore {
        fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
            Ok(self.0.get(package_id).cloned())
        }
    }

    impl TestPackageStore {
        /// Adds a module-less package whose linkage table maps each dependency
        /// onto itself, which is what a package that has never been upgraded
        /// past its dependencies looks like.
        fn with_package(
            mut self,
            id: ObjectId,
            dependencies: impl IntoIterator<Item = ObjectId>,
        ) -> Self {
            let linkage_table: BTreeMap<_, _> = dependencies
                .into_iter()
                .map(|dep| {
                    (
                        dep,
                        UpgradeInfo {
                            upgraded_id: dep,
                            upgraded_version: Version::OBJECT_START,
                        },
                    )
                })
                .collect();
            let package = MovePackage::new(
                id,
                Version::OBJECT_START,
                BTreeMap::new(),
                MAX_PACKAGE_SIZE,
                vec![],
                linkage_table,
            )
            .unwrap();
            self.0.insert(
                id,
                PackageObject::new(Object::new_from_package(
                    package,
                    TransactionDigest::genesis_marker(),
                )),
            );
            self
        }
    }

    fn id(byte: u8) -> ObjectId {
        ObjectId::new([byte; 32])
    }

    fn authenticator_ref(package: ObjectId) -> AuthenticatorFunctionRef {
        AuthenticatorFunctionRef::V1(AuthenticatorFunctionRefV1 {
            package,
            module: "account".to_string(),
            function: "authenticate".to_string(),
        })
    }

    fn deny_packages(packages: impl IntoIterator<Item = ObjectId>) -> DenyRuleSet {
        DenyRuleSet {
            denied_packages: packages.into_iter().collect(),
            ..Default::default()
        }
    }

    fn assert_denied(result: IotaResult) {
        assert!(matches!(
            result.unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::TransactionDenied { .. }
            }
        ));
    }

    #[test]
    fn denies_the_authenticator_package_itself() {
        let package = id(1);
        let store = TestPackageStore::default().with_package(package, []);

        assert_denied(check_authenticator_packages(
            &deny_packages([package]),
            &[authenticator_ref(package)],
            &store,
        ));
    }

    #[test]
    fn denies_a_dependency_of_the_authenticator_package() {
        let (package, dependency) = (id(1), id(2));
        let store = TestPackageStore::default().with_package(package, [dependency]);

        assert_denied(check_authenticator_packages(
            &deny_packages([dependency]),
            &[authenticator_ref(package)],
            &store,
        ));
    }

    #[test]
    fn denies_when_any_one_of_several_authenticators_is_denied() {
        // A sponsored transaction carries one authenticator per signer, and the
        // sender's must be judged even when the sponsor's is fine.
        let (sender_package, sponsor_package) = (id(1), id(2));
        let store = TestPackageStore::default()
            .with_package(sender_package, [])
            .with_package(sponsor_package, []);

        assert_denied(check_authenticator_packages(
            &deny_packages([sender_package]),
            &[
                authenticator_ref(sponsor_package),
                authenticator_ref(sender_package),
            ],
            &store,
        ));
    }

    #[test]
    fn allows_a_package_that_is_not_denied() {
        let (package, dependency, denied) = (id(1), id(2), id(3));
        let store = TestPackageStore::default().with_package(package, [dependency]);

        check_authenticator_packages(
            &deny_packages([denied]),
            &[authenticator_ref(package)],
            &store,
        )
        .unwrap();
    }

    #[test]
    fn reads_no_package_when_nothing_is_denied() {
        // The empty store would report the package as missing, so an error here
        // would mean the early exit failed to spare the read.
        check_authenticator_packages(
            &DenyRuleSet::default(),
            &[authenticator_ref(id(1))],
            &TestPackageStore::default(),
        )
        .unwrap();
    }

    #[test]
    fn reports_a_missing_authenticator_package() {
        let missing = id(1);

        assert!(matches!(
            check_authenticator_packages(
                &deny_packages([id(9)]),
                &[authenticator_ref(missing)],
                &TestPackageStore::default(),
            )
            .unwrap_err(),
            IotaError::UserInput {
                error: UserInputError::ObjectNotFound { object_id, .. }
            } if object_id == missing
        ));
    }
}
