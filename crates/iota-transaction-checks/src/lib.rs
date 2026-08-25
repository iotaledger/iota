// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod deny;

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {
    use std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    };

    use iota_config::verifier_signing_config::VerifierSigningConfig;
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{
        Address, ObjectId, ObjectReference, Owner, Transaction, TransactionKind, Version,
    };
    use iota_types::{
        IOTA_AUTHENTICATOR_STATE_OBJECT_ID, IOTA_CLOCK_OBJECT_SHARED_VERSION,
        error::{IotaError, IotaResult, UserInputError, UserInputResult},
        executable_transaction::VerifiedExecutableTransaction,
        fp_bail, fp_ensure,
        gas::IotaGasStatus,
        metrics::BytecodeVerifierMetrics,
        object::Object,
        transaction::{
            CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult,
            ObjectReadResultKind, ProgrammableTransactionExt, ReceivingObjectReadResult,
            ReceivingObjects, TransactionAPI,
        },
        transaction_executor::InputCheckRelaxations,
    };
    use tracing::{error, instrument};

    trait IntoChecked {
        fn into_checked(self) -> CheckedInputObjects;
    }

    impl IntoChecked for InputObjects {
        fn into_checked(self) -> CheckedInputObjects {
            CheckedInputObjects::new_with_checked_transaction_inputs(self)
        }
    }

    // Entry point for all checks related to gas.
    // Called on both signing and execution.
    // On success the gas part of the transaction (gas data and gas coins)
    // is verified and good to go
    fn get_gas_status(
        objects: &InputObjects,
        gas: &[ObjectReference],
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        transaction: &Transaction,
        authentication_gas_budget: u64,
        is_execute_transaction_to_effects: bool,
        relaxations: InputCheckRelaxations,
    ) -> IotaResult<IotaGasStatus> {
        if transaction.is_system_tx() {
            Ok(IotaGasStatus::new_unmetered())
        } else {
            check_gas(
                objects,
                protocol_config,
                reference_gas_price,
                gas,
                transaction.gas_price(),
                transaction.gas_budget(),
                authentication_gas_budget,
                is_execute_transaction_to_effects,
                relaxations,
            )
        }
    }

    /// Checks whether a transaction may run, for signing, for a certificate, or
    /// for a simulation.
    ///
    /// `relaxations` names the checks a caller drops. A simulation with
    /// [`VmChecks::Disabled`](iota_types::transaction_executor::VmChecks::Disabled)
    /// passes [`InputCheckRelaxations::SIMULATION`]; everything bound for
    /// execution passes [`InputCheckRelaxations::EXECUTION`]. A caller that
    /// needs a check relaxed must name it in `InputCheckRelaxations`, with the
    /// reason on the field, rather than validating its inputs somewhere else:
    /// the point of routing every caller through here is that a check added
    /// below applies to all of them until someone says otherwise.
    #[instrument(level = "trace", skip_all, fields(tx_digest = ?transaction.digest()))]
    pub fn check_transaction_input(
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        transaction: &Transaction,
        input_objects: InputObjects,
        receiving_objects: &ReceivingObjects,
        metrics: &Arc<BytecodeVerifierMetrics>,
        verifier_signing_config: &VerifierSigningConfig,
        authentication_gas_budget: u64,
        relaxations: InputCheckRelaxations,
    ) -> IotaResult<(IotaGasStatus, CheckedInputObjects)> {
        let gas_status = check_transaction_input_inner(
            protocol_config,
            reference_gas_price,
            transaction,
            &input_objects,
            &[],
            authentication_gas_budget,
            false,
            relaxations,
        )?;
        check_receiving_objects(&input_objects, receiving_objects, relaxations)?;
        // Runs verifier, which could be expensive.
        check_non_system_packages_to_be_published(
            transaction,
            protocol_config,
            metrics,
            verifier_signing_config,
        )?;

        Ok((gas_status, input_objects.into_checked()))
    }

    #[instrument(level = "trace", skip_all, fields(tx_digest = ?transaction.digest()))]
    pub fn check_transaction_input_with_given_gas(
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        transaction: &Transaction,
        mut input_objects: InputObjects,
        receiving_objects: ReceivingObjects,
        gas_object: Object,
        metrics: &Arc<BytecodeVerifierMetrics>,
        verifier_signing_config: &VerifierSigningConfig,
    ) -> IotaResult<(IotaGasStatus, CheckedInputObjects)> {
        let gas_object_ref = gas_object.object_ref();
        input_objects.push(ObjectReadResult::new_from_gas_object(&gas_object));

        let gas_status = check_transaction_input_inner(
            protocol_config,
            reference_gas_price,
            transaction,
            &input_objects,
            &[gas_object_ref],
            0,
            true,
            InputCheckRelaxations::EXECUTION,
        )?;
        check_receiving_objects(
            &input_objects,
            &receiving_objects,
            InputCheckRelaxations::EXECUTION,
        )?;
        // Runs verifier, which could be expensive.
        check_non_system_packages_to_be_published(
            transaction,
            protocol_config,
            metrics,
            verifier_signing_config,
        )?;

        Ok((gas_status, input_objects.into_checked()))
    }

    // Since the purpose of this function is to audit certified transactions,
    // the checks here should be a strict subset of the checks in
    // check_transaction_input(). For checks not performed in this function but
    // in check_transaction_input(), we should add a comment calling out the
    // difference.
    #[instrument(level = "trace", skip_all)]
    pub fn check_certificate_input(
        cert: &VerifiedExecutableTransaction,
        input_objects: InputObjects,
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
    ) -> IotaResult<(IotaGasStatus, CheckedInputObjects)> {
        let transaction = cert.data().transaction();
        let gas_status = check_transaction_input_inner(
            protocol_config,
            reference_gas_price,
            transaction,
            &input_objects,
            &[],
            0,
            true,
            InputCheckRelaxations::EXECUTION,
        )?;
        // NB: We do not check receiving objects when executing. Only at signing
        // time do we check. NB: move verifier is only checked at
        // signing time, not at execution.

        Ok((gas_status, input_objects.into_checked()))
    }

    /// A common function to check the `MoveAuthenticator` inputs for signing.
    ///
    /// Checks that the authenticator inputs meet the requirements and returns
    /// checked authenticator input objects, among which we also find the
    /// account object.
    #[instrument(level = "trace", skip_all)]
    pub fn check_move_authenticator_input_for_validation(
        authenticator_input_objects: InputObjects,
    ) -> IotaResult<CheckedInputObjects> {
        check_move_authenticator_objects(&authenticator_input_objects)?;

        Ok(authenticator_input_objects.into_checked())
    }

    /// A function to aggregate the checked authenticator input objects for
    /// multiple `MoveAuthenticators` into one `CheckedInputObjects` to be used
    /// for execution.
    pub fn aggregate_authenticator_input_objects(
        per_authenticator_checked_input_objects: &[&CheckedInputObjects],
    ) -> IotaResult<CheckedInputObjects> {
        let mut aggregated_authenticator_input_objects =
            CheckedInputObjects::new_with_checked_transaction_inputs(InputObjects::new(vec![]));

        for authenticator_checked_input_objects in per_authenticator_checked_input_objects.iter() {
            aggregated_authenticator_input_objects = checked_input_objects_union(
                aggregated_authenticator_input_objects,
                authenticator_checked_input_objects,
            )?;
        }

        Ok(aggregated_authenticator_input_objects)
    }

    /// A function to check the `MoveAuthenticator` inputs for execution and
    /// then for certificate execution.
    /// To be used instead of check_certificate_input when there is a Move
    /// authenticator present.
    ///
    /// Checks that there is enough gas to pay for the authenticator and
    /// transaction execution in the transaction inputs. And that the
    /// authenticator inputs meet the requirements.
    /// It returns the gas status, the checked authenticator input objects, and
    /// the union of the checked authenticator input objects and transaction
    /// input objects.
    #[instrument(level = "trace", skip_all)]
    pub fn check_certificate_and_move_authenticator_input(
        cert: &VerifiedExecutableTransaction,
        tx_input_objects: InputObjects,
        per_authenticator_input_objects: Vec<InputObjects>,
        authenticator_gas_budget: u64,
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
    ) -> IotaResult<(IotaGasStatus, Vec<CheckedInputObjects>, CheckedInputObjects)> {
        // Check Move authenticator inputs first
        per_authenticator_input_objects
            .iter()
            .try_for_each(check_move_authenticator_objects)?;

        // Check certificate inputs next
        let transaction = cert.data().transaction();
        let gas_status = check_transaction_input_inner(
            protocol_config,
            reference_gas_price,
            transaction,
            &tx_input_objects,
            &[],
            authenticator_gas_budget,
            true,
            InputCheckRelaxations::EXECUTION,
        )?;

        let per_authenticator_checked_input_objects = per_authenticator_input_objects
            .into_iter()
            .map(|objects| objects.into_checked())
            .collect::<Vec<_>>();

        // Create a checked union of input objects
        let mut input_objects_union = tx_input_objects.into_checked();
        for objects in per_authenticator_checked_input_objects.iter() {
            input_objects_union = checked_input_objects_union(input_objects_union, objects)?;
        }

        Ok((
            gas_status,
            per_authenticator_checked_input_objects,
            input_objects_union,
        ))
    }

    // Common checks performed for transactions and certificates.
    fn check_transaction_input_inner(
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        transaction: &Transaction,
        input_objects: &InputObjects,
        // Overrides the gas objects in the transaction.
        gas_override: &[ObjectReference],
        authentication_gas_budget: u64,
        is_execute_transaction_to_effects: bool,
        relaxations: InputCheckRelaxations,
    ) -> IotaResult<IotaGasStatus> {
        // Cheap validity checks that is ok to run multiple times during processing.
        let gas = if gas_override.is_empty() {
            transaction.gas()
        } else {
            gas_override
        };

        let gas_status = get_gas_status(
            input_objects,
            gas,
            protocol_config,
            reference_gas_price,
            transaction,
            authentication_gas_budget,
            is_execute_transaction_to_effects,
            relaxations,
        )?;
        check_objects(transaction, input_objects, relaxations)?;

        Ok(gas_status)
    }

    /// Checks the receiving references against the objects they name.
    ///
    /// Two separable things happen here. Whether each reference is current —
    /// its version and digest match the loaded object — is an
    /// optimistic-concurrency question, dropped per half by
    /// [`InputCheckRelaxations::any_receiving_object_version`] and
    /// [`InputCheckRelaxations::any_receiving_object_digest`].
    /// What the object is, and that no reference duplicates another or collides
    /// with an input object, is not relaxed by anything: the duplicate
    /// rejection below is the only one there is, since `CallArg::Receiving`
    /// is not part of `input_objects()` and so escapes its
    /// `DuplicateObjectRefInput` dedup. Without it a duplicated receiving
    /// ticket reaches the object runtime, which treats receiving the same
    /// object twice as impossible.
    #[instrument(level = "trace", skip_all)]
    fn check_receiving_objects(
        input_objects: &InputObjects,
        receiving_objects: &ReceivingObjects,
        relaxations: InputCheckRelaxations,
    ) -> Result<(), IotaError> {
        let mut objects_in_txn: HashSet<_> = input_objects
            .object_kinds()
            .map(|x| x.object_id())
            .collect();

        // Since we're at signing we check that every object reference that we are
        // receiving is the most recent version of that object. If it's been
        // received at the version specified we let it through to allow the
        // transaction to run and fail to unlock any other objects in
        // the transaction. Otherwise, we return an error.
        //
        // If there are any object IDs in common (either between receiving objects and
        // input objects) we return an error.
        for ReceivingObjectReadResult { object_ref, object } in receiving_objects.iter() {
            fp_ensure!(
                object_ref.version < Version::MAX_VALID_EXCL,
                UserInputError::InvalidSequenceNumber.into()
            );

            let Some(object) = object.as_object() else {
                // object was previously received
                continue;
            };

            // A reference is fine if it names an address-owned object and matches
            // it on both counts the caller still cares about, so the block below
            // is for the ones that are not. Relax each equality here rather than
            // reordering the block: which error a caller gets when a reference
            // fails more than one test is observable.
            //
            // Keep the two conditions in step with the two `fp_ensure!`s inside.
            // The trailing `match object.owner` has no gate of its own, and its
            // `Owner::Address` arm is a `debug_assert!(false)` — dead only
            // because every path that enters here with an address owner bails at
            // whichever `fp_ensure!` let it in. Skipping a test while still
            // admitting the references it would have rejected makes that arm
            // reachable, which panics in a debug build.
            if !(object.owner.is_address()
                && (object.version() == object_ref.version
                    || relaxations.any_receiving_object_version)
                && (object.digest() == object_ref.digest
                    || relaxations.any_receiving_object_digest))
            {
                if !relaxations.any_receiving_object_version {
                    // Version mismatch
                    fp_ensure!(
                        object.version() == object_ref.version,
                        UserInputError::ObjectVersionUnavailableForConsumption {
                            provided_obj_ref: *object_ref,
                            current_version: object.version(),
                        }
                        .into()
                    );
                }

                // Tried to receive a package
                fp_ensure!(
                    !object.is_package(),
                    UserInputError::MovePackageAsObject {
                        object_id: object_ref.object_id
                    }
                    .into()
                );

                if !relaxations.any_receiving_object_digest {
                    // Digest mismatch
                    let expected_digest = object.digest();
                    fp_ensure!(
                        expected_digest == object_ref.digest,
                        UserInputError::InvalidObjectDigest {
                            object_id: object_ref.object_id,
                            expected_digest
                        }
                        .into()
                    );
                }

                match object.owner {
                    Owner::Address(_) => {
                        debug_assert!(
                            false,
                            "Receiving object {object_ref:?} is invalid but we expect it should be valid. {object:?}"
                        );
                        error!(
                            "Receiving object {:?} is invalid but we expect it should be valid. {:?}",
                            object_ref, object
                        );
                        // We should never get here, but if for some reason we do just default to
                        // object not found and reject signing the transaction.
                        fp_bail!(
                            UserInputError::ObjectNotFound {
                                object_id: object_ref.object_id,
                                version: Some(object_ref.version),
                            }
                            .into()
                        )
                    }
                    Owner::Object(owner) => {
                        fp_bail!(
                            UserInputError::InvalidChildObjectArgument {
                                child_id: object.id(),
                                parent_id: owner,
                            }
                            .into()
                        )
                    }
                    Owner::Shared(_) => fp_bail!(UserInputError::NotSharedObject.into()),
                    Owner::Immutable => fp_bail!(
                        UserInputError::MutableParameterExpected {
                            object_id: object_ref.object_id
                        }
                        .into()
                    ),
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                };
            }

            fp_ensure!(
                !objects_in_txn.contains(&object_ref.object_id),
                UserInputError::DuplicateObjectRefInput.into()
            );

            objects_in_txn.insert(object_ref.object_id);
        }
        Ok(())
    }

    /// Check transaction gas data/info and gas coins consistency.
    /// Return the gas status to be used for the lifecycle of the transaction.
    #[instrument(level = "trace", skip_all)]
    fn check_gas(
        objects: &InputObjects,
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        gas: &[ObjectReference],
        gas_price: u64,
        transaction_gas_budget: u64,
        authentication_gas_budget: u64,
        is_execute_transaction_to_effects: bool,
        relaxations: InputCheckRelaxations,
    ) -> IotaResult<IotaGasStatus> {
        let gas_budget_to_set = if authentication_gas_budget > 0 {
            // If there is an authentication gas budget, then we are checking if
            // max_gas_budget is Some. If not, that is UserInputError.
            let protocol_max_auth_gas =
                protocol_config.max_auth_gas_as_option().ok_or_else(|| {
                    UserInputError::Unsupported(
                        "Transaction requires authentication gas but max_auth_gas is not enabled"
                            .to_string(),
                    )
                })?;

            // Execution phase:
            //  - meter transaction + authentication;
            //  - it needs the full budget.
            // Signing phase:
            //  - meter only authentication;
            //  - it only needs authentication budget.
            if is_execute_transaction_to_effects {
                transaction_gas_budget
            } else {
                authentication_gas_budget.min(protocol_max_auth_gas)
            }
        } else {
            // If there is no authentication gas budget, then we are only checking the
            // transaction gas budget.
            transaction_gas_budget
        };

        // Budget to check is always the one set by the user (which should cover full
        // transaction + authentication costs).
        let gas_budget_to_check = transaction_gas_budget;

        let gas_status = IotaGasStatus::new(
            gas_budget_to_set,
            gas_price,
            reference_gas_price,
            protocol_config,
        )?;

        // Check balance and coins consistency
        // Load all gas coins
        let objects: BTreeMap<_, _> = objects.iter().map(|o| (o.id(), o)).collect();
        let mut gas_objects = vec![];
        for obj_ref in gas {
            let obj = objects.get(&obj_ref.object_id);
            let obj = *obj.ok_or(UserInputError::ObjectNotFound {
                object_id: obj_ref.object_id,
                version: Some(obj_ref.version),
            })?;
            gas_objects.push(obj);
        }
        gas_status.check_gas_balance(
            &gas_objects,
            gas_budget_to_check,
            !relaxations.unbounded_gas_budget,
        )?;
        Ok(gas_status)
    }

    /// Check all the objects used in the transaction against the database, and
    /// ensure that they are all the correct version and number.
    #[instrument(level = "trace", skip_all)]
    fn check_objects(
        transaction: &Transaction,
        objects: &InputObjects,
        relaxations: InputCheckRelaxations,
    ) -> UserInputResult<()> {
        // We require that mutable objects cannot show up more than once.
        let mut used_objects: HashSet<Address> = HashSet::new();
        for object in objects.iter() {
            if object.is_mutable() {
                fp_ensure!(
                    used_objects.insert(object.id().into()),
                    UserInputError::MutableObjectUsedMoreThanOnce {
                        object_id: object.id()
                    }
                );
            }
        }

        if !transaction.is_genesis_tx() && objects.is_empty() {
            return Err(UserInputError::ObjectInputArityViolation);
        }

        let gas_coins: HashSet<ObjectId> =
            HashSet::from_iter(transaction.gas().iter().map(|obj_ref| obj_ref.object_id));
        for object in objects.iter() {
            let input_object_kind = object.input_object_kind;

            match &object.object {
                ObjectReadResultKind::Object(object) => {
                    // For Gas Object, we check the object is owned by gas owner
                    let owner_address = if gas_coins.contains(&object.id()) {
                        transaction.gas_owner()
                    } else {
                        transaction.sender()
                    };
                    // Check if the object contents match the type of lock we need for
                    // this object.
                    let system_transaction = transaction.is_system_tx();
                    check_one_object(
                        &owner_address,
                        input_object_kind,
                        object,
                        system_transaction,
                        relaxations,
                    )?;
                }
                // We skip checking a deleted shared object because it no longer exists
                ObjectReadResultKind::DeletedSharedObject(_, _) => (),
                // We skip checking shared objects from cancelled transactions since we are not
                // reading it.
                ObjectReadResultKind::CancelledTransactionObject(_) => (),
            }
        }

        Ok(())
    }

    /// Check one object against a reference
    fn check_one_object(
        owner: &Address,
        object_kind: InputObjectKind,
        object: &Object,
        system_transaction: bool,
        relaxations: InputCheckRelaxations,
    ) -> UserInputResult {
        match object_kind {
            InputObjectKind::MovePackage(package_id) => {
                fp_ensure!(
                    object.data.as_opt_package().is_some(),
                    UserInputError::MoveObjectAsPackage {
                        object_id: package_id
                    }
                );
            }
            InputObjectKind::ImmOrOwnedMoveObject(object_ref) => {
                fp_ensure!(
                    !object.is_package(),
                    UserInputError::MovePackageAsObject {
                        object_id: object_ref.object_id
                    }
                );
                fp_ensure!(
                    object_ref.version < Version::MAX_VALID_EXCL,
                    UserInputError::InvalidSequenceNumber
                );

                // This is an invariant - we just load the object with the given ID and version.
                assert_eq!(
                    object.version(),
                    object_ref.version,
                    "The fetched object version {} does not match the requested version {}, object id: {}",
                    object.version(),
                    object_ref.version,
                    object.id(),
                );

                // Check the digest matches - user could give a mismatched ObjectDigest
                if !relaxations.any_object_digest {
                    let expected_digest = object.digest();
                    fp_ensure!(
                        expected_digest == object_ref.digest,
                        UserInputError::InvalidObjectDigest {
                            object_id: object_ref.object_id,
                            expected_digest
                        }
                    );
                }

                match object.owner {
                    Owner::Immutable => {
                        // Nothing else to check for Immutable.
                    }
                    Owner::Address(actual_owner) => {
                        // Check the owner is correct. Only this arm is relaxed: whether
                        // the sender owns the object is a question of permission, which
                        // a simulation may ask past. The arms below are not — those
                        // objects cannot be owned inputs at all, and the engine treats
                        // the checks here as having established that.
                        if !relaxations.any_object_owner {
                            fp_ensure!(
                                owner == &actual_owner,
                                UserInputError::IncorrectUserSignature {
                                    error: format!(
                                        "Object {} is owned by account address {}, but given owner/signer address is {}",
                                        object_ref.object_id, actual_owner, owner
                                    ),
                                }
                            );
                        }
                    }
                    Owner::Object(owner) => {
                        return Err(UserInputError::InvalidChildObjectArgument {
                            child_id: object.id(),
                            parent_id: owner,
                        });
                    }
                    Owner::Shared(_) => {
                        // This object is a mutable shared object. However the transaction
                        // specifies it as an owned object. This is inconsistent.
                        return Err(UserInputError::NotSharedObject);
                    }
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                };
            }
            InputObjectKind::SharedMoveObject {
                id: ObjectId::CLOCK,
                initial_shared_version: IOTA_CLOCK_OBJECT_SHARED_VERSION,
                mutable: true,
            } => {
                // Only system transactions can accept the Clock
                // object as a mutable parameter.
                if system_transaction {
                    return Ok(());
                } else {
                    return Err(UserInputError::ImmutableParameterExpected {
                        object_id: ObjectId::CLOCK,
                    });
                }
            }
            InputObjectKind::SharedMoveObject {
                id: ObjectId::AUTHENTICATOR_STATE,
                ..
            } => {
                if system_transaction {
                    return Ok(());
                } else {
                    return Err(UserInputError::InaccessibleSystemObject {
                        object_id: ObjectId::AUTHENTICATOR_STATE,
                    });
                }
            }
            InputObjectKind::SharedMoveObject {
                id: ObjectId::RANDOMNESS_STATE,
                mutable: true,
                ..
            } => {
                // Only system transactions can accept the Random
                // object as a mutable parameter.
                if system_transaction {
                    return Ok(());
                } else {
                    return Err(UserInputError::ImmutableParameterExpected {
                        object_id: ObjectId::RANDOMNESS_STATE,
                    });
                }
            }
            InputObjectKind::SharedMoveObject {
                initial_shared_version: input_initial_shared_version,
                ..
            } => {
                fp_ensure!(
                    object.version() < Version::MAX_VALID_EXCL,
                    UserInputError::InvalidSequenceNumber
                );

                match object.owner {
                    Owner::Address(_) | Owner::Object(_) | Owner::Immutable => {
                        // When someone locks an object as shared it must be shared already.
                        return Err(UserInputError::NotSharedObject);
                    }
                    Owner::Shared(actual_initial_shared_version) => {
                        fp_ensure!(
                            input_initial_shared_version == actual_initial_shared_version,
                            UserInputError::SharedObjectStartingVersionMismatch
                        )
                    }
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }
        };
        Ok(())
    }

    /// Check all the `MoveAuthenticator` related input objects against the
    /// database.
    #[instrument(level = "trace", skip_all)]
    fn check_move_authenticator_objects(
        authenticator_objects: &InputObjects,
    ) -> UserInputResult<()> {
        for object in authenticator_objects.iter() {
            let input_object_kind = object.input_object_kind;

            match &object.object {
                ObjectReadResultKind::Object(object) => {
                    check_one_move_authenticator_object(input_object_kind, object)?;
                }
                // We skip checking a deleted shared object because it no longer exists.
                ObjectReadResultKind::DeletedSharedObject(_, _) => (),
                // We skip checking shared objects from cancelled transactions since we are not
                // reading it.
                ObjectReadResultKind::CancelledTransactionObject(_) => (),
            }
        }

        Ok(())
    }

    /// Check one `MoveAuthenticator` input object.
    fn check_one_move_authenticator_object(
        object_kind: InputObjectKind,
        object: &Object,
    ) -> UserInputResult {
        match object_kind {
            InputObjectKind::MovePackage(package_id) => {
                return Err(UserInputError::PackageIsInMoveAuthenticatorInput { package_id });
            }
            InputObjectKind::ImmOrOwnedMoveObject(object_ref) => {
                fp_ensure!(
                    !object.is_package(),
                    UserInputError::MovePackageAsObject {
                        object_id: object_ref.object_id
                    }
                );
                fp_ensure!(
                    object_ref.version < Version::MAX_VALID_EXCL,
                    UserInputError::InvalidSequenceNumber
                );

                // This is an invariant - we just load the object with the given ID and version.
                assert_eq!(
                    object.version(),
                    object_ref.version,
                    "The fetched object version {} does not match the requested version {}, object id: {}",
                    object.version(),
                    object_ref.version,
                    object.id(),
                );

                // Check the digest matches - user could give a mismatched `ObjectDigest`.
                let expected_digest = object.digest();
                fp_ensure!(
                    expected_digest == object_ref.digest,
                    UserInputError::InvalidObjectDigest {
                        object_id: object_ref.object_id,
                        expected_digest
                    }
                );

                match object.owner {
                    Owner::Immutable => {
                        // Nothing else to check for Immutable.
                    }
                    Owner::Address(_) => {
                        return Err(UserInputError::AddressOwnedIsInMoveAuthenticatorInput {
                            object_id: object.id(),
                        });
                    }
                    Owner::Object(_) => {
                        return Err(UserInputError::ObjectOwnedIsInMoveAuthenticatorInput {
                            object_id: object.id(),
                        });
                    }
                    Owner::Shared(_) => {
                        // This object is a mutable shared object. However the transaction
                        // specifies it as an owned object. This is inconsistent.
                        return Err(UserInputError::NotSharedObject);
                    }
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                };
            }
            InputObjectKind::SharedMoveObject {
                id: IOTA_AUTHENTICATOR_STATE_OBJECT_ID,
                ..
            } => {
                return Err(UserInputError::InaccessibleSystemObject {
                    object_id: IOTA_AUTHENTICATOR_STATE_OBJECT_ID,
                });
            }
            InputObjectKind::SharedMoveObject {
                id, mutable: true, ..
            } => {
                return Err(UserInputError::MutableSharedIsInMoveAuthenticatorInput {
                    object_id: id,
                });
            }
            InputObjectKind::SharedMoveObject {
                initial_shared_version: input_initial_shared_version,
                ..
            } => {
                fp_ensure!(
                    object.version() < Version::MAX_VALID_EXCL,
                    UserInputError::InvalidSequenceNumber
                );

                match object.owner {
                    Owner::Address(_) | Owner::Object(_) | Owner::Immutable => {
                        // When someone locks an object as shared it must be shared already.
                        return Err(UserInputError::NotSharedObject);
                    }
                    Owner::Shared(actual_initial_shared_version) => {
                        fp_ensure!(
                            input_initial_shared_version == actual_initial_shared_version,
                            UserInputError::SharedObjectStartingVersionMismatch
                        )
                    }
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }
        };
        Ok(())
    }

    /// Create a union of two CheckedInputObjects, ensuring consistency
    /// for objects that appear in both sets. The base_set is consumed and
    /// returned with the union. The other_set is borrowed.
    /// In the case of shared objects, the mutability can differ, but the
    /// initial shared version must match. For other object kinds, they must
    /// match exactly.
    pub fn checked_input_objects_union(
        base_set: CheckedInputObjects,
        other_set: &CheckedInputObjects,
    ) -> IotaResult<CheckedInputObjects> {
        let mut base_set = base_set.into_inner();
        for other_object in other_set.inner().iter() {
            if let Some(base_object) = base_set.find_object_id_mut(other_object.id()) {
                // This is an invariant
                assert_eq!(
                    base_object.object, other_object.object,
                    "The object read result for input objects with the same id must be equal"
                );

                // In the case of an alive object, check that the object kind matches exactly,
                // or that, if it is a shared object, only the mutability changes
                if let ObjectReadResultKind::Object(_) = &other_object.object {
                    base_object
                        .input_object_kind
                        .left_union_with_checks(&other_object.input_object_kind)?;
                }
            } else {
                base_set.push(other_object.clone());
            }
        }
        Ok(base_set.into_checked())
    }

    /// Check package verification timeout
    #[instrument(level = "trace", skip_all)]
    pub fn check_non_system_packages_to_be_published(
        transaction: &Transaction,
        protocol_config: &ProtocolConfig,
        metrics: &Arc<BytecodeVerifierMetrics>,
        verifier_signing_config: &VerifierSigningConfig,
    ) -> UserInputResult<()> {
        // Only meter non-system programmable transaction blocks
        if transaction.is_system_tx() {
            return Ok(());
        }

        let TransactionKind::Programmable(pt) = transaction.kind() else {
            return Ok(());
        };

        // Use the same verifier and meter for all packages, custom configured for
        // signing.
        let signing_limits = Some(verifier_signing_config.limits_for_signing());
        let mut verifier = iota_execution::verifier(protocol_config, signing_limits, metrics);
        let mut meter = verifier.meter(verifier_signing_config.meter_config_for_signing());

        // Measure time for verifying all packages in the PTB
        let shared_meter_verifier_timer = metrics
            .verifier_runtime_per_ptb_success_latency
            .start_timer();

        let verifier_status = pt
            .non_system_packages_to_be_published()
            .try_for_each(|module_bytes| {
                verifier.meter_module_bytes(protocol_config, module_bytes, meter.as_mut())
            })
            .map_err(|e| UserInputError::PackageVerificationTimedout { err: e.to_string() });

        match verifier_status {
            Ok(_) => {
                // Success: stop and record the success timer
                shared_meter_verifier_timer.stop_and_record();
            }
            Err(err) => {
                // Failure: redirect the success timers output to the failure timer and
                // discard the success timer
                metrics
                    .verifier_runtime_per_ptb_timeout_latency
                    .observe(shared_meter_verifier_timer.stop_and_discard());
                return Err(err);
            }
        };

        Ok(())
    }
}
