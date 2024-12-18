// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, bb8::Pool},
};

pub(crate) type PgPool =
    diesel_async::pooled_connection::bb8::Pool<diesel_async::AsyncPgConnection>;

pub async fn get_connection_pool(database_url: String) -> PgPool {
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .await
        .expect("Could not build Postgres DB connection pool")
}
