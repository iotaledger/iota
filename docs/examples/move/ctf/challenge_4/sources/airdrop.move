module ctf::airdrop {
    use iota::table::{Self, Table};
    use iota::coin::{Self, Coin};
    use iota::balance::{Self, Balance};
    use ctf::counter::{Self, Counter};


    public struct AirdroppedTo has store {
        user: address,
    }

    public struct Vault has key {
        id: UID,
        balance: Balance<AIRDROP>,
        userlist: Table<address, AirdroppedTo>,
    }

    public struct AIRDROP has drop {}

    public struct Flag has key, store {
        id: UID,
        user: address
    }

    fun init (witness: AIRDROP, ctx: &mut TxContext) {
        counter::create_counter(ctx);

        let initializer = tx_context::sender(ctx);
        let (mut coincap, coindata) = coin::create_currency(witness, 18, b"HORSE", b"Horse Tokens", b"To The Moon", option::none(), ctx);
        let coins_minted = coin::mint<AIRDROP>(&mut coincap, 10000, ctx);
        transfer::public_freeze_object(coindata);
        transfer::public_transfer(coincap, initializer);
        transfer::share_object(
            Vault {
            id: object::new(ctx),
            balance: coin::into_balance<AIRDROP>(coins_minted),
            userlist: table::new<address, AirdroppedTo>(ctx),
            }
        );
    }

    public entry fun airdrop(vault: &mut Vault, ctx: &mut TxContext) {
        let sender = tx_context::sender(ctx);
        assert!(!table::contains<address, AirdroppedTo>(&vault.userlist, sender) ,1);
        let mut balance_drop = balance::split(&mut vault.balance, 1);
        let coin_drop = coin::take(&mut balance_drop, 1, ctx);
        transfer::public_transfer(coin_drop, sender);
        balance::destroy_zero(balance_drop);
        table::add<address, AirdroppedTo>(&mut vault.userlist, sender, AirdroppedTo {
            user: sender,
        });
    }

    public entry fun get_flag(user_counter: &mut Counter, coin_drop: &mut Coin<AIRDROP>, ctx: &mut TxContext) {

        counter::increment(user_counter);
        counter::is_within_limit(user_counter);

        let expected_value = coin::value(coin_drop);
        assert!(expected_value == 2, 2);

        transfer::public_transfer(Flag {
            id: object::new(ctx),
            user: tx_context::sender(ctx)
        }, tx_context::sender(ctx));
    }
}
