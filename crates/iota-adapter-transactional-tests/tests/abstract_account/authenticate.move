// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account {
    use iota::account::{Self, AuthenticatorInfoV1};
    use iota::dynamic_field;
    use iota::auth_context::AuthContext;

    #[error(code = 0)]
    const ETransactionSenderIsNotTheAccount: vector<u8> =
        b"Transaction must be signed by the account.";

    public struct AbstractAccountBuilder {
        account: AbstractAccount,
    }

    public struct AbstractAccount has key {
        id: UID,
    }

    public fun builder(
        authenticator: AuthenticatorInfoV1,
        ctx: &mut TxContext,
    ): AbstractAccountBuilder {
        let mut builder = AbstractAccountBuilder {
            account: AbstractAccount { id: object::new(ctx) },
        };

        account::attach_auth_info_v1(&mut builder.account.id, authenticator);
        builder
    }

    public fun add_dynamic_field<Name: copy + drop + store, Value: store>(
        mut self: AbstractAccountBuilder,
        name: Name,
        value: Value,
    ): AbstractAccountBuilder {
        dynamic_field::add(&mut self.account.id, name, value);
        self
    }

    public fun finish(self: AbstractAccountBuilder): AbstractAccount {
        let AbstractAccountBuilder { account } = self;
        account
    }

    public fun share(self: AbstractAccount) {
        iota::transfer::share_object(self);
    }

    // === Admin Functions ===

    /// Check that the sender of this transaction is the account.
    public fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
        assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
    }

        /// A dynamic field key for the account owner public key.
    public struct OwnerPublicKey has copy, drop, store {}

    public fun create(
        public_key: vector<u8>,
        authenticator: AuthenticatorInfoV1,
        ctx: &mut TxContext,
    ) {
        let account = builder(authenticator, ctx)
            .add_dynamic_field(OwnerPublicKey {}, public_key)
            .finish();
        account.share();
    }

    public fun authenticate(account: &AbstractAccount, _auth_ctx: &AuthContext, ctx: &TxContext) {
        ensure_tx_sender_is_account(account, ctx);
    }
}

//# programmable --sender A --inputs b"10" @test "abstract_account" "authenticate"
//> 0: iota::account::create_auth_info_v1(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));

//# view-object 2,2

//# set-address a_account object(2,2)

//# programmable --sender A --inputs 7000000000 @a_account
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 5,0

//# abstract --gas-payment 5,0 --auth-inputs object(2,2) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 7,0