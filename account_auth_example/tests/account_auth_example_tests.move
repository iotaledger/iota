
#[test_only]
module account_auth_example::account_auth_example_tests;

use account_auth_example::m;

#[test]
fun test_account_auth_example() {
    m::crate_auth();
}

