// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useCurrentAccount } from '@iota/dapp-kit';
import { useState } from 'react';

import { AuctionBidDialog, AuctionDetails, AuctionItem, getUserAuctionStatus } from '@/auctions';

interface ExtendedNameCardProps {
    name: string;
    auctionDetails: AuctionDetails;
}

export function ExtendedAuctionCard({ name, auctionDetails }: ExtendedNameCardProps) {
    const [showBidDialog, setShowBidDialog] = useState<boolean>(false);
    const address = useCurrentAccount()?.address ?? '';
    const status = getUserAuctionStatus(auctionDetails.metadata, address);

    return (
        <>
            <AuctionItem
                auction={auctionDetails}
                onBidClick={() => setShowBidDialog(true)}
                auctionStatus={status}
            />

            {showBidDialog && (
                <AuctionBidDialog
                    name={name}
                    closeDialog={() => {
                        setShowBidDialog(false);
                    }}
                />
            )}
        </>
    );
}
