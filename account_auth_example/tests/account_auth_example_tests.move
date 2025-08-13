
#[test_only]
module account_auth_example::account_auth_example_tests;

use account_auth_example::m;
use std::ascii;
use iota::account;

#[test]
fun bind_to_test_module() {
    // This is necessary to force the move compiler in including the
    // account_auth_example::m in the test module.
    // Without this call, the main module won't be available from the test module
    // making all the test fail outright, because the functions weren't found.
    m::bind_to_test_module();
}

#[test]
fun test_minimally_viable_auth_function() {
    let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"minimally_viable_auth_function"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun has_to_be_public_auth_function() {
    let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"has_to_be_public_auth_function"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun crate_auth_at_least_two_args() {
    let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"crate_auth_at_least_two_args"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun crate_auth_auth_context_cant_be_value() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"crate_auth_auth_context_cant_be_value"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun auth_context_cant_be_mutable_ref() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"auth_context_cant_be_mutable_ref"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun tx_context_cant_be_value() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"tx_context_cant_be_value"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun tx_context_cant_be_mutable_ref() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"tx_context_cant_be_mutable_ref"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun auth_context_isnt_struct() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"auth_context_isnt_struct"));
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun tx_context_isnt_struct() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"tx_context_is_struct"));
    account::drop_auth_info_v1(account_info);
}

#[test]
fun arg_immutable_ref() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"arg_immutable_ref"));
    account::drop_auth_info_v1(account_info);
}

#[test]
fun arg_value() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"arg_value"));
    account::drop_auth_info_v1(account_info);
}

#[test]
fun arg_mutable_value() {
   let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"arg_mutable_value"));
    account::drop_auth_info_v1(account_info);
}