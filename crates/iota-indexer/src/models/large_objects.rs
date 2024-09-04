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
    select,
    sql_types::{BigInt, Binary, Integer, Nullable},
    RunQueryDsl,
};

use crate::{
    db::PgConnectionPool,
    errors::{Context, IndexerError},
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
    /// Returns an error if the object doesn't exist.
    fn lo_get(loid: Oid, offset: Nullable<BigInt>, length: Nullable<Integer> ) -> Binary
}

/// Store raw data as a large object in chunks.
///
/// Returns the object identifier (`oid`) represented as `u32`.
pub fn put_large_object(
    data: Vec<u8>,
    chunk_size: usize,
    pool: &PgConnectionPool,
) -> Result<u32, IndexerError> {
    let mut conn = crate::db::get_pg_pool_connection(pool)?;
    let oid: u32 = select(lo_create(0))
        .get_result(&mut conn)
        .map_err(IndexerError::from)
        .context("failed to store large object")?;

    if let Err(err) = i64::try_from(data.len()) {
        return Err(IndexerError::GenericError(err.to_string()));
    }

    for (chunk_num, chunk) in data.chunks(chunk_size).enumerate() {
        let offset = (chunk_num * chunk_size) as i64;
        tracing::trace!("storing large-object chunk at offset {}", offset);
        // TODO: (to treat in a different issue):
        // remove dangling chunks (either by using a transaction or by handlng manually)
        //
        // additionally we could apply a backoff retry strategy
        select(lo_put(oid, offset, chunk))
            .execute(&mut conn)
            .map_err(IndexerError::from)
            .context("failed to insert large object chunk")?;
    }
    Ok(oid)
}

/// Get a large object from the database in chunks.
pub fn get_large_object(
    oid: u32,
    chunk_size: usize,
    pool: &PgConnectionPool,
) -> Result<Vec<u8>, IndexerError> {
    let mut conn = crate::db::get_pg_pool_connection(pool)?;
    let mut chunk_num = 0;
    let mut data: Vec<u8> = vec![];
    loop {
        let length =
            i32::try_from(chunk_size).map_err(|e| IndexerError::GenericError(e.to_string()))?;
        let offset = (chunk_num * chunk_size) as i64;

        tracing::trace!("fetching large-object chunk at offset {}", offset);

        let chunk = select(lo_get(oid, Some(offset), Some(length)))
            .get_result::<Vec<u8>>(&mut conn)
            .map_err(IndexerError::from)
            .context("failed to query large object chunk")?;

        let chunk_len = chunk.len();
        data.extend(chunk);

        if chunk_len < chunk_size {
            break;
        }
        chunk_num += 1;
    }
    Ok(data)
}
