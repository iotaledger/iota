// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A public key whose holder approved attaching it to one account.
///
/// The key holder signs the message of `iota::approval_message` as an IOTA personal message. The
/// lines this module writes name the key and where it may go. For an existing account:
///
/// ```
/// public key: <hex of the scheme flag and the raw key bytes>
/// account: 0x<hex of the account address>
/// ```
///
/// For a new `SmartAccount`, whose address is not known when the key holder signs:
///
/// ```
/// public key: <hex of the scheme flag and the raw key bytes>
/// account: new SmartAccount with built-in authenticator
/// sender: 0x<hex of the transaction sender>
/// ```
///
/// An `AuthenticatedPublicKey` has no abilities, so it is used in the transaction that made it.
/// Only this package can take the key out of it, and only for the destination the key holder
/// approved.
module iota::authenticated_public_key;

use iota::approval_message;
use iota::hex;
use iota::protocol_config;
use iota::public_key::PublicKey;
use iota::verified_nonce::VerifiedNonce;

// === Errors ===

#[error(code = 0)]
const EBuiltinAuthenticatorsNotEnabled: vector<u8> = b"Built-in Move authenticators not enabled.";
#[error(code = 1)]
const EApprovalForAnotherAccount: vector<u8> =
    b"The public key was approved for another account.";
#[error(code = 2)]
const EApprovalForNewSmartAccount: vector<u8> =
    b"The public key was approved for a new SmartAccount, not for an existing account.";
#[error(code = 3)]
const EApprovalForExistingAccount: vector<u8> =
    b"The public key was approved for an existing account, not for a new SmartAccount.";

// === Structs ===

/// A public key whose holder approved attaching it to `account`, or to a new `SmartAccount` when
/// `account` is none.
public struct AuthenticatedPublicKey {
    public_key: PublicKey,
    /// The account the key holder approved, or none for a new `SmartAccount` created by the
    /// transaction sender.
    account: Option<address>,
}

// === Public Functions ===

/// Checks the key holder's approval to attach `public_key` to a new `SmartAccount` with the
/// built-in authenticator, created by the transaction sender, and returns the key authenticated
/// for it.
///
/// `signature` is by `public_key`, in the wire format of its scheme, over the approval message
/// for `AuthenticatedPublicKey` with the lines `account: new SmartAccount with built-in
/// authenticator` and `sender:` naming the transaction sender. The approval is valid up to and
/// including the epoch `valid_until_epoch`.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if the signature is not valid.
public fun verify_for_new_smart_account(
    nonce: VerifiedNonce<AuthenticatedPublicKey>,
    public_key: PublicKey,
    signature: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
): AuthenticatedPublicKey {
    check_builtin_authenticators_enabled();

    let mut lines = public_key_line(&public_key);
    lines.append(b"\naccount: new SmartAccount with built-in authenticator\nsender: 0x");
    lines.append(hex::encode(ctx.sender().to_bytes()));

    approval_message::verify_signature(
        nonce,
        &public_key,
        lines,
        signature,
        valid_until_epoch,
        ctx,
    );

    AuthenticatedPublicKey { public_key, account: option::none() }
}

/// Checks the key holder's approval to attach `public_key` to the existing account `account`,
/// and returns the key authenticated for that account.
///
/// `signature` is by `public_key`, in the wire format of its scheme, over the approval message
/// for `AuthenticatedPublicKey` with an `account:` line naming `account`. The approval is valid
/// up to and including the epoch `valid_until_epoch`.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if the signature is not valid.
public fun verify_for_existing_account(
    nonce: VerifiedNonce<AuthenticatedPublicKey>,
    public_key: PublicKey,
    account: address,
    signature: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
): AuthenticatedPublicKey {
    check_builtin_authenticators_enabled();

    let mut lines = public_key_line(&public_key);
    lines.append(b"\naccount: 0x");
    lines.append(hex::encode(account.to_bytes()));

    approval_message::verify_signature(
        nonce,
        &public_key,
        lines,
        signature,
        valid_until_epoch,
        ctx,
    );

    AuthenticatedPublicKey { public_key, account: option::some(account) }
}

// === View Functions ===

/// Returns the authenticated public key.
public fun public_key(self: &AuthenticatedPublicKey): &PublicKey {
    &self.public_key
}

/// Returns the account the key holder approved, or none for a new `SmartAccount`.
public fun account(self: &AuthenticatedPublicKey): &Option<address> {
    &self.account
}

// === Package Functions ===

/// Destroys the approval and returns its key.
///
/// Aborts if the approval is not for the existing account `account`.
public(package) fun into_public_key_for_existing_account(
    self: AuthenticatedPublicKey,
    account: address,
): PublicKey {
    let AuthenticatedPublicKey { public_key, account: approved_account } = self;
    assert!(approved_account.is_some(), EApprovalForNewSmartAccount);
    assert!(approved_account.destroy_some() == account, EApprovalForAnotherAccount);
    public_key
}

/// Destroys the approval and returns its key.
///
/// Aborts if the approval is not for a new `SmartAccount`.
public(package) fun into_public_key_for_new_smart_account(self: AuthenticatedPublicKey): PublicKey {
    let AuthenticatedPublicKey { public_key, account } = self;
    assert!(account.is_none(), EApprovalForExistingAccount);
    public_key
}

// === Private Functions ===

/// Returns the `public key:` line for `public_key`.
fun public_key_line(public_key: &PublicKey): vector<u8> {
    let mut prefixed_key = vector[public_key.scheme().flag()];
    prefixed_key.append(*public_key.raw_bytes());

    let mut line = b"public key: ";
    line.append(hex::encode(prefixed_key));
    line
}

/// Aborts if the built-in Move authenticators feature is disabled in the protocol config.
fun check_builtin_authenticators_enabled() {
    assert!(
        protocol_config::is_feature_enabled(b"enable_builtin_move_authenticators"),
        EBuiltinAuthenticatorsNotEnabled,
    );
}
