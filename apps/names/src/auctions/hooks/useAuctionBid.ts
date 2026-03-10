// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';
import { queryKey } from '@/hooks/queryKey';

import { buildCreateAuctionTransaction, buildPlaceBidTransaction } from '../lib/utils/transaction';
import { useAuctionHouse } from './useAuctionHouse';
import { useGetAuctionMetadata } from './useGetAuctionMetadata';

export interface UseActionBidParams {
    name: string;
    bidNanos: bigint;
}

export function useAuctionBid({ name, bidNanos }: UseActionBidParams) {
    const account = useCurrentAccount();
    const { iotaNamesClient } = useIotaNamesClient();
    const iotaClient = useIotaClient();
    const address = account?.address ?? '';
    const { data: auctionHouse } = useAuctionHouse();
    const { data: auctionMetadata, isLoading: isLoadingAuctionMetadata } =
        useGetAuctionMetadata(name);

    // Only once its finished loading we check if the bid amount is enough or not, but only if there is an existing auction
    const enableAuctionBid = !isLoadingAuctionMetadata
        ? auctionMetadata
            ? bidNanos >= auctionMetadata.minBidNanos
            : true
        : false;

    const isAuctionPresent = !!auctionMetadata;

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            ...queryKey.placeBid(address || ''),
            name,
            bidNanos.toString(),
            isAuctionPresent,
        ],
        queryFn: async () => {
            if (!auctionHouse) throw new Error('Auction house not loaded');
            const auctionPackageId = iotaNamesClient.getPackage('auctionPackageId');
            const iotaNamesObjectId = iotaNamesClient.getPackage('iotaNamesObjectId');
            const transaction = !isAuctionPresent
                ? buildCreateAuctionTransaction(
                      auctionPackageId,
                      iotaNamesObjectId,
                      auctionHouse.auctionHouseId,
                      address,
                      bidNanos,
                      name,
                  )
                : buildPlaceBidTransaction(
                      auctionPackageId,
                      auctionHouse.auctionHouseId,
                      address,
                      bidNanos,
                      name,
                  );

            const txBytes = await transaction.build({
                client: iotaClient,
            });

            const txDryRun = await iotaClient.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });

            if (txDryRun.effects.status.status !== 'success') {
                throw new Error(txDryRun.effects.status.error || 'Transaction dry run failed');
            }

            return {
                txBytes,
            };
        },
        enabled: enableAuctionBid,
        select: ({ txBytes }) => {
            return {
                transaction: Transaction.from(txBytes),
            };
        },
    });
}
