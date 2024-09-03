// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Expose server-side large-object functions.
//!
//! Based on https://github.com/diesel-rs/diesel/issues/2127#issuecomment-846524605
//!
//! See also https://www.postgresql.org/docs/current/lo-funcs.html.

use diesel::{
    define_sql_function,
    pg::sql_types::Oid,
    sql_types::{BigInt, Binary, Integer, Nullable},
};

define_sql_function! {
    /// Returns an `Oid` of an empty new large object.
    fn lo_create(loid: Oid) -> Oid
}

define_sql_function! {
    /// Writes data starting at the given offset within
    /// the large object.
    ///
    /// The large object is enlarged if necessary.
    fn lo_put(loid: Oid, offset: BigInt, data: Binary)
}

define_sql_function! {
    /// Gets the large object with OID `loid`.
    /// Returns an erorr if the object doesn't exist.
    fn lo_get(loid: Oid, offset: Nullable<BigInt>, length: Nullable<Integer> ) -> Binary
}
