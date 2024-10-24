// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::coin_balance_tests {
    use iota::test_scenario;
    use iota::pay;
    use iota::coin;
    use iota::balance;
    use iota::iota::IOTA;

    #[test]
    fun type_morphing() {
        let mut scenario = test_scenario::begin(@0x1);

        let balance = balance::zero<IOTA>();
        let coin = balance.into_coin(scenario.ctx());
        let balance = coin.into_balance();

        balance.destroy_zero();

        let mut coin = coin::mint_for_testing<IOTA>(100, scenario.ctx());
        let balance_mut = coin::balance_mut(&mut coin);
        let sub_balance = balance_mut.split(50);

        assert!(sub_balance.value() == 50);
        assert!(coin.value() == 50);

        let mut balance = coin.into_balance();
        balance.join(sub_balance);

        assert!(balance.value() == 100);

        let coin = balance.into_coin(scenario.ctx());
        pay::keep(coin, scenario.ctx());
        scenario.end();
    }
}
