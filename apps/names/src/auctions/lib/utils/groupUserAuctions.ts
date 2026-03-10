// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AuctionDetails } from '@/auctions/hooks/useAuctions';

import { getUserAuctionStatus } from './auctionStatus';

export type AuctionCard = {
    details: AuctionDetails;
    status: AuctionCardStatus;
};

export enum AuctionCardStatus {
    Active = 'active',
    Claimable = 'claimable',
    Lost = 'lost',
}

export function groupUserAuctions(auctionDetails: AuctionDetails[], address: string) {
    return auctionDetails.reduce((acc, auctionDetails): AuctionCard[] => {
        const status = getUserAuctionStatus(auctionDetails.metadata, address);

        if (status === 'top_bidder' || status === 'outbid') {
            acc.push({ details: auctionDetails, status: AuctionCardStatus.Active });
        } else if (status === 'claimable' || status === 'winner') {
            acc.push({ details: auctionDetails, status: AuctionCardStatus.Claimable });
        } else if (status === 'lost') {
            acc.push({ details: auctionDetails, status: AuctionCardStatus.Lost });
        }

        return acc;
    }, [] as AuctionCard[]);
}
