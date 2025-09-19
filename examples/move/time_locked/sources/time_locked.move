module time_locked::time_locked;

use account_template::account_template::{Self, IOTAccount};
use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::clock::Clock;
use iota::dynamic_field;
use iota::ed25519;

#[error(code = 1)]
const EAccountStillLocked: vector<u8> = b"The account is still locked.";

#[error(code = 10)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";

// Can this cause problems when upgrading to a different version and also trying to update it later?
public struct ReservedDfNames has copy, drop, store {}

public struct UnlockTime has copy, drop, store {}

public struct OwnerPublicKey has copy, drop, store {}

public fun create(
    unlock_time: u64,
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
) {
    // Create a UID for an account object.
    let mut id = object::new(ctx);

    let reserved_df_names = vector<std::type_name::TypeName>[
        std::type_name::get<ReservedDfNames>(),
        std::type_name::get<UnlockTime>(),
    ];

    // Add the authenticator info as a dynamic field.
    dynamic_field::add(&mut id, ReservedDfNames {}, reserved_df_names);
    // Add the authenticator info as a dynamic field.
    dynamic_field::add(&mut id, account::authenticator_df_name(), authenticator);

    // Add the unlock time as a dynamic field.
    dynamic_field::add(&mut id, UnlockTime {}, unlock_time);
    // Add the account owner public key as a dynamic field.
    dynamic_field::add(&mut id, OwnerPublicKey {}, public_key);

    account_template::shared_create(id);
}

public fun authenticate(
    self: &IOTAccount,
    clock: &Clock,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    account_template::ensure_tx_sender_is_account(self, ctx);

    authenticate_time(self, clock, auth_ctx, ctx);
    authenticate_ed25519(self, signature, auth_ctx, ctx);
}

fun authenticate_time(self: &IOTAccount, clock: &Clock, _auth_ctx: &AuthContext, _ctx: &TxContext) {
    let unlock_time: &u64 = self.borrow_field(UnlockTime {});

    let now = clock.timestamp_ms();
    // Enforce the time lock
    assert!(now >= *unlock_time, EAccountStillLocked);
}

/// Ed25519 signature authenticator.
/// This could be private as nobody should call it directly at all.
fun authenticate_ed25519(
    self: &IOTAccount,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    let public_key: &vector<u8> = self.borrow_field(OwnerPublicKey {});
    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&signature, public_key, ctx.digest()),
        EEd25519VerificationFailed,
    );
}

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun create_owner_public_key_for_testing(): OwnerPublicKey {
    OwnerPublicKey {}
}

#[test_only]
public fun create_unlock_time_key_for_testing(): UnlockTime {
    UnlockTime {}
}
