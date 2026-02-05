// tests modules cannot use private account internal functions outside of the defining module
// even if it has store

module a::m {
    use iota::account;
    use iota::authenticator_function::AuthenticatorFunctionRefV1;

    public fun t1<A: key + store>(account: A, authenticator: AuthenticatorFunctionRefV1<A>) {
        account::create_account_v1(account, authenticator);
    }

    public fun t2<A: key + store>(account: A, authenticator: AuthenticatorFunctionRefV1<A>) {
        account::create_immutable_account_v1(account, authenticator);
    }

    public fun t3<A: key + store>(
        account: &mut A,
        authenticator: AuthenticatorFunctionRefV1<A>,
    ): AuthenticatorFunctionRefV1<A> {
        account::rotate_auth_function_ref_v1(account, authenticator)
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}

module iota::authenticator_function {
    use iota::object::UID;

    struct AuthenticatorFunctionRefV1<phantom Account: key> {
        id: UID,
    }
}

module iota::account {
    use iota::authenticator_function::AuthenticatorFunctionRefV1;

    public fun create_account_v1<Account: key>(_: Account, _: AuthenticatorFunctionRefV1<Account>) {
        abort 0
    }

    public fun create_immutable_account_v1<Account: key>(
        _: Account,
        _: AuthenticatorFunctionRefV1<Account>,
    ) {
        abort 0
    }

    public fun rotate_auth_function_ref_v1<Account: key>(
        _: &mut Account,
        _: AuthenticatorFunctionRefV1<Account>,
    ): AuthenticatorFunctionRefV1<Account> {
        abort 0
    }
}
