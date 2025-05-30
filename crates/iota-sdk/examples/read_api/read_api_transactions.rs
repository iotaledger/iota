// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example uses the ReadApi to get transaction blocks as paginated
//! response and stream.
//!
//! cargo run --example read_api_transactions

use futures::StreamExt;
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::{IotaTransactionBlockResponseQuery, TransactionFilter},
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    // Get the latest 5 transaction blocks, so we can use a digest of it as cursor
    let transactions_block_page = client
        .read_api()
        .query_transaction_blocks(IotaTransactionBlockResponseQuery::default(), None, 5, true)
        .await?;

    // Get the last ~5 transaction blocks as stream (the tx for the digest in the
    // cursor is not returned, so there could be 4, but there could also already be
    // new tx(s), so that there are 5 or even more)
    let mut txs = client
        .read_api()
        .get_transactions_stream(
            IotaTransactionBlockResponseQuery::default(),
            transactions_block_page.data.last().unwrap().digest,
            false,
        )
        .boxed();

    while let Some(tx) = txs.next().await {
        println!("{tx:?}");
    }

    // Get a tx stream with a TransactionFilter
    let mut txs = client
        .read_api()
        .get_transactions_stream(
            IotaTransactionBlockResponseQuery::new_with_filter(
                TransactionFilter::MoveFunction {
                    package: "0x3".parse()?,
                    module: Some("iota_system".into()),
                    function: Some("request_add_stake".into()),
                },
                // There are also options for filtering txs, for example by address:
                // TransactionFilter::FromOrToAddress {
                //     addr: "0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215"
                //         .parse()?,
                // },
            ),
            None,
            true,
        )
        .boxed();
    println!(
        "Latest tx that called 0x3::iota_system::request_add_stake:\n{:?}",
        txs.next().await.unwrap()
    );

    Ok(())
}
