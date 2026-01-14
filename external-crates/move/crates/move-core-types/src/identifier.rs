// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! An identifier is the name of an entity (module, resource, function, etc) in
//! Move.
//!
//! A valid identifier consists of an ASCII string which satisfies any of the
//! conditions:
//!
//! * The first character is a letter and the remaining characters are letters,
//!   digits or underscores.
//! * The first character is an underscore, and there is at least one further
//!   letter, digit or underscore.
//!
//! The spec for allowed identifiers is similar to Rust's spec
//! ([as of version 1.38](https://doc.rust-lang.org/1.38.0/reference/identifiers.html)).
//!
//! Allowed identifiers are currently restricted to ASCII due to unresolved
//! issues with Unicode normalization. See [Rust issue #55467](https://github.com/rust-lang/rust/issues/55467) and the
//! associated RFC for some discussion. Unicode identifiers may eventually be
//! supported once these issues are worked out.
//!
//! This module only determines allowed identifiers at the bytecode level. Move
//! source code will likely be more restrictive than even this, with a "raw
//! identifier" escape hatch similar to Rust's `r#` identifiers.
//!
//! Among other things, identifiers are used to:
//! * specify keys for lookups in storage
//! * do cross-module lookups while executing transactions

/// Return true if this character can appear in a Move identifier.
///
/// Note: there are stricter restrictions on whether a character can begin a
/// Move identifier--only alphabetic characters are allowed here.
#[inline]
pub const fn is_valid_identifier_char(c: char) -> bool {
    matches!(c, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9')
}

/// Returns `true` if all bytes in `b` after the offset `start_offset` are valid
/// ASCII identifier characters.
const fn all_bytes_valid(b: &[u8], start_offset: usize) -> bool {
    let mut i = start_offset;
    // TODO(philiphayes): use for loop instead of while loop when it's stable in
    // const fn's.
    while i < b.len() {
        if !is_valid_identifier_char(b[i] as char) {
            return false;
        }
        i += 1;
    }
    true
}

/// Describes what identifiers are allowed.
///
/// For now this is deliberately restrictive -- we would like to evolve this in
/// the future.
// TODO: "<SELF>" is coded as an exception. It should be removed once
// CompiledScript goes away. Note: needs to be pub as it's used in the
// `ident_str!` macro.
pub const fn is_valid(s: &str) -> bool {
    // Rust const fn's don't currently support slicing or indexing &str's, so we
    // have to operate on the underlying byte slice. This is not a problem as
    // valid identifiers are (currently) ASCII-only.
    let b = s.as_bytes();
    match b {
        b"<SELF>" => true,
        [b'a'..=b'z', ..] | [b'A'..=b'Z', ..] => all_bytes_valid(b, 1),
        [b'_', ..] if b.len() > 1 => all_bytes_valid(b, 1),
        _ => false,
    }
}

/// Returns the abstract size of the identifier
/// TODO (ade): use macro to enforce determinism
pub fn identifier_abstract_size_for_gas_metering(ident: &IdentStr) -> AbstractMemorySize {
    AbstractMemorySize::new((ident.len()) as u64)
}

/// A regex describing what identifiers are allowed. Used for proptests.
// TODO: "<SELF>" is coded as an exception. It should be removed once CompiledScript goes away.
#[cfg(any(test, feature = "fuzzing"))]
#[allow(dead_code)]
pub(crate) static ALLOWED_IDENTIFIERS: &str =
    r"(?:[a-zA-Z][a-zA-Z0-9_]*)|(?:_[a-zA-Z0-9_]+)|(?:<SELF>)";
#[cfg(any(test, feature = "fuzzing"))]
#[allow(dead_code)]
pub(crate) static ALLOWED_NO_SELF_IDENTIFIERS: &str =
    r"(?:[a-zA-Z][a-zA-Z0-9_]*)|(?:_[a-zA-Z0-9_]+)";

pub use iota_sdk_types::{Identifier, IdentifierRef as IdentStr};

use crate::gas_algebra::AbstractMemorySize;

/// `ident_str!` is a compile-time validated macro that constructs a
/// `&'static IdentStr` from a const `&'static str`.
///
/// ### Example
///
/// Creating a valid static or const [`IdentStr`]:
///
/// ```rust
/// use move_core_types::{ident_str, identifier::IdentStr};
/// const VALID_IDENT: &'static IdentStr = ident_str!("MyCoolIdentifier");
///
/// const THING_NAME: &'static str = "thing_name";
/// const THING_IDENT: &'static IdentStr = ident_str!(THING_NAME);
/// ```
///
/// In contrast, creating an invalid [`IdentStr`] will fail at compile time:
///
/// ```rust,compile_fail
/// use move_core_types::{ident_str, identifier::IdentStr};
/// const INVALID_IDENT: &'static IdentStr = ident_str!("123Foo"); // Fails to compile!
/// ```
// TODO(philiphayes): this should really be an associated const fn like `IdentStr::new`;
// unfortunately, both unsafe-reborrow and unsafe-transmute don't currently work
// inside const fn's. Only unsafe-transmute works inside static const-blocks
// (but not const-fn's).
#[macro_export]
macro_rules! ident_str {
    ($ident:expr) => {
        $crate::identifier::IdentStr::const_new($ident)
    };
}
