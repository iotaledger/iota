// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests that fetching events filtered on a tx digest that has no events correctly returns no nodes.
// Also tests that fetching events filtered on a tx digest that has events returns the correct
// number of page-limit-bound nodes.

//# init --protocol-version 1 --addresses Test=0x0 --accounts A B --simulator

//# publish
module Test::M1 {
    use iota::event;

    public struct EventA has copy, drop {
        new_value: u64
    }

    public entry fun no_emit(value: u64): u64 {
        value
    }

    public entry fun emit_2(value: u64) {
        event::emit(EventA { new_value: value });
        event::emit(EventA { new_value: value + 1})
    }
}

//# run Test::M1::no_emit --sender A --args 0

//# run Test::M1::emit_2 --sender A --args 2

//# run Test::M1::emit_2 --sender B --args 4

//# create-checkpoint

//# run-graphql
{
    transactionBlocks {
        nodes {
            digest
        }
    }
}

//# run-graphql
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(filter: {transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql --cursors {"tx":3,"e":1,"c":1}
# When the tx digest and after cursor are on the same tx, we'll use the after cursor's event sequence number
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(after: "@{cursor_0}" filter: {transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql --cursors {"tx":1,"e":1,"c":1}
# If the after cursor does not match the transaction digest's tx sequence number,
# we will get an empty response, since it's not possible to fetch an event
# that isn't of the same tx sequence number
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(after: "@{cursor_0}" filter: {transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}


//# run-graphql
{
    # `transactionDigest` is the digest of the 5th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(filter: {transactionDigest: "EjPV7ZNfRSSNrwYojeSier99WUqXn6n5ZcCM3MqoGSvP"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql --cursors {"tx":4,"e":0,"c":1}
{
    # `transactionDigest` is the digest of the 5th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(after: "@{cursor_0}" filter: {transactionDigest: "EjPV7ZNfRSSNrwYojeSier99WUqXn6n5ZcCM3MqoGSvP"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}


//# run-graphql
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(last: 10 filter: {transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql --cursors {"tx":3,"e":1,"c":1}
# When the tx digest and cursor are on the same tx, we'll use the cursor's event sequence number
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(last: 10 before: "@{cursor_0}" filter: {transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql --cursors {"tx":4,"e":1,"c":1}
# If the cursor does not match the transaction digest's tx sequence number,
# we will get an empty response, since it's not possible to fetch an event
# that isn't of the same tx sequence number
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(last: 10 before: "@{cursor_0}" filter: {transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}


//# run-graphql
{
    # `transactionDigest` is the digest of the 5th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(last: 10 filter: {transactionDigest: "EjPV7ZNfRSSNrwYojeSier99WUqXn6n5ZcCM3MqoGSvP"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql --cursors {"tx":4,"e":1,"c":1}
{
    # `transactionDigest` is the digest of the 5th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(last: 10 before: "@{cursor_0}" filter: {transactionDigest: "EjPV7ZNfRSSNrwYojeSier99WUqXn6n5ZcCM3MqoGSvP"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql
# correct sender
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(filter: {sender: "@{A}" transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql
# correct sender
{
    # `transactionDigest` is the digest of the 5th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(filter: {sender: "@{B}" transactionDigest: "EjPV7ZNfRSSNrwYojeSier99WUqXn6n5ZcCM3MqoGSvP"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql
# incorrect sender
{
    # `transactionDigest` is the digest of the 4th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(filter: {sender: "@{B}" transactionDigest: "9HqHoe72uYughxmWvjskK4g67vLbGScoDZ2q4TEkcDbu"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}

//# run-graphql
# incorrect sender
{
    # `transactionDigest` is the digest of the 5th transaction returned by
    # task 6 (see `tx_digest.exp`)
    events(filter: {sender: "@{A}" transactionDigest: "EjPV7ZNfRSSNrwYojeSier99WUqXn6n5ZcCM3MqoGSvP"}) {
        edges {
            cursor
            node {
                json
            }
        }
    }
}
