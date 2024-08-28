// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTransactionSummary } from './';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

export function useExtendedTransactionSummary(
    transactionDigest: string,
): ReturnType<typeof useTransactionSummary> {
    const currentAccount = useCurrentAccount();
    const client = useIotaClient();

    const { data: transactionData } = useQuery<IotaTransactionBlockResponse>({
        queryKey: ['transactions-by-id', transactionDigest],
        queryFn: async () => {
            return client.getTransactionBlock({
                digest: transactionDigest,
                options: {
                    showBalanceChanges: true,
                    showObjectChanges: true,
                    showInput: true,
                    showEffects: true,
                    showEvents: true,
                },
            });
        },
        enabled: !!transactionDigest,
        retry: 8,
    });

    const summary = useTransactionSummary({
        transaction: transactionData,
        currentAddress: currentAccount?.address,
        recognizedPackagesList: [],
    });

    return summary;
}
