// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AuctionMetadata } from '../types/metadata';

type AuctionMetadataStatus = 'active' | 'ended' | 'not_found';

export type UserAuctionStatus =
    | 'top_bidder'
    | 'outbid'
    | 'winner'
    | 'lost'
    | 'claimable'
    | 'claimed'
    | 'unknown';

export function isAuctionActive(auctionMetadata: AuctionMetadata | null): boolean {
    if (!auctionMetadata) return false;

    const now = new Date();
    return now < auctionMetadata.endTimestamp;
}

export function getAuctionMetadataStatus(
    auctionMetadata: AuctionMetadata | null,
): AuctionMetadataStatus {
    if (!auctionMetadata) return 'not_found';

    if (auctionMetadata.isActive) return 'active';
    return 'ended';
}

function isUserWinner(auctionMetadata: AuctionMetadata | null, userAddress: string): boolean {
    if (!auctionMetadata) return false;

    return auctionMetadata.winner === userAddress;
}

export function getUserAuctionStatus(
    auctionMetadata: AuctionMetadata | null,
    userAddress?: string,
): UserAuctionStatus {
    if (!auctionMetadata || !userAddress) {
        return 'unknown';
    }

    const isWinner = isUserWinner(auctionMetadata, userAddress);
    const auctionMetadataStatus = getAuctionMetadataStatus(auctionMetadata);

    if (auctionMetadataStatus === 'active') {
        return isWinner ? 'top_bidder' : 'outbid';
    } else if (auctionMetadataStatus === 'ended') {
        if (isWinner) {
            return 'claimable';
        } else {
            return 'lost';
        }
    } else if (auctionMetadataStatus === 'not_found') {
        return 'claimed';
    }

    return 'unknown';
}
