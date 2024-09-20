module ctf::vault{
    use iota::coin::{Self, Coin};
    use iota::balance::{Self, Balance};
    use iota::event;
    use ctf::ctfa::{Self, MintA};
    use ctf::ctfb::{Self, MintB};

    public struct Vault<phantom A, phantom B> has key {
        id: UID,
        coin_a: Balance<A>,
        coin_b: Balance<B>,
        flashed: bool
    }

    public struct Flag has copy, drop {
        flag: bool,
        user: address
    }

    public struct Receipt {
        id: ID,
        a_to_b: bool,
        repay_amount: u64
    }

    public entry fun initialize<A,B>(capa: MintA<A>, capb: MintB<B>,ctx: &mut TxContext) {
        let vault = Vault<A, B> {
            id: object::new(ctx),
            coin_a: coin::into_balance(ctfa::mint_for_vault(capa, ctx)),
            coin_b: coin::into_balance(ctfb::mint_for_vault(capb, ctx)),
            flashed: false
        };
        transfer::share_object(vault);
    }

    public fun flash<A,B>(vault: &mut Vault<A,B>, amount: u64, a_to_b: bool, ctx: &mut TxContext): (Coin<A>, Coin<B>, Receipt) {
        assert!(!vault.flashed, 1);
        let (coin_a, coin_b) = if (a_to_b) {
        (coin::zero<A>(ctx), coin::from_balance(balance::split(&mut vault.coin_b, amount ), ctx))
        }
        else {
        (coin::from_balance(balance::split(&mut vault.coin_a, amount ), ctx), coin::zero<B>(ctx))
        };

        let receipt = Receipt {
            id: object::id(vault),
            a_to_b,
            repay_amount: amount
        };
        vault.flashed = true;

        (coin_a, coin_b, receipt)

    }

    public fun repay_flash<A,B>(vault: &mut Vault<A,B>, coina: Coin<A>, coinb: Coin<B>, receipt: Receipt) {
        let Receipt {
            id: _,
            a_to_b: a2b,
            repay_amount: amount
        } = receipt;
        if (a2b) {
            assert!(coin::value(&coinb) >= amount, 0);
        } else {
            assert!(coin::value(&coina) >= amount, 1);
        };
        balance::join(&mut vault.coin_a, coin::into_balance(coina));
        balance::join(&mut vault.coin_b, coin::into_balance(coinb));
        vault.flashed = false;
    }

    public fun swap_a_to_b<A,B>(vault: &mut Vault<A,B>, coina:Coin<A>, ctx: &mut TxContext): Coin<B> {
            let amount_out_B = coin::value(&coina) * balance::value(&vault.coin_b) / balance::value(&vault.coin_a);
            coin::put<A>(&mut vault.coin_a, coina);
            coin::take(&mut vault.coin_b, amount_out_B, ctx)
    }

    public fun swap_b_to_a<A,B>(vault: &mut Vault<A,B>, coinb:Coin<B>, ctx: &mut TxContext): Coin<A> {
            let amount_out_A = coin::value(&coinb) * balance::value(&vault.coin_a) / balance::value(&vault.coin_b);
            coin::put<B>(&mut vault.coin_b, coinb);
            coin::take(&mut vault.coin_a, amount_out_A, ctx)
    }

    public fun get_flag<A,B>(vault: &Vault<A,B>, ctx: &TxContext) {
        assert!(
            balance::value(&vault.coin_a) == 0 && balance::value(&vault.coin_b) == 0, 123
        );
        event::emit(
            Flag {
                flag: true,
                user: tx_context::sender(ctx)
            }
        );
    }
}
