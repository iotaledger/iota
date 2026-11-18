// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_http::Config;
use tokio::net::TcpStream;

use crate::common::{ok_app, serve, setup, wait_until};

#[tokio::test]
async fn connections_of_a_client_are_counted_at_each_accept() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, ok_app());

    let mut streams = vec![];
    for _ in 0..3 {
        streams.push(TcpStream::connect(address).await.unwrap());
        let open = streams.len() as u64;
        wait_until("the connection to be counted", || {
            reader.histogram_totals("client_connections", &[]).count == open
        })
        .await;
    }

    let client_connections =
        reader.histogram_totals("client_connections", &[("scope", "loopback")]);
    assert_eq!(
        (client_connections.count, client_connections.sum),
        (3, 6.0),
        "1 + 2 + 3"
    );
    assert_eq!(
        reader.value("client_connections_peak", &[("scope", "loopback")]),
        3.0
    );
    assert_eq!(reader.value("clients", &[("scope", "loopback")]), 1.0);
    assert_eq!(reader.value("clients", &[("scope", "public")]), 0.0);

    drop(streams);
    wait_until("the client to leave", || {
        reader.value("clients", &[("scope", "loopback")]) == 0.0
    })
    .await;
    let connect_again = TcpStream::connect(address).await.unwrap();
    wait_until("the new connection", || {
        reader.histogram_totals("client_connections", &[]).count == 4
    })
    .await;
    assert_eq!(
        reader.histogram_totals("client_connections", &[]).sum,
        7.0,
        "the key left with its last connection, so the next one is the first"
    );
    drop(connect_again);
}
