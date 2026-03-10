// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { queryKey } from '@/hooks';

import { AuctionMetadata } from '../lib/types';
import { getTimeRemaining } from '../lib/utils';

/**
 * Refresh the auction once it has ended.
 */
export function useAuctionRefresh(auctionMetadata?: AuctionMetadata | null) {
    const queryClient = useQueryClient();

    useEffect(() => {
        if (auctionMetadata && auctionMetadata.isActive) {
            const timeRemainingMs = getTimeRemaining(auctionMetadata);
            const timeout = setTimeout(() => {
                queryClient.invalidateQueries({
                    queryKey: [...queryKey.auctionMetadata(auctionMetadata.name)],
                    exact: false,
                });
            }, timeRemainingMs);

            return () => clearTimeout(timeout);
        }
    }, [auctionMetadata?.name, auctionMetadata?.isActive]);
}
