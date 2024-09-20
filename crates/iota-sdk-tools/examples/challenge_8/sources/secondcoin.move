module ctf::ctfb {
    use iota::coin::{Self, Coin, TreasuryCap};

    public struct CTFB has drop {}

    public struct MintB<phantom CTFB> has key, store {
        id: UID,
        cap: TreasuryCap<CTFB>
    }

    fun init(witness: CTFB, ctx: &mut TxContext) {
         // Get a treasury cap for the coin and give it to the transaction sender
        let (treasury_cap, metadata) = coin::create_currency<CTFB>(
            witness,
            1,
            b"CTFB",
            b"CTF B Coin",
            b"Token for the CTF",
            option::none(),
            ctx
        );
        let mint = MintB<CTFB> {
            id: object::new(ctx),
            cap: treasury_cap
        };
        transfer::share_object(mint);
        transfer::public_freeze_object(metadata);
    }

    public(package) fun mint_for_vault<CTFB>(
        mut mint: MintB<CTFB>,
        ctx: &mut TxContext
    ): Coin<CTFB> {
        let coinb = coin::mint<CTFB>(&mut mint.cap, 100, ctx);
        coin::mint_and_transfer(
            &mut mint.cap,
            10,
            tx_context::sender(ctx),
            ctx
        );
        let MintB<CTFB> {id: ida, cap: capa} = mint;
        object::delete(ida);
        transfer::public_freeze_object(capa);
        coinb
    }

}
