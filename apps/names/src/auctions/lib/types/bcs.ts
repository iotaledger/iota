// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';

export const NameBcs = bcs.struct('Name', {
    labels: bcs.vector(bcs.string()),
});

const BalanceBcs = bcs.struct('Balance', {
    value: bcs.u64(),
});

export const AuctionHouseBcs = bcs.struct('AuctionHouse', {
    id: bcs.Address,
    balance: BalanceBcs,
    auctions: bcs.struct('LinkedTable', {
        id: bcs.Address,
        size: bcs.u64(),
        head: bcs.option(NameBcs),
        tail: bcs.option(NameBcs),
    }),
});

export const AuctionFieldBcs = bcs.struct('AuctionField', {
    id: bcs.Address,
    name: NameBcs,
    value: bcs.struct('Node', {
        prev: bcs.option(NameBcs),
        next: bcs.option(NameBcs),
        value: bcs.struct('Auction', {
            name: NameBcs,
            start_timestamp_ms: bcs.u64(),
            end_timestamp_ms: bcs.u64(),
            winner: bcs.Address,
            current_bid: bcs.struct('Coin', {
                id: bcs.Address,
                balance: BalanceBcs,
            }),
            nft: bcs.struct('Nft', {
                id: bcs.Address,
                name: NameBcs,
                name_str: bcs.string(),
                expiration_timestamp_ms: bcs.u64(),
            }),
        }),
    }),
});
