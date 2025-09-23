#[test_only]
module account_auth_example::account_auth_example_tests;

use account_auth_example::main_m::AUTH_SOME_AUTHENTICATE_FN;
use iota::account::create_auth_info_v1_fotw;

public struct NOT_AOTW has drop {}

#[test]
fun aotw_success() {
    create_auth_info_v1_fotw<AUTH_SOME_AUTHENTICATE_FN>();
}

#[test, expected_failure]
fun aotw_fail() {
    create_auth_info_v1_fotw<NOT_AOTW>();
}
