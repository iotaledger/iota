// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
    result::Error,
    BoolExpressionMethods, Connection, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl,
    SelectableHelper,
};
use iota_types::digests::TransactionDigest;

use crate::{
    models::{IotaProgressStore, TokenTransfer as DBTokenTransfer},
    schema,
    schema::{
        iota_error_transactions, iota_progress_store::txn_digest, token_transfer,
        token_transfer_data,
    },
    ProcessedTxnData,
};

pub(crate) type PgPool = Pool<ConnectionManager<PgConnection>>;

const IOTA_PROGRESS_STORE_DUMMY_KEY: i32 = 1;

pub fn get_connection_pool(database_url: String) -> PgPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build Postgres DB connection pool")
}

// TODO: add retry logic
pub fn write(pool: &PgPool, token_txns: Vec<ProcessedTxnData>) -> Result<(), anyhow::Error> {
    if token_txns.is_empty() {
        return Ok(());
    }
    let (transfers, data, errors) = token_txns.iter().fold(
        (vec![], vec![], vec![]),
        |(mut transfers, mut data, mut errors), d| {
            match d {
                ProcessedTxnData::TokenTransfer(t) => {
                    transfers.push(t.to_db());
                    if let Some(d) = t.to_data_maybe() {
                        data.push(d)
                    }
                }
                ProcessedTxnData::Error(e) => errors.push(e.to_db()),
            }
            (transfers, data, errors)
        },
    );

    let connection = &mut pool.get()?;
    connection.transaction(|conn| {
        diesel::insert_into(token_transfer_data::table)
            .values(&data)
            .on_conflict_do_nothing()
            .execute(conn)?;
        diesel::insert_into(token_transfer::table)
            .values(&transfers)
            .on_conflict_do_nothing()
            .execute(conn)?;
        diesel::insert_into(iota_error_transactions::table)
            .values(&errors)
            .on_conflict_do_nothing()
            .execute(conn)
    })?;
    Ok(())
}

pub fn update_iota_progress_store(
    pool: &PgPool,
    tx_digest: TransactionDigest,
) -> Result<(), anyhow::Error> {
    let mut conn = pool.get()?;
    diesel::insert_into(schema::iota_progress_store::table)
        .values(&IotaProgressStore {
            id: IOTA_PROGRESS_STORE_DUMMY_KEY,
            txn_digest: tx_digest.inner().to_vec(),
        })
        .on_conflict(schema::iota_progress_store::dsl::id)
        .do_update()
        .set(txn_digest.eq(tx_digest.inner().to_vec()))
        .execute(&mut conn)?;
    Ok(())
}

pub fn read_iota_progress_store(pool: &PgPool) -> anyhow::Result<Option<TransactionDigest>> {
    let mut conn = pool.get()?;
    let val: Option<IotaProgressStore> =
        crate::schema::iota_progress_store::dsl::iota_progress_store
            .select(IotaProgressStore::as_select())
            .first(&mut conn)
            .optional()?;
    match val {
        Some(val) => Ok(Some(TransactionDigest::try_from(
            val.txn_digest.as_slice(),
        )?)),
        None => Ok(None),
    }
}

pub fn get_latest_eth_token_transfer(
    pool: &PgPool,
    finalized: bool,
) -> Result<Option<DBTokenTransfer>, Error> {
    use crate::schema::token_transfer::dsl::*;

    let connection = &mut pool.get().unwrap();

    if finalized {
        token_transfer
            .filter(data_source.eq("ETH").and(status.eq("Deposited")))
            .order(block_height.desc())
            .first::<DBTokenTransfer>(connection)
            .optional()
    } else {
        token_transfer
            .filter(status.eq("DepositedUnfinalized"))
            .order(block_height.desc())
            .first::<DBTokenTransfer>(connection)
            .optional()
    }
}
