#[test_only]
module account_auth_example::account_auth_example_tests;

use account_auth_example::main_m::AUTH_ARG_VALUE;
use iota::account::create_auth_info_v1_fotw;

public struct NOT_AOTW has drop {}

// WON'T BUILD
// public struct AUTH_FAIL has drop {}

#[test]
fun aotw_success() {
    create_auth_info_v1_fotw<AUTH_ARG_VALUE>();
}

#[test, expected_failure]
fun aotw_fail() {
    create_auth_info_v1_fotw<NOT_AOTW>();
}
