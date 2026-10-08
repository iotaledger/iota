// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Rotation rules for `SmartAccount`: let other modules limit how an account's authenticator and
/// fields are changed.
///
/// A module can attach a rule to an account, identified by a witness type `Rule` that only that
/// module can create. While rules are attached, `smart_account::rotate_auth_function_ref_v1`
/// aborts, and rotating the authenticator is a three-step request, like a `TransferRequest` under
/// a `TransferPolicy`:
///
/// ```move
/// let mut request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
///     &account,
///     new_authenticator,
///     ctx,
/// );
/// some_rule_module::approve(&account, &mut request); // once per rule attached to the account
/// smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(&mut account, request, ctx);
/// ```
///
/// Each rule module checks the request and adds its receipt with `add_auth_rotation_receipt`; the
/// confirmation aborts unless every attached rule added one.
///
/// A rule is attached together with a `Config` value of the rule module's choice, stored on the
/// account under `AuthRotationRuleKey<Rule>`. Unlike a `TransferPolicy`, where the policy owner
/// can remove any rule, only the module that defines `Rule` can add or remove it: a rule protects
/// the account from whoever controls its current authenticator, so that party must not be able
/// to drop it.
///
/// # Fields
/// An attached rule can also guard a field type `Name` (`add_field_rotation_rule`). While any rule
/// guards `Name`, `smart_account`'s field functions abort for it, and every change to it (adding,
/// removing, replacing or mutably borrowing the field) is a request that every rule guarding
/// `Name` must approve:
///
/// ```move
/// let mut request = smart_account_rotation_rules::request_field_change_v1<Name>(
///     &account,
///     smart_account_rotation_rules::field_remove(),
///     ctx,
/// );
/// some_rule_module::approve_field_change(&account, &mut request); // once per guarding rule
/// let value = smart_account_rotation_rules::remove_field(&mut account, request, name, ctx);
/// ```
///
/// The built-in authenticator's public key field (`builtin_authenticator_functions` module's
/// `PublicKeyFieldName`) can't be guarded: `smart_account_builtin_auth` changes it directly, always
/// together with the authenticator, and its `BuiltinAuthRule` approves those rotations.
///
/// # Approvals and later changes
/// A rule checks the account when it adds its receipt, but the change happens at confirmation.
/// A confirmation therefore aborts if another rotation or guarded field change was confirmed on
/// the account after the request was made, so every receipt still holds for the state the
/// change applies to. A rule should only depend on the authenticator and on fields it guards:
/// changes to other fields don't count.
module iota::smart_account_rotation_rules;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions::PublicKeyFieldName;
use iota::dynamic_field;
use iota::event;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};
use iota::vec_set::{Self, VecSet};
use std::type_name::{Self, TypeName};

// === Errors ===

#[error(code = 0)]
const EAuthRotationRuleAlreadyAttached: vector<u8> =
    b"The rotation rule is already attached to the account.";
#[error(code = 1)]
const EAuthRotationRuleNotAttached: vector<u8> =
    b"The rotation rule is not attached to the account.";
#[error(code = 2)]
const EAuthRotationRulesNotSatisfied: vector<u8> =
    b"The receipts of the rotation request do not match the rotation rules of the account.";
#[error(code = 3)]
const EAuthRotationRequestForAnotherAccount: vector<u8> =
    b"The rotation request was made for another account.";
#[error(code = 4)]
const EFieldRotationRuleAlreadyAttached: vector<u8> =
    b"The rotation rule already guards the field type.";
#[error(code = 5)]
const EFieldRotationRuleNotAttached: vector<u8> =
    b"The rotation rule does not guard the field type.";
#[error(code = 6)]
const EAuthRotationRuleGuardsFields: vector<u8> =
    b"The rotation rule still guards field types; remove those first.";
#[error(code = 7)]
const EFieldChangeOperationMismatch: vector<u8> =
    b"The field change request was made for another operation.";
#[error(code = 8)]
const EAccountChangedAfterRequest: vector<u8> =
    b"Another rotation or guarded field change was confirmed after the request was made.";
#[error(code = 9)]
const EFieldTypeNotGuardable: vector<u8> =
    b"The built-in authenticator's public key field can't be guarded.";

// === Constants ===

const FIELD_ADD: u8 = 0;
const FIELD_REMOVE: u8 = 1;
const FIELD_ROTATE: u8 = 2;
const FIELD_BORROW_MUT: u8 = 3;

