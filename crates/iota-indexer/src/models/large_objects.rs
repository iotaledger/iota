// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Expose server-side large-object functions.
//!
//! Based on <https://github.com/diesel-rs/diesel/issues/2127#issuecomment-846524605>
//!
//! See also <https://www.postgresql.org/docs/current/lo-funcs.html>

use std::time::Duration;

use diesel::{
    RunQueryDsl, define_sql_function,
    pg::sql_types::Oid,
    r2d2::R2D2Connection,
    select,
    sql_types::{BigInt, Binary, Integer, Nullable},
};
use downcast::Any;

use crate::{
    db::ConnectionPool,
    errors::{Context, IndexerError},
    transactional_blocking_with_retry,
};

const DB_COMMIT_SLEEP_DURATION: Duration = Duration::from_secs(3600);

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

/// Create an empty large object
///
/// Returns the object identifier (`oid`) represented as `u32`.
pub fn create_large_object<T: R2D2Connection + Send + 'static>(
    pool: &ConnectionPool<T>,
) -> Result<u32, IndexerError> {
    transactional_blocking_with_retry!(
        pool,
        |conn| select(lo_create(0)).get_result(conn),
        DB_COMMIT_SLEEP_DURATION
    )
    .map_err(IndexerError::from)
    .context("failed to store large object")
}

/// Store raw data as a large object in chunks.
///
/// Returns the object identifier (`oid`) represented as `u32`.
pub fn put_large_object_in_chunks<T: R2D2Connection + Send + 'static>(
    data: Vec<u8>,
    chunk_size: usize,
    pool: &ConnectionPool<T>,
) -> Result<u32, IndexerError> {
    let oid = create_large_object(pool)?;

    if let Err(err) = i64::try_from(data.len()) {
        return Err(IndexerError::Generic(err.to_string()));
    }

    for (chunk_num, chunk) in data.chunks(chunk_size).enumerate() {
        let offset = (chunk_num * chunk_size) as i64;
        tracing::trace!("Storing large-object chunk at offset {}", offset);
        // TODO: (to treat in a different issue):
        // remove dangling chunks (either by using a transaction or by handling
        // manually)
        //
        // additionally we could apply a backoff retry strategy
        transactional_blocking_with_retry!(
            pool,
            |conn| select(lo_put(oid, offset, chunk)).execute(conn),
            DB_COMMIT_SLEEP_DURATION
        )
        .map_err(IndexerError::from)
        .context("failed to insert large object chunk")?;
    }
    Ok(oid)
}

/// Get a large object from the database in chunks.
pub fn get_large_object_in_chunks<T: R2D2Connection + Send + 'static>(
    oid: u32,
    chunk_size: usize,
    pool: &ConnectionPool<T>,
) -> Result<Vec<u8>, IndexerError> {
    let mut data: Vec<u8> = vec![];
    let mut chunk_num = 0;
    loop {
        let length = i32::try_from(chunk_size).map_err(|e| IndexerError::Generic(e.to_string()))?;
        let offset = (chunk_num * chunk_size) as i64;

        tracing::trace!("Fetching large-object chunk at offset {}", offset);

        let chunk = transactional_blocking_with_retry!(
            pool,
            |conn| { select(lo_get(oid, Some(offset), Some(length))).get_result::<Vec<u8>>(conn) },
            DB_COMMIT_SLEEP_DURATION
        )
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
