#[test_only]
module account_auth_example::account_auth_example_tests;

use account_auth_example::main_m;
use account_auth_example::object_m;
use account_auth_example::option_m;
use account_auth_example::template_m;
use account_auth_example::vector_m;
use iota::account;
use std::ascii;

#[test]
fun bind_to_test_module() {
    // This is necessary to force the move compiler in including the
    // account_auth_example::m in the test module.
    // Without this call, the main_m module won't be available from the test module
    // making all the test fail outright, because the functions weren't found.
    main_m::bind_to_test_module();
    object_m::bind_to_test_module();
    template_m::bind_to_test_module();
    vector_m::bind_to_test_module();
    option_m::bind_to_test_module();
}

#[test]
fun test_minimally_viable_auth_function() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"minimally_viable_auth_function"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun has_to_be_public_auth_function() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"has_to_be_public_auth_function"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun crate_auth_at_least_two_args() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"crate_auth_at_least_two_args"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun crate_auth_auth_context_cant_be_value() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"crate_auth_auth_context_cant_be_value"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun auth_context_cant_be_mutable_ref() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"auth_context_cant_be_mutable_ref"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun tx_context_cant_be_value() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"tx_context_cant_be_value"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun tx_context_cant_be_mutable_ref() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"tx_context_cant_be_mutable_ref"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun auth_context_isnt_struct() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"auth_context_isnt_struct"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun tx_context_isnt_struct() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"tx_context_is_struct"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun arg_value() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"arg_value"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun arg_mutable_value() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"arg_mutable_value"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun with_signer() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"main_m"),
        ascii::string(b"with_signer"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"object_m"),
        ascii::string(b"object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"object_m"),
        ascii::string(b"object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun object_by_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"object_m"),
        ascii::string(b"object_by_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun template_primitive_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"template_primitive_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun templated_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"templated_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun templated_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"templated_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun templated_non_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"templated_non_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun template_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"template_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun template_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"template_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun template_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"template_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun templated_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"templated_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun templated_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"templated_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun templated_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"template_m"),
        ascii::string(b"templated_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_primitive_immutable_reference_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_primitive_immutable_reference_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_primitive_mutable_reference_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_primitive_mutable_reference_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_primitive_by_value_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_primitive_by_value_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_non_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_non_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_template_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_template_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_template_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_template_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_templated_non_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_templated_non_object_by_value_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_templated_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_templated_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_templated_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_templated_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_template_object_immutable_reference_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_template_object_immutable_reference_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_template_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_template_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_template_object_mutable_reference_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_template_object_mutable_reference_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun vector_templated_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_templated_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_templated_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_templated_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun vector_templated_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"vector_m"),
        ascii::string(b"vector_templated_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_primitive_immutable_reference_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_primitive_immutable_reference_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_primitive_mutable_reference_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_primitive_mutable_reference_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_primitive_by_value_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_primitive_by_value_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_non_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_non_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_template_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_template_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_template_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_template_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_templated_non_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_templated_non_object_by_value_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_templated_non_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_templated_non_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_templated_non_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_templated_non_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_template_object_immutable_reference_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_template_object_immutable_reference_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_template_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_template_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_template_object_mutable_reference_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_template_object_mutable_reference_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test]
fun option_templated_object_immutable_ref_success() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_templated_object_immutable_ref_success"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_templated_object_by_value_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_templated_object_by_value_fail"),
    );
    account::drop_auth_info_v1(account_info);
}

#[test, expected_failure]
fun option_templated_object_mutable_ref_fail() {
    let account_info = account::create_auth_info_v1(
        @0x0,
        ascii::string(b"option_m"),
        ascii::string(b"option_templated_object_mutable_ref_fail"),
    );
    account::drop_auth_info_v1(account_info);
}