// === Structs ===

/// A request to rotate the account's authenticator to `authenticator`, created by
/// `request_auth_function_ref_rotation_v1`.
///
/// It has no abilities, so the transaction can only end after
/// `confirm_auth_function_ref_rotation_v1` consumes it.
public struct AuthRotationRequest {
    account_id: ID,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    change_count: u64,
    receipts: VecSet<TypeName>,
}

/// Dynamic field key of the set of rotation rules attached to an account.
public struct AuthRotationRulesKey has copy, drop, store {}

/// Dynamic field key of the configuration of the rotation rule `Rule`.
public struct AuthRotationRuleKey<phantom Rule: drop> has copy, drop, store {}

/// A request to change a field of type `Name` with `operation` (see `field_add`, `field_remove`,
/// `field_rotate` and `field_borrow_mut`), created by `request_field_change_v1`.
///
/// It has no abilities, so the transaction can only end after the matching guarded field
/// function consumes it.
public struct FieldChangeRequest<phantom Name> {
    account_id: ID,
    operation: u8,
    change_count: u64,
    receipts: VecSet<TypeName>,
}

/// Dynamic field key of the set of rotation rules guarding the field type `Name`.
public struct FieldRotationRulesKey<phantom Name> has copy, drop, store {}

/// Dynamic field key of the set of field types guarded by the rotation rule `Rule`.
public struct AuthRotationRuleFieldsKey<phantom Rule: drop> has copy, drop, store {}

/// Dynamic field key of the number of rotations and guarded field changes confirmed on an
/// account.
public struct ChangeCountKey has copy, drop, store {}

// === Events ===

/// Event: emitted when a rotation rule is attached to an account.
public struct AuthRotationRuleAdded has copy, drop {
    account_id: ID,
    rule: TypeName,
}

/// Event: emitted when a rotation rule is removed from an account.
public struct AuthRotationRuleRemoved has copy, drop {
    account_id: ID,
    rule: TypeName,
}

/// Event: emitted when a rotation rule starts guarding a field type of an account.
public struct FieldRotationRuleAdded has copy, drop {
    account_id: ID,
    rule: TypeName,
    field: TypeName,
}

/// Event: emitted when a rotation rule stops guarding a field type of an account.
public struct FieldRotationRuleRemoved has copy, drop {
    account_id: ID,
    rule: TypeName,
    field: TypeName,
}

// === SmartAccountBuilder Functions ===

/// Attaches the rotation rule `Rule`, with its `config`, to the account being built.
///
/// Emits an `AuthRotationRuleAdded` event on success.
///
/// Aborts if `Rule` is already attached.
public fun with_auth_rotation_rule<Rule: drop, Config: store + drop>(
    mut builder: SmartAccountBuilder,
    _: Rule,
    config: Config,
): SmartAccountBuilder {
    insert_auth_rotation_rule<Rule, Config>(builder.borrow_uid_mut(), config);
    builder
}

/// Makes the rotation rule `Rule`, already attached to the account being built, guard the field
/// type `Name`.
///
/// Emits a `FieldRotationRuleAdded` event on success.
///
/// Aborts if `Rule` is not attached, or already guards `Name`.
/// Aborts if `Name` is the built-in authenticator's public key field.
public fun with_field_rotation_rule<Rule: drop, Name: copy + drop + store>(
    mut builder: SmartAccountBuilder,
    _: Rule,
): SmartAccountBuilder {
    insert_field_rotation_rule<Rule, Name>(builder.borrow_uid_mut());
    builder
}

// === AuthRotationRequest Functions ===

/// Adds the receipt of `Rule` to the rotation request, confirming that `Rule` approves it.
///
/// Called by the module that defines `Rule`, after checking the request.
public fun add_auth_rotation_receipt<Rule: drop>(self: &mut AuthRotationRequest, _: Rule) {
    self.receipts.insert(type_name::get<Rule>())
}

/// Returns the ID of the account the rotation request was made for.
public fun request_account_id(self: &AuthRotationRequest): ID {
    self.account_id
}

/// Borrows the authenticator the rotation request rotates to.
public fun request_authenticator(
    self: &AuthRotationRequest,
): &AuthenticatorFunctionRefV1<SmartAccount> {
    &self.authenticator
}

// === FieldChangeRequest Functions ===

