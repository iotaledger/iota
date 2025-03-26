// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::test_utils {
    public fun assert_eq<T: drop>(t1: T, t2: T) {
        assert_ref_eq(&t1, &t2)
    }

    public fun assert_ref_eq<T>(t1: &T, t2: &T) {
        let res = t1 == t2;
        if (!res) {
            print(b"Assertion failed:");
            std::debug::print(t1);
            print(b"!=");
            std::debug::print(t2);
            abort(0)
        }
    }

    /// Function checks that the two passed vectors contain the same elements,
    /// regardless of their position in the vector.
    public fun assert_same_elems<T: drop>(t1: vector<T>, t2: vector<T>) {
        let len1 = t1.length();
        let len2 = t2.length();
        // If lengths are different, they can't be equal
        if (len1 != len2) {
            print(b"Assertion failed: lengths do not match");
            std::debug::print(&len1);
            print(b"!=");
            std::debug::print(&len2);
            abort(0)
        };

        // Vectors to store unique elements and their counts
        let mut unique_values = vector<u64>[];
        let mut counts1 = vector<u64>[];
        let mut counts2 = vector<u64>[];

        // Count occurrences in v1
        let mut i = 0;
        while (i < len1) {
            let value = &t1[i];
            let mut found = false;
            let mut j = 0;

            while (j < vector::length(&unique_values)) {
                if (&t1[unique_values[j]] == value) {
                    let count = counts1[j];
                    *vector::borrow_mut(&mut counts1, j) = count + 1;
                    found = true;
                    break
                };
                j = j + 1;
            };

            if (!found) {
                vector::push_back(&mut unique_values, i);
                vector::push_back(&mut counts1, 1);
                vector::push_back(&mut counts2, 0);
            };

            i = i + 1;
        };

        // Count occurrences in v2
        let mut i = 0;
        while (i < len2) {
            let value = &t2[i];
            let mut found = false;
            let mut j = 0;

            while (j < vector::length(&unique_values)) {
                if (&t1[unique_values[j]] == value) {
                    let count = counts2[j];
                    *vector::borrow_mut(&mut counts2, j) = count + 1;
                    found = true;
                    break
                };
                j = j + 1;
            };

            if (!found) {
                print(b"Assertion failed: elements do not match");
                std::debug::print(&t1);
                print(b"!=");
                std::debug::print(&t2);
                abort(0) 
            };

            i = i + 1;
        };

        // Compare counts
        let mut i = 0;
        while (i < vector::length(&counts1)) {
            if (counts1[i] != counts2[i]) {
                print(b"Assertion failed: elements do not match");
                std::debug::print(&t1);
                print(b"!=");
                std::debug::print(&t2);
                abort(0) 
            };
            i = i + 1;
        };
    }

    public fun print(str: vector<u8>) {
        std::debug::print(&str.to_ascii_string())
    }

    public native fun destroy<T>(x: T);

    public native fun create_one_time_witness<T: drop>(): T;
}
