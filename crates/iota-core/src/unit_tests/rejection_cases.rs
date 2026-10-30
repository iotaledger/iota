// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transactions that validation rejects, shared by the tests of admission
//! (`submit_tx`) and of post-consensus validation, so both are checked against
//! the same inputs.

use iota_sdk_types::{
    Address, ConsensusCommitDigest, ConsensusCommitPrologueV1,
    ConsensusDeterminedVersionAssignments, GasPayment, GenesisTransaction, Identifier, ObjectId,
    ObjectReference, Transaction, TransactionKind,
};
use iota_types::{
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair},
    error::{IotaError, UserInputError},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, SenderSignedTransactionAPI, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI,
        TransactionEnvelope,
    },
    utils::{to_sender_signed_transaction, to_sender_signed_transaction_with_multi_signers},
};

use crate::authority::AuthorityState;

type Check = Box<dyn Fn(&IotaError)>;

/// A transaction that must be rejected.
pub(crate) struct RejectionCase {
    pub name: String,
    pub tx: TransactionEnvelope,
    /// Checks the error `submit_tx` rejects the transaction with.
    pub at_submit: Check,
    /// Checks the error the transaction is dropped with at a commit. `None`
    /// when the commit does not look at what makes the transaction invalid.
    pub at_commit: Option<Check>,
}

fn user_input(error: &IotaError) -> &UserInputError {
    match error {
        IotaError::UserInput { error } => error,
        other => panic!("expected a user input error, got {other:?}"),
    }
}

fn user_input_is(expected: UserInputError) -> Check {
    Box::new(move |error| assert_eq!(user_input(error), &expected))
}

fn user_input_matches(matches: fn(&UserInputError) -> bool) -> Check {
    Box::new(move |error| {
        let error = user_input(error);
        assert!(matches(error), "unexpected error: {error:?}");
    })
}

fn gas_object_not_owned() -> Check {
    user_input_matches(|e| matches!(e, UserInputError::GasObjectNotOwnedObject { .. }))
}

fn incorrect_user_signature() -> Check {
    user_input_matches(|e| matches!(e, UserInputError::IncorrectUserSignature { .. }))
}

#[derive(Clone, Copy)]
enum Flavor {
    Transfer,
    SystemCall,
}

struct Env<'a> {
    state: &'a AuthorityState,
    sender: Address,
    key: AccountPrivateKey,
    rgp: u64,
}

impl Env<'_> {
    fn insert(&self, object: Object) -> ObjectReference {
        let object_ref = object.object_ref();
        self.state.insert_genesis_object(object);
        object_ref
    }

    fn owned(&self) -> ObjectReference {
        self.insert(Object::with_owner_for_testing(self.sender))
    }

    fn gas(&self, balance: u64) -> ObjectReference {
        self.insert(Object::with_id_owner_gas_for_testing(
            ObjectId::random(),
            self.sender,
            balance,
        ))
    }

    fn budget(&self) -> u64 {
        self.rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER
    }

    fn sign(&self, tx: Transaction) -> TransactionEnvelope {
        to_sender_signed_transaction(tx, &self.key)
    }

    fn transfer(&self, gas: ObjectReference, budget: u64, price: u64) -> Transaction {
        Transaction::new_transfer(dbg_addr(2), self.owned(), self.sender, gas, budget, price)
    }

    /// A valid transaction of the given kind, with a gas coin and, for the
    /// transfer, an object of its own.
    fn valid(&self, flavor: Flavor) -> Transaction {
        let gas = self.gas(iota_types::object::GAS_VALUE_FOR_TESTING);
        match flavor {
            Flavor::Transfer => self.transfer(gas, self.budget(), self.rgp),
            Flavor::SystemCall => Transaction::new_move_call(
                self.sender,
                ObjectId::SYSTEM,
                Identifier::IOTA_SYSTEM_MODULE,
                Identifier::from_static("request_add_validator"),
                vec![],
                gas,
                vec![CallArg::IOTA_SYSTEM_MUTABLE],
                self.budget(),
                self.rgp,
            )
            .unwrap(),
        }
    }

    fn with_gas_coins(
        &self,
        kind: TransactionKind,
        coins: Vec<ObjectReference>,
        budget: u64,
    ) -> TransactionEnvelope {
        self.sign(Transaction::new_with_gas_coins(
            kind,
            self.sender,
            coins,
            budget,
            self.rgp,
        ))
    }

    fn transfer_iota_kind(&self) -> TransactionKind {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(dbg_addr(2), None);
        TransactionKind::Programmable(builder.finish())
    }
}

