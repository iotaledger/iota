#[test_only]
module account_auth_example::account_auth_example_tests;

use account_auth_example::main_m::will_fail_at_execution_time_because_otw;
use iota::test_scenario;

#[test, expected_failure]
fun otw_fail() {
    let mut scenario = test_scenario::begin(@0x1);
    will_fail_at_execution_time_because_otw(scenario.ctx());
    test_scenario::end(scenario);
}