/// The operation that adds a field (`add_field`).
public fun field_add(): u8 { FIELD_ADD }

/// The operation that removes a field (`remove_field`).
public fun field_remove(): u8 { FIELD_REMOVE }

/// The operation that replaces a field (`rotate_field`).
public fun field_rotate(): u8 { FIELD_ROTATE }

/// The operation that mutably borrows a field (`borrow_field_mut`).
public fun field_borrow_mut(): u8 { FIELD_BORROW_MUT }

/// Adds the receipt of `Rule` to the field change request, confirming that `Rule` approves it.
///
/// Called by the module that defines `Rule`, after checking the request.
public fun add_field_change_receipt<Rule: drop, Name>(
    self: &mut FieldChangeRequest<Name>,
    _: Rule,
) {
    self.receipts.insert(type_name::get<Rule>())
}

/// Returns the ID of the account the field change request was made for.
public fun field_request_account_id<Name>(self: &FieldChangeRequest<Name>): ID {
    self.account_id
}

/// Returns the operation of the field change request.
public fun field_request_operation<Name>(self: &FieldChangeRequest<Name>): u8 {
    self.operation
}

// === View Functions ===

/// Returns `true` if and only if the rotation rule `Rule` is attached to the account.
public fun has_auth_rotation_rule<Rule: drop>(account: &SmartAccount): bool {
    dynamic_field::exists_(account.uid(), AuthRotationRuleKey<Rule> {})
}

/// Borrows the configuration of the rotation rule `Rule`.
///
/// Aborts if `Rule` is not attached, or if its configuration is not a `Config`.
public fun borrow_auth_rotation_rule_config<Rule: drop, Config: store + drop>(
    account: &SmartAccount,
    _: Rule,
): &Config {
    assert!(has_auth_rotation_rule<Rule>(account), EAuthRotationRuleNotAttached);

    dynamic_field::borrow(account.uid(), AuthRotationRuleKey<Rule> {})
}

/// Returns the types of the rotation rules attached to the account, each of which must approve
/// a rotation of its authenticator.
public fun auth_rotation_rules(account: &SmartAccount): vector<TypeName> {
    if (!dynamic_field::exists_(account.uid(), AuthRotationRulesKey {})) return vector[];

    let rules: &VecSet<TypeName> = dynamic_field::borrow(account.uid(), AuthRotationRulesKey {});
    *rules.keys()
}

/// Returns `true` if and only if the rotation rule `Rule` guards the field type `Name`.
public fun has_field_rotation_rule<Rule: drop, Name: copy + drop + store>(
    account: &SmartAccount,
): bool {
    let field_rules = FieldRotationRulesKey<Name> {};
    if (!dynamic_field::exists_(account.uid(), field_rules)) return false;

    let rules: &VecSet<TypeName> = dynamic_field::borrow(account.uid(), field_rules);
    rules.contains(&type_name::get<Rule>())
}

/// Returns the types of the rotation rules guarding the field type `Name`, each of which must
/// approve a change to it.
public fun field_rotation_rules<Name: copy + drop + store>(
    account: &SmartAccount,
): vector<TypeName> {
    if (!dynamic_field::exists_(account.uid(), FieldRotationRulesKey<Name> {})) return vector[];

    let rules: &VecSet<TypeName> = dynamic_field::borrow(
        account.uid(),
        FieldRotationRulesKey<Name> {},
    );
    *rules.keys()
}

// === Admin Functions ===

/// Attaches the rotation rule `Rule`, with its `config`, to the account. From then on, every
/// rotation of the authenticator needs a receipt of `Rule`.
///
/// Emits an `AuthRotationRuleAdded` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `Rule` is already attached.
public fun add_auth_rotation_rule<Rule: drop, Config: store + drop>(
    account: &mut SmartAccount,
    _: Rule,
    config: Config,
    ctx: &TxContext,
) {
    account.ensure_tx_sender_is_smart_account(ctx);

    insert_auth_rotation_rule<Rule, Config>(account.uid_mut(), config);
}

