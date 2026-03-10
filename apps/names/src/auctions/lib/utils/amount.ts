// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NANOS_PER_IOTA } from '@iota/iota-sdk/utils';

import { AuctionMetadata } from '../types/metadata';

export function getCurrentBidAmount(auction: AuctionMetadata | null): bigint {
    if (!auction) return BigInt(0);

    return auction.currentBidNanos;
}

export function getNextBidAmount(auction: AuctionMetadata | null): bigint {
    const currentBid = getCurrentBidAmount(auction);
    const oneIotaInNanos = BigInt(1) * NANOS_PER_IOTA;

    return currentBid + oneIotaInNanos;
}
