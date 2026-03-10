// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { useMutation, UseMutationOptions } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';
import { queryKey } from '@/hooks/queryKey';

import { buildClaimNameTransaction } from '../lib/utils/transaction';
import { useAuctionHouse } from './useAuctionHouse';

export function useClaimAuctionTransaction(
    address: string,
    name: string,
    options: UseMutationOptions = {},
) {
    const { iotaNamesClient } = useIotaNamesClient();
    const { data: auctionHouseData } = useAuctionHouse();
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const iotaClient = useIotaClient();

    return useMutation({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        mutationKey: [...queryKey.claimAuction(name, address)],
        mutationFn: async () => {
            if (!auctionHouseData?.auctionHouseId) {
                throw new Error('Auction house not available');
            }

            const auctionPackageId = iotaNamesClient.getPackage('auctionPackageId');

            const transaction = buildClaimNameTransaction(
                auctionPackageId,
                auctionHouseData.auctionHouseId,
                address,
                name,
            );

            const builtTransaction = await signAndExecuteTransaction({
                transaction,
            });

            await iotaClient.waitForTransaction({
                digest: builtTransaction.digest,
            });
        },
        gcTime: 0,
        ...options,
    });
}