/// Removes the rotation rule `Rule` from the account and returns its configuration.
///
/// Emits an `AuthRotationRuleRemoved` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `Rule` is not attached, or if its configuration is not a `Config`.
/// Aborts if `Rule` still guards field types; remove those with `remove_field_rotation_rule`.
public fun remove_auth_rotation_rule<Rule: drop, Config: store + drop>(
    account: &mut SmartAccount,
    _: Rule,
    ctx: &TxContext,
): Config {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_auth_rotation_rule<Rule>(account), EAuthRotationRuleNotAttached);
    assert!(
        !dynamic_field::exists_(account.uid(), AuthRotationRuleFieldsKey<Rule> {}),
        EAuthRotationRuleGuardsFields,
    );

    let account_id = account.uid_mut();
    let rule = type_name::get<Rule>();
    let rules: &mut VecSet<TypeName> = dynamic_field::borrow_mut(
        account_id,
        AuthRotationRulesKey {},
    );
    rules.remove(&rule);
    let has_rules = !rules.is_empty();
    smart_account::set_has_auth_rotation_rules(account_id, has_rules);

    event::emit(AuthRotationRuleRemoved { account_id: account_id.to_inner(), rule });

    dynamic_field::remove(account_id, AuthRotationRuleKey<Rule> {})
}

/// Borrows the configuration of the rotation rule `Rule` mutably.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `Rule` is not attached, or if its configuration is not a `Config`.
public fun borrow_auth_rotation_rule_config_mut<Rule: drop, Config: store + drop>(
    account: &mut SmartAccount,
    _: Rule,
    ctx: &TxContext,
): &mut Config {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_auth_rotation_rule<Rule>(account), EAuthRotationRuleNotAttached);

    dynamic_field::borrow_mut(account.uid_mut(), AuthRotationRuleKey<Rule> {})
}

/// Starts rotating the account's authenticator to `authenticator`.
///
/// Pass the request to each rotation rule attached to the account, then to
/// `confirm_auth_function_ref_rotation_v1`, in the same transaction.
///
/// Aborts if the transaction sender is not the account.
public fun request_auth_function_ref_rotation_v1(
    account: &SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): AuthRotationRequest {
    account.ensure_tx_sender_is_smart_account(ctx);

    AuthRotationRequest {
        account_id: object::id(account),
        authenticator,
        change_count: change_count(account),
        receipts: vec_set::empty(),
    }
}

/// Rotates the account's authenticator to the one in `request`, and returns the previous one.
///
/// Emits an `account::AuthenticatorFunctionRefV1Rotated` event upon success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `request` was made for another account.
/// Aborts if another rotation or guarded field change was confirmed after `request` was made.
/// Aborts unless the receipts in `request` are exactly the rotation rules attached to the
/// account.
public fun confirm_auth_function_ref_rotation_v1(
    account: &mut SmartAccount,
    request: AuthRotationRequest,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    account.ensure_tx_sender_is_smart_account(ctx);

    let AuthRotationRequest { account_id, authenticator, change_count, receipts } = request;
    assert!(account_id == object::id(account), EAuthRotationRequestForAnotherAccount);
    assert!(change_count == change_count(account), EAccountChangedAfterRequest);

    let rules = auth_rotation_rules(account);
    let receipts = receipts.into_keys();
    assert!(receipts.length() == rules.length(), EAuthRotationRulesNotSatisfied);
    rules.do!(|rule| assert!(receipts.contains(&rule), EAuthRotationRulesNotSatisfied));

    count_change(account.uid_mut());
    account.rotate_auth_function_ref_v1_unchecked(authenticator)
}

/// Makes the rotation rule `Rule`, already attached to the account, guard the field type `Name`.
/// From then on, every change to a field of type `Name` needs a receipt of `Rule`.
///
/// Emits a `FieldRotationRuleAdded` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `Rule` is not attached, or already guards `Name`.
/// Aborts if `Name` is the built-in authenticator's public key field.
public fun add_field_rotation_rule<Rule: drop, Name: copy + drop + store>(
    account: &mut SmartAccount,
    _: Rule,
    ctx: &TxContext,
) {
    account.ensure_tx_sender_is_smart_account(ctx);

    insert_field_rotation_rule<Rule, Name>(account.uid_mut());
}

