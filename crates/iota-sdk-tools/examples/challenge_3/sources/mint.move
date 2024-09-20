module ctf::mintcoin {
    use iota::coin::{Self, Coin, TreasuryCap};
    use iota::event;
    use ctf::counter::{Self, Counter};

    public struct MINTCOIN has drop {}

    public struct Flag has copy, drop {
        user: address,
        flag: bool,
    }

    #[allow(lint(share_owned))]
    fun init(
        witness: MINTCOIN,
        ctx: &mut TxContext
    ) {
        counter::create_counter(ctx);
        let (coincap, coindata) = coin::create_currency(
            witness,
            0,
            b"MintCoin",
            b"Mint Coin",
            b"A coin that anyone can mint, mind blowing!",
            option::none(),
            ctx
        );
        transfer::public_freeze_object(coindata);
        transfer::public_share_object(coincap);
    }

    public entry fun mint_coin(
        cap: &mut TreasuryCap<MINTCOIN>,
        ctx: &mut TxContext
    ) {
        coin::mint_and_transfer<MINTCOIN>(cap, 2, tx_context::sender(ctx), ctx);
    }

    public entry fun burn_coin(
        cap: &mut TreasuryCap<MINTCOIN>,
        coins: Coin<MINTCOIN>
    ) {
        coin::burn(cap, coins);
    }

    public entry fun get_flag(
        user_counter: &mut Counter,
        coins: &mut Coin<MINTCOIN>,
        ctx: &mut TxContext
    ) {
        counter::increment(user_counter);
        counter::is_within_limit(user_counter);

        let limit = coin::value(coins);
        assert!(limit == 5, 1);
        event::emit(
            Flag {
                user: tx_context::sender(ctx),
                flag: true,
            }
        );
    }
}
