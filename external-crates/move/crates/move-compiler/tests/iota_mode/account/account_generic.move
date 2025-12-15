// tests modules cannot use private account internal functions outside of the defining module

module a::m {
    use iota::account::{Self, AuthenticatorInfoV1};

    public fun t1<A: key>(account: A, authenticator: AuthenticatorInfoV1<A>) {
        account::create_account_v1(account, authenticator);
    }

    public fun t2<A: key>(account: A, authenticator: AuthenticatorInfoV1<A>) {
        account::create_immutable_account_v1(account, authenticator);
    }

    public fun t3<A: key>(
        account: &mut A,
        authenticator: AuthenticatorInfoV1<A>,
    ): AuthenticatorInfoV1<A> {
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
