// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// ANCHOR: custom
module book::custom_string {
    /// Anyone can implement a custom string-like type by wrapping a vector.
    public struct MyString {
        bytes: vector<u8>,
    }

    /// Implement a `from_bytes` function to convert a vector of bytes to a string.
    public fun from_bytes(bytes: vector<u8>): MyString {
        MyString { bytes }
    }

    /// Implement a `bytes` function to convert a string to a vector of bytes.
    public fun bytes(self: &MyString): &vector<u8> {
        &self.bytes
    }
}
// ANCHOR_END: custom

module book::string_ascii {
    // use std::ascii::String;

    #[allow(unused_variable)]
    #[test]
    fun using_strings() {
        // ANCHOR: ascii
        // the module is `std::ascii` and the type is `String`
        use std::ascii::{Self, String};

        // strings can be created using the `string` function
        // type declaration is not necessary, we put it here for clarity
        let hey: String = ascii::string(b"Hey");

        // there is a handy alias `.to_ascii_string()` on the `vector<u8>` type
        let hey = b"Hey".to_ascii_string();

        // ANCHOR_END: ascii
    }
}

#[allow(unused_variable)]
module book::string_utf {
    #[test]
    fun using_strings() {
        // ANCHOR: utf8
        // the module is `std::string` and the type is `String`
        use std::string::{Self, String};

        // strings are normally created using the `utf8` function
        // type declaration is not necessary, we put it here for clarity
        let hello: String = string::utf8(b"Hello");

        // The `.to_string()` alias on the `vector<u8>` is more convenient
        let hello = b"Hello".to_string();
        // ANCHOR_END: utf8
    }

    #[test]
    fun safe_strings() {
        // ANCHOR: safe_utf8
        // this is a valid UTF-8 string
        let hello = b"Hello".try_to_string();

        assert!(hello.is_some(), 0); // abort if the value is not valid UTF-8

        // this is not a valid UTF-8 string
        let invalid = b"\xFF".try_to_string();

        assert!(invalid.is_none(), 0); // abort if the value is valid UTF-8
        // ANCHOR_END: safe_utf8
    }

    #[test]
    fun string_operations() {
        let mut str = b"Hello,".to_string();
        let another = b" World!".to_string();

        // append(String) adds the content to the end of the string
        str.append(another);

        // `sub_string(start, end)` copies a slice of the string
        str.sub_string(0, 5); // "Hello"

        // `length()` returns the number of bytes in the string
        str.length(); // 12 (bytes)

        // methods can also be chained! Get the length of a substring
        str.sub_string(0, 5).length(); // 5 (bytes)

        // check if the string is empty
        str.is_empty(); // false

        // get the underlying byte vector for custom operations
        let bytes: &vector<u8> = str.as_bytes();
    }
}
