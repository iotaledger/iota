#[test_only]
module clt_tutorial::voucher_tests {
    use clt_tutorial::voucher::{
        buy_led_bulb,
        gift_voucher,
        return_voucher,
        register_shop,
        test_init,
        VOUCHER
    };
    
    use iota::coin::TreasuryCap;
    use iota::test_scenario;
    use iota::token::{Token, TokenPolicy, TokenPolicyCap};

    // Test default voucher workflow
    #[test]
    fun test_voucher_workflow() {
        let municipality = @0xBABE;
        let household = @0xCAFE;
        let shop = @0xFACE;

        let mut scenario_val = test_scenario::begin(municipality);
        let scenario = &mut scenario_val;
        {
            test_init(scenario.ctx());
        };

        // Gift voucher to household
        scenario.next_tx(municipality);
        {
            let mut treasury_cap = scenario.take_from_sender<TreasuryCap<VOUCHER>>();
            gift_voucher(&mut treasury_cap, 1, household, scenario.ctx());
            scenario.return_to_sender(treasury_cap);
        };

        // Register shop
        scenario.next_tx(municipality);
        {
            let addresses = vector[municipality, shop];
            let mut policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let treasury_cap = scenario.take_from_sender<TokenPolicyCap<VOUCHER>>();
            register_shop(&mut policy, &treasury_cap, addresses, scenario.ctx());

            test_scenario::return_shared(policy);
            scenario.return_to_sender(treasury_cap);
        };

        // Buy LED bulb as household
        scenario.next_epoch(household);
        {
            let voucher = scenario.take_from_sender<Token<VOUCHER>>();
            let policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let (led_bulb, action_request) = buy_led_bulb(voucher, &policy, shop, scenario.ctx());
            
            std::debug::print(&action_request);

            policy.confirm_request(action_request, scenario.ctx());
            transfer::public_transfer(led_bulb, tx_context::sender(scenario.ctx()));

            test_scenario::return_shared(policy);
        };

        // Return voucher as shop
        scenario.next_epoch(shop);
        {
            let voucher = scenario.take_from_sender<Token<VOUCHER>>();
            let mut policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let action_request = return_voucher(voucher, &policy, scenario.ctx());
            
            std::debug::print(&action_request);

            policy.confirm_request_mut(action_request, scenario.ctx());

            test_scenario::return_shared(policy);
        };

        scenario_val.end();
    }

    // Test voucher workflow with failing scenarios where shop tries to use voucher
    #[test, expected_failure(abort_code = clt_tutorial::allowlist_rule::ESenderIsAShop)]
    fun test_shop_using_voucher_fail() {
        let municipality = @0xBABE;
        let shop = @0xFACE;
        let shop2 = @0xCAFE;

        let mut scenario_val = test_scenario::begin(municipality);
        let scenario = &mut scenario_val;
        {
            test_init(scenario.ctx());
        };

        // Register shops
        scenario.next_tx(municipality);
        {
            let addresses = vector[shop, shop2];
            let mut policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let treasury_cap = scenario.take_from_sender<TokenPolicyCap<VOUCHER>>();
            register_shop(&mut policy, &treasury_cap, addresses, scenario.ctx());

            test_scenario::return_shared(policy);
            scenario.return_to_sender(treasury_cap);
        };

        // Gift voucher to shop
        scenario.next_tx(municipality);
        {
            let mut treasury_cap = scenario.take_from_sender<TreasuryCap<VOUCHER>>();
            gift_voucher(&mut treasury_cap, 1, shop, scenario.ctx());
            scenario.return_to_sender(treasury_cap);
        };

        // Buy LED bulb as shop
        scenario.next_epoch(shop);
        {
            let voucher = scenario.take_from_sender<Token<VOUCHER>>();
            let policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let (led_bulb, action_request) = buy_led_bulb(voucher, &policy, shop2, scenario.ctx());

            policy.confirm_request(action_request, scenario.ctx());
            transfer::public_transfer(led_bulb, tx_context::sender(scenario.ctx()));

            test_scenario::return_shared(policy);
        };

        scenario_val.end();
    }

    // Test voucher workflow with failing scenarios where household tries to return voucher
    #[test, expected_failure(abort_code = clt_tutorial::allowlist_rule::ESenderNotAShop)]
    fun test_household_returning_voucher_fail() {
        let municipality = @0xBABE;
        let household = @0xCAFE;
        let shop = @0xFACE;

        let mut scenario_val = test_scenario::begin(municipality);
        let scenario = &mut scenario_val;
        {
            test_init(scenario.ctx());
        };

        // Register shop
        scenario.next_tx(municipality);
        {
            let addresses = vector[shop];
            let mut policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let treasury_cap = scenario.take_from_sender<TokenPolicyCap<VOUCHER>>();
            register_shop(&mut policy, &treasury_cap, addresses, scenario.ctx());

            test_scenario::return_shared(policy);
            scenario.return_to_sender(treasury_cap);
        };

        scenario.next_tx(municipality);
        {
            let mut treasury_cap = scenario.take_from_sender<TreasuryCap<VOUCHER>>();
            gift_voucher(&mut treasury_cap, 1, household, scenario.ctx());
            scenario.return_to_sender(treasury_cap);
        };

        scenario.next_epoch(household);
        {
            let voucher = scenario.take_from_sender<Token<VOUCHER>>();
            let mut policy = scenario.take_shared<TokenPolicy<VOUCHER>>();
            let action_request = return_voucher(voucher, &policy, scenario.ctx());
            
            std::debug::print(&action_request);

            policy.confirm_request_mut(action_request, scenario.ctx());

            test_scenario::return_shared(policy);
        };

        scenario_val.end();
    }
}