/// Makes the rotation rule `Rule` stop guarding the field type `Name`.
///
/// Emits a `FieldRotationRuleRemoved` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `Rule` does not guard `Name`.
public fun remove_field_rotation_rule<Rule: drop, Name: copy + drop + store>(
    account: &mut SmartAccount,
    _: Rule,
    ctx: &TxContext,
) {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_field_rotation_rule<Rule, Name>(account), EFieldRotationRuleNotAttached);

    let account_id = account.uid_mut();
    let rule = type_name::get<Rule>();
    let field = type_name::get<Name>();

    let field_rules: &mut VecSet<TypeName> = dynamic_field::borrow_mut(
        account_id,
        FieldRotationRulesKey<Name> {},
    );
    field_rules.remove(&rule);
    if (field_rules.is_empty()) {
        dynamic_field::remove<_, VecSet<TypeName>>(account_id, FieldRotationRulesKey<Name> {});
        smart_account::set_has_field_rotation_rules<Name>(account_id, false);
    };

    let rule_fields: &mut VecSet<TypeName> = dynamic_field::borrow_mut(
        account_id,
        AuthRotationRuleFieldsKey<Rule> {},
    );
    rule_fields.remove(&field);
    if (rule_fields.is_empty()) {
        dynamic_field::remove<_, VecSet<TypeName>>(account_id, AuthRotationRuleFieldsKey<Rule> {});
    };

    event::emit(FieldRotationRuleRemoved { account_id: account_id.to_inner(), rule, field });
}

/// Starts a change to a field of type `Name` with `operation` (`field_add`, `field_remove`,
/// `field_rotate` or `field_borrow_mut`).
///
/// Pass the request to each rotation rule guarding `Name`, then to the guarded field function
/// for `operation`, in the same transaction.
///
/// Aborts if the transaction sender is not the account.
public fun request_field_change_v1<Name: copy + drop + store>(
    account: &SmartAccount,
    operation: u8,
    ctx: &TxContext,
): FieldChangeRequest<Name> {
    account.ensure_tx_sender_is_smart_account(ctx);

    FieldChangeRequest {
        account_id: object::id(account),
        operation,
        change_count: change_count(account),
        receipts: vec_set::empty(),
    }
}

/// Adds a field guarded by rotation rules, with an approved `field_add` request.
///
/// Aborts if `request` was made for another account or operation, if another rotation or guarded
/// field change was confirmed after it was made, or unless its receipts are exactly the rotation
/// rules guarding the field type.
/// Aborts if a field with the same `name` already exists.
public fun add_field<Name: copy + drop + store, Value: store>(
    account: &mut SmartAccount,
    request: FieldChangeRequest<Name>,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    confirm_field_change(account, request, FIELD_ADD, ctx);

    dynamic_field::add(account.uid_mut(), name, value);
}

/// Removes a field guarded by rotation rules, with an approved `field_remove` request.
///
/// Aborts if `request` was made for another account or operation, if another rotation or guarded
/// field change was confirmed after it was made, or unless its receipts are exactly the rotation
/// rules guarding the field type.
/// Aborts if no field with the specified `name` exists.
public fun remove_field<Name: copy + drop + store, Value: store>(
    account: &mut SmartAccount,
    request: FieldChangeRequest<Name>,
    name: Name,
    ctx: &TxContext,
): Value {
    confirm_field_change(account, request, FIELD_REMOVE, ctx);

    dynamic_field::remove(account.uid_mut(), name)
}

/// Replaces a field guarded by rotation rules with `value` and returns the previous one, with an
/// approved `field_rotate` request.
///
/// Aborts if `request` was made for another account or operation, if another rotation or guarded
/// field change was confirmed after it was made, or unless its receipts are exactly the rotation
/// rules guarding the field type.
/// Aborts if no field with the specified `name` exists.
public fun rotate_field<Name: copy + drop + store, Value: store>(
    account: &mut SmartAccount,
    request: FieldChangeRequest<Name>,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    confirm_field_change(account, request, FIELD_ROTATE, ctx);

    let account_id = account.uid_mut();
    let previous_value = dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
    previous_value
}

/// Mutably borrows a field guarded by rotation rules, with an approved `field_borrow_mut`
/// request.
///
/// Aborts if `request` was made for another account or operation, if another rotation or guarded
/// field change was confirmed after it was made, or unless its receipts are exactly the rotation
/// rules guarding the field type.
/// Aborts if no field with the specified `name` exists.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    account: &mut SmartAccount,
    request: FieldChangeRequest<Name>,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    confirm_field_change(account, request, FIELD_BORROW_MUT, ctx);

    dynamic_field::borrow_mut(account.uid_mut(), name)
}

// === Private Functions ===