/// The rejected transactions of `state`'s test environment, each over objects
/// that were added to `state` for it alone.
pub(crate) fn rejection_cases(state: &AuthorityState) -> Vec<RejectionCase> {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let rgp = state.reference_gas_price_for_testing().unwrap();
    let env = Env {
        state,
        sender,
        key,
        rgp,
    };
    let protocol_config = state.epoch_store_for_testing().protocol_config().clone();
    let min_budget = protocol_config.base_tx_cost_fixed() * rgp;
    let max_budget = protocol_config.max_tx_gas();

    let mut cases = Vec::new();
    let mut add =
        |name: &str, tx: TransactionEnvelope, at_submit: Check, at_commit: Option<Check>| {
            cases.push(RejectionCase {
                name: name.to_owned(),
                tx,
                at_submit,
                at_commit,
            });
        };

    for (flavor, label) in [(Flavor::Transfer, "transfer"), (Flavor::SystemCall, "call")] {
        let (_, unknown_key): (Address, AccountPrivateKey) = get_key_pair();
        add(
            &format!("{label}: signed by an unknown key"),
            to_sender_signed_transaction(env.valid(flavor), &unknown_key),
            Box::new(|e| {
                assert!(
                    matches!(e, IotaError::SignerSignatureAbsent { .. }),
                    "{e:?}"
                )
            }),
            None,
        );

        let mut tx = env.sign(env.valid(flavor));
        *tx.data_mut_for_testing().tx_signatures_mut_for_testing() = vec![];
        add(
            &format!("{label}: no signature"),
            tx,
            Box::new(|e| {
                assert!(
                    matches!(
                        e,
                        IotaError::SignerSignatureNumberMismatch {
                            expected: 1,
                            actual: 0
                        }
                    ),
                    "{e:?}"
                )
            }),
            None,
        );

        let mut tx = env.sign(env.valid(flavor));
        let sigs = tx.data_mut_for_testing().tx_signatures_mut_for_testing();
        sigs.push(sigs[0].clone());
        add(
            &format!("{label}: extra signature"),
            tx,
            Box::new(|e| {
                assert!(
                    matches!(
                        e,
                        IotaError::SignerSignatureNumberMismatch {
                            expected: 1,
                            actual: 2
                        }
                    ),
                    "{e:?}"
                )
            }),
            None,
        );

        let mut tx = env.valid(flavor);
        let new_addr = Address::random();
        tx.gas_data_mut().owner = new_addr;
        *tx.sender_mut_for_testing() = new_addr;
        add(
            &format!("{label}: sender and gas owner replaced after signing the data"),
            env.sign(tx),
            Box::new(|e| {
                assert!(
                    matches!(e, IotaError::SignerSignatureAbsent { .. }),
                    "{e:?}"
                )
            }),
            None,
        );

        let mut tx = env.valid(flavor);
        tx.gas_data_mut().owner = Address::random();
        add(
            &format!("{label}: gas owner is not the signer"),
            env.sign(tx),
            Box::new(|e| {
                assert!(
                    matches!(
                        e,
                        IotaError::SignerSignatureNumberMismatch {
                            expected: 2,
                            actual: 1
                        }
                    ),
                    "{e:?}"
                )
            }),
            None,
        );

        let mut tx = env.valid(flavor);
        tx.gas_data_mut().objects = vec![];
        add(
            &format!("{label}: empty gas"),
            env.sign(tx),
            user_input_is(UserInputError::MissingGasPayment),
            Some(user_input_is(UserInputError::MissingGasPayment)),
        );

        let mut tx = env.valid(flavor);
        let gas_data = tx.gas_data_mut();
        let duplicate = gas_data.objects[0];
        gas_data.objects.push(duplicate);
        let mutable_object_used_twice = || {
            user_input_matches(|e| {
                matches!(e, UserInputError::MutableObjectUsedMoreThanOnce { .. })
            })
        };
        add(
            &format!("{label}: duplicate gas"),
            env.sign(tx),
            mutable_object_used_twice(),
            Some(mutable_object_used_twice()),
        );

        let system_kinds = [
            (
                "genesis",
                TransactionKind::Genesis(GenesisTransaction {
                    objects: vec![],
                    events: vec![],
                }),
            ),
            (
                "consensus commit prologue",
                TransactionKind::ConsensusCommitPrologueV1(ConsensusCommitPrologueV1 {
                    epoch: 0,
                    round: 0,
                    sub_dag_index: None,
                    commit_timestamp_ms: 42,
                    consensus_commit_digest: ConsensusCommitDigest::default(),
                    consensus_determined_version_assignments:
                        ConsensusDeterminedVersionAssignments::CanceledTransactions {
                            canceled_transactions: Vec::new(),
                        },
                }),
            ),
            ("end of epoch", TransactionKind::EndOfEpoch(vec![])),
        ];
        for (kind_name, kind) in system_kinds {
            let mut tx = env.valid(flavor);
            *tx.kind_mut() = kind;
            let unsupported =
                || user_input_matches(|e| matches!(e, UserInputError::Unsupported { .. }));
            add(
                &format!("{label}: user sends a {kind_name} transaction"),
                env.sign(tx),
                unsupported(),
                Some(unsupported()),
            );
        }
    }

    // Gas budget, balance and price.
    let balance = iota_types::object::GAS_VALUE_FOR_TESTING;
    let gas = env.gas(max_budget);
    let tx = env.transfer(gas, min_budget - rgp, rgp);
    add(
        "gas budget below the minimum",
        env.sign(tx),
        user_input_is(UserInputError::GasBudgetTooLow {
            gas_budget: min_budget - rgp,
            min_budget,
        }),
        Some(user_input_is(UserInputError::GasBudgetTooLow {
            gas_budget: min_budget - rgp,
            min_budget,
        })),
    );

    let gas = env.gas(max_budget);
    let tx = env.transfer(gas, max_budget + 1, rgp);
    let too_high = || {
        user_input_is(UserInputError::GasBudgetTooHigh {
            gas_budget: max_budget + 1,
            max_budget,
        })
    };
    add(
        "gas budget above the maximum",
        env.sign(tx),
        too_high(),
        Some(too_high()),
    );

    let gas = env.gas(min_budget - 1);
    let tx = env.transfer(gas, min_budget, rgp);
    let balance_too_low = |gas_balance: u64, needed_gas_amount: u64| {
        move || {
            user_input_is(UserInputError::GasBalanceTooLow {
                gas_balance: gas_balance.into(),
                needed_gas_amount: needed_gas_amount.into(),
            })
        }
    };
    add(
        "gas balance below the budget",
        env.sign(tx),
        balance_too_low(min_budget - 1, min_budget)(),
        Some(balance_too_low(min_budget - 1, min_budget)()),
    );

    let gas = env.gas(max_budget);
    let tx = env.transfer(gas, min_budget, rgp - 1);
    let under_rgp = || user_input_matches(|e| matches!(e, UserInputError::GasPriceUnderRGP { .. }));
    add(
        "gas price below the reference gas price",
        env.sign(tx),
        under_rgp(),
        Some(under_rgp()),
    );

    // Paying out of coins that cannot cover the budget.
    let pay = |recipients: Vec<Address>, amounts: Vec<u64>| {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_iota(recipients, amounts).unwrap();
        TransactionKind::Programmable(builder.finish())
    };
    let pay_all = || {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_all_iota(dbg_addr(2));
        TransactionKind::Programmable(builder.finish())
    };
    let pay_recipients = || vec![dbg_addr(1), dbg_addr(2)];
    let pay_cases = [
        (
            "pay from one coin",
            pay(pay_recipients(), vec![100, 100]),
            vec![2000],
            2_200_000,
        ),
        (
            "pay from two coins",
            pay(pay_recipients(), vec![100, 100]),
            vec![800, 700],
            2_000_000,
        ),
        ("pay all from one coin", pay_all(), vec![1800], 2_000_000),
        (
            "pay all from two coins",
            pay_all(),
            vec![1000, 1000],
            2_500_000,
        ),
    ];
    for (name, kind, balances, budget) in pay_cases {
        let coins = balances.iter().map(|b| env.gas(*b)).collect();
        let total: u64 = balances.iter().sum();
        add(
            &format!("gas balance below the budget: {name}"),
            env.with_gas_coins(kind, coins, budget),
            balance_too_low(total, budget)(),
            Some(balance_too_low(total, budget)()),
        );
    }

    let mut builder = ProgrammableTransactionBuilder::new();
    for _ in 0..10 {
        builder.transfer_iota(dbg_addr(2), Some(1));
    }
    let gas = env.gas(env.budget() - 1);
    add(
        "gas balance below the budget: ten commands",
        env.with_gas_coins(
            TransactionKind::Programmable(builder.finish()),
            vec![gas],
            env.budget(),
        ),
        balance_too_low(env.budget() - 1, env.budget())(),
        Some(balance_too_low(env.budget() - 1, env.budget())()),
    );

    // Gas coins the sender may not use.
    let sponsor = Address::random();
    let sponsor_gas = env.insert(Object::with_owner_for_testing(sponsor));
    let tx = Transaction::new_with_gas_data(
        env.transfer_iota_kind(),
        sender,
        GasPayment {
            objects: vec![sponsor_gas],
            owner: sender,
            price: rgp,
            budget: env.budget(),
        },
    );
    add(
        "sponsored: the sender is named as gas owner",
        env.sign(tx),
        incorrect_user_signature(),
        Some(incorrect_user_signature()),
    );

    let (third_party, third_party_key): (Address, AccountPrivateKey) = get_key_pair();
    let sponsor_gas = env.insert(Object::with_owner_for_testing(sponsor));
    let tx = Transaction::new_with_gas_data(
        env.transfer_iota_kind(),
        sender,
        GasPayment {
            objects: vec![sponsor_gas],
            owner: third_party,
            price: rgp,
            budget: env.budget(),
        },
    );
    add(
        "sponsored: the gas owner who signs does not own the gas coin",
        to_sender_signed_transaction_with_multi_signers(tx, vec![&env.key, &third_party_key]),
        incorrect_user_signature(),
        Some(incorrect_user_signature()),
    );

    let single_gas_coin_tx = |gas: ObjectReference| {
        env.sign(Transaction::new_transfer_iota(
            dbg_addr(2),
            sender,
            None,
            gas,
            env.budget(),
            rgp,
        ))
    };
    let immutable_gas = env.insert(Object::immutable_with_id_for_testing(ObjectId::random()));
    add(
        "gas coin is immutable",
        single_gas_coin_tx(immutable_gas),
        gas_object_not_owned(),
        Some(gas_object_not_owned()),
    );

    let parent = env.owned();
    let child_gas = env.insert(Object::with_object_owner_for_testing(
        ObjectId::random(),
        parent.object_id,
    ));
    add(
        "gas coin is owned by an object",
        single_gas_coin_tx(child_gas),
        gas_object_not_owned(),
        Some(gas_object_not_owned()),
    );

    // A second gas coin the sender may not use, next to a valid one.
    let second_coin_cases = [
        (
            "shared",
            Object::shared_for_testing(),
            gas_object_not_owned as fn() -> Check,
        ),
        (
            "immutable",
            Object::immutable_with_id_for_testing(ObjectId::random()),
            gas_object_not_owned,
        ),
        (
            "owned by an object",
            Object::with_object_owner_for_testing(ObjectId::random(), env.owned().object_id),
            gas_object_not_owned,
        ),
        (
            "owned by another address",
            Object::with_owner_for_testing(Address::ZERO),
            incorrect_user_signature,
        ),
    ];
    for (name, bad_coin, check) in second_coin_cases {
        let good = env.gas(balance);
        let bad = env.insert(bad_coin);
        add(
            &format!("second gas coin is {name}"),
            env.with_gas_coins(env.transfer_iota_kind(), vec![good, bad], max_budget),
            check(),
            None,
        );
    }

    // Owned inputs the sender may not use.
    let other = Address::random();
    let foreign_object = env.insert(Object::with_owner_for_testing(other));
    let gas = env.gas(balance);
    let tx = Transaction::new_transfer(dbg_addr(2), foreign_object, sender, gas, env.budget(), rgp);
    add(
        "input object is owned by another address",
        env.sign(tx),
        incorrect_user_signature(),
        Some(incorrect_user_signature()),
    );

    let (unknown, unknown_key): (Address, AccountPrivateKey) = get_key_pair();
    let object = env.owned();
    let gas = env.gas(balance);
    let tx = Transaction::new_transfer(dbg_addr(2), object, unknown, gas, env.budget(), rgp);
    add(
        "sender owns neither the input nor the gas coin",
        to_sender_signed_transaction(tx, &unknown_key),
        incorrect_user_signature(),
        Some(incorrect_user_signature()),
    );

    cases
}
