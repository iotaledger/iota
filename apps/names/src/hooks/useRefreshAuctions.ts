// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import toast from 'react-hot-toast';

import { AuctionDetails } from '@/auctions';
import { queryKey } from '@/hooks';

export function useRefreshAuctions(auctions: AuctionDetails[]) {
    const queryClient = useQueryClient();
    const account = useCurrentAccount();
    const [isRefreshing, setIsRefreshing] = useState(false);

    async function handleRefresh(): Promise<void> {
        if (isRefreshing) return;

        setIsRefreshing(true);
        try {
            // Invalidate auctions data
            await queryClient.invalidateQueries({
                queryKey: queryKey.userAuctionHistory(account?.address),
            });

            // Invalidate auction metadata
            await queryClient.invalidateQueries({
                queryKey: queryKey.auctionList(),
            });

            // Invalidate auction metadata for specific auctions the user is participating in
            if (auctions && auctions.length > 0) {
                const invalidatePromises = auctions.map((auction) =>
                    queryClient.invalidateQueries({
                        queryKey: queryKey.auctionMetadata(auction.name),
                    }),
                );
                await Promise.all(invalidatePromises);
            }

            toast.success('Refreshed successfully');
        } catch (error) {
            toast.error('Failed to refresh data');
        } finally {
            setIsRefreshing(false);
        }
    }

    return {
        isRefreshing,
        handleRefresh,
    };
}
