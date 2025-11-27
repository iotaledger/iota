module account::account;

use iota::account::AuthenticatorInfoV1CompatibilityProof;
use iota::package_metadata::PackageMetadataV1;

public struct Account has key, store {
    id: UID,
}

public struct ACCOUNT has drop {}

fun init(_otw: ACCOUNT, ctx: &mut TxContext) {
    // Shares the account object, anyone can claim it by calling the link_auth function
    transfer::public_share_object(Account {
        id: object::new(ctx),
    });
}

/// Wrapper because of &mut UID
public fun attach_auth_info_v1<AccountType: key>(account: &mut Account, authenticator_proof: AuthenticatorInfoV1CompatibilityProof<AccountType>,) {
    iota::account::attach_auth_info_v1<AccountType>(&mut account.id, authenticator_proof);
}

public fun link_auth(account: &mut Account, package: &PackageMetadataV1, module_name: std::ascii::String, function_name: std::ascii::String) {
    let authenticator = iota::account::create_auth_info_v1<Account>(package, module_name, function_name);
    let authenticator_proof = iota::account::check_auth_info_v1_compatibility<Account>(account, authenticator);
    iota::account::attach_auth_info_v1<Account>(&mut account.id, authenticator_proof);
}

/// An unsecure example authenticator function that checks if the provided message is "hello".
#[authenticator]
public fun authenticate(
    _account: &Account,
    msg: std::ascii::String,
    _auth_ctx: &iota::auth_context::AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == std::ascii::string(b"hello"), 0);
}
