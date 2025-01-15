// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This example demonstrates different way to use errors.
module errors::errors {
    const EConstError: u64 = 1;
    const EAbortError: u64 = 2;
    #[error]
    const EVectorError: vector<u8> = b"This is an error message.";

    public entry fun numeric_error() {
        assert!(
            false,
            0,
        );
    }

    public entry fun numeric_const_error() {
        assert!(
            false,
            EConstError,
        );
    }

    public entry fun abort_error() {
        abort EAbortError
    }

    public entry fun vector_error() {
        abort EVectorError
    }
}

#[test_only]
module errors::errors_test {
    use iota::test_scenario as ts;

    #[test]
    #[expected_failure(abort_code = 0, location=errors::errors)]
    fun test_numeric_error() {
        let user0 = @0xA;
        let mut ts = ts::begin(user0);

        {
            ts.next_tx(user0);
            errors::errors::numeric_error();
        };

        ts.end();
    }

    #[test]
    #[expected_failure(abort_code = errors::errors::EConstError)]
    fun test_numeric_const_error() {
        let user0 = @0xA;
        let mut ts = ts::begin(user0);

        {
            ts.next_tx(user0);
            errors::errors::numeric_const_error();
        };

        ts.end();
    }

    #[test]
    #[expected_failure(abort_code = errors::errors::EAbortError)]
    fun test_abort_error() {
        let user0 = @0xA;
        let mut ts = ts::begin(user0);

        {
            ts.next_tx(user0);
            errors::errors::abort_error();
        };

        ts.end();
    }

    #[test]
    #[expected_failure(abort_code = errors::errors::EVectorError)]
    fun test_vector_error() {
        let user0 = @0xA;
        let mut ts = ts::begin(user0);

        {
            ts.next_tx(user0);
            errors::errors::vector_error();
        };

        ts.end();
    }
}


/* CLI commands, package is published in the testnet

PACKAGE_ID=0x974d61b4e495aa6b464759cbb9e1acea321246151d5db9afd3aacd1d8cda03dc
iota client ptb \
--move-call $PACKAGE_ID::errors::numeric_error \
--gas-budget 10000000

iota client ptb \
--move-call $PACKAGE_ID::errors::numeric_const_error \
--gas-budget 10000000

iota client ptb \
--move-call $PACKAGE_ID::errors::abort_error \
--gas-budget 10000000

iota client ptb \
--move-call $PACKAGE_ID::errors::vector_error \
--gas-budget 10000000

*/