/// Consumes a field change request after checking it. The caller makes the change right after.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `request` was made for another account or another operation than `operation`.
/// Aborts if another rotation or guarded field change was confirmed after `request` was made.
/// Aborts unless the receipts in `request` are exactly the rotation rules guarding `Name`.
fun confirm_field_change<Name: copy + drop + store>(
    account: &mut SmartAccount,
    request: FieldChangeRequest<Name>,
    operation: u8,
    ctx: &TxContext,
) {
    account.ensure_tx_sender_is_smart_account(ctx);

    let FieldChangeRequest { account_id, operation: requested, change_count, receipts } = request;
    assert!(account_id == object::id(account), EAuthRotationRequestForAnotherAccount);
    assert!(requested == operation, EFieldChangeOperationMismatch);
    assert!(change_count == change_count(account), EAccountChangedAfterRequest);

    let rules = field_rotation_rules<Name>(account);
    let receipts = receipts.into_keys();
    assert!(receipts.length() == rules.length(), EAuthRotationRulesNotSatisfied);
    rules.do!(|rule| assert!(receipts.contains(&rule), EAuthRotationRulesNotSatisfied));

    count_change(account.uid_mut());
}

fun change_count(account: &SmartAccount): u64 {
    let account_id = account.uid();
    if (dynamic_field::exists_(account_id, ChangeCountKey {})) {
        *dynamic_field::borrow(account_id, ChangeCountKey {})
    } else {
        0
    }
}

fun count_change(account_id: &mut UID) {
    if (dynamic_field::exists_(account_id, ChangeCountKey {})) {
        let count: &mut u64 = dynamic_field::borrow_mut(account_id, ChangeCountKey {});
        *count = *count + 1;
    } else {
        dynamic_field::add(account_id, ChangeCountKey {}, 1u64);
    }
}

fun insert_auth_rotation_rule<Rule: drop, Config: store + drop>(
    account_id: &mut UID,
    config: Config,
) {
    assert!(
        !dynamic_field::exists_(account_id, AuthRotationRuleKey<Rule> {}),
        EAuthRotationRuleAlreadyAttached,
    );
    if (!dynamic_field::exists_(account_id, AuthRotationRulesKey {})) {
        dynamic_field::add(account_id, AuthRotationRulesKey {}, vec_set::empty<TypeName>());
    };

    let rule = type_name::get<Rule>();
    let rules: &mut VecSet<TypeName> = dynamic_field::borrow_mut(
        account_id,
        AuthRotationRulesKey {},
    );
    rules.insert(rule);
    dynamic_field::add(account_id, AuthRotationRuleKey<Rule> {}, config);
    smart_account::set_has_auth_rotation_rules(account_id, true);

    event::emit(AuthRotationRuleAdded { account_id: account_id.to_inner(), rule });
}

fun insert_field_rotation_rule<Rule: drop, Name: copy + drop + store>(account_id: &mut UID) {
    assert!(type_name::get<Name>() != type_name::get<PublicKeyFieldName>(), EFieldTypeNotGuardable);
    assert!(
        dynamic_field::exists_(account_id, AuthRotationRuleKey<Rule> {}),
        EAuthRotationRuleNotAttached,
    );

    let rule = type_name::get<Rule>();
    let field = type_name::get<Name>();

    if (!dynamic_field::exists_(account_id, FieldRotationRulesKey<Name> {})) {
        dynamic_field::add(account_id, FieldRotationRulesKey<Name> {}, vec_set::empty<TypeName>());
    };
    let field_rules: &mut VecSet<TypeName> = dynamic_field::borrow_mut(
        account_id,
        FieldRotationRulesKey<Name> {},
    );
    assert!(!field_rules.contains(&rule), EFieldRotationRuleAlreadyAttached);
    field_rules.insert(rule);

    if (!dynamic_field::exists_(account_id, AuthRotationRuleFieldsKey<Rule> {})) {
        dynamic_field::add(
            account_id,
            AuthRotationRuleFieldsKey<Rule> {},
            vec_set::empty<TypeName>(),
        );
    };
    let rule_fields: &mut VecSet<TypeName> = dynamic_field::borrow_mut(
        account_id,
        AuthRotationRuleFieldsKey<Rule> {},
    );
    rule_fields.insert(field);

    smart_account::set_has_field_rotation_rules<Name>(account_id, true);

    event::emit(FieldRotationRuleAdded { account_id: account_id.to_inner(), rule, field });
}
