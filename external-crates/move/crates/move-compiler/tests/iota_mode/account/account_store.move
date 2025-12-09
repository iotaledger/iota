// can use private account internal functions inside of the defining module if it has store

module a::m {
    use iota::account::{Self, AuthenticatorInfoV1};

    struct A has key, store {
        id: iota::object::UID,
    }

    public fun t1(account: A, authenticator: AuthenticatorInfoV1<A>) {
        account::create_account_v1(account, authenticator);
    }

    public fun t2(account: A, authenticator: AuthenticatorInfoV1<A>) {
        account::create_immutable_account_v1(account, authenticator);
    }

    public fun t3(account: &mut A, authenticator: AuthenticatorInfoV1<A>): AuthenticatorInfoV1<A> {
        account::rotate_auth_info_v1(account, authenticator)
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}

module iota::account {
    use iota::object::UID;

    struct AuthenticatorInfoV1<phantom Account: key> {
        id: UID,
    }

    public fun create_account_v1<Account: key>(_: Account, _: AuthenticatorInfoV1<Account>) {
        abort 0
    }

    public fun create_immutable_account_v1<Account: key>(
        _: Account,
        _: AuthenticatorInfoV1<Account>,
    ) {
        abort 0
    }

    public fun rotate_auth_info_v1<Account: key>(
        _: &mut Account,
        _: AuthenticatorInfoV1<Account>,
    ): AuthenticatorInfoV1<Account> {
        abort 0
    }
}
