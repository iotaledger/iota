// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { createMigrationTransaction, getGasSummary, useMaxTransactionSizeBytes } from '@iota/core';

export function useMigrationTransaction(
    address: string,
    basicOutputObjects: IotaObjectData[],
    nftOutputObjects: IotaObjectData[],
) {
    const client = useIotaClient();
    const basicOutputObjectsIds = basicOutputObjects.map(({ objectId }) => objectId);
    const nftOutputObjectsIds = nftOutputObjects.map(({ objectId }) => objectId);
    const { data: maxSizeBytes = Infinity } = useMaxTransactionSizeBytes();

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['migration-transaction', address, basicOutputObjectsIds, nftOutputObjectsIds],
        queryFn: async () => {
            const transaction = await createMigrationTransaction(
                client,
                address,
                basicOutputObjects,
                nftOutputObjects,
            );
            transaction.setSender(address);
            const txBytes = await transaction.build({ client, maxSizeBytes });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            return {
                transaction,
                txDryRun,
            };
        },
        enabled: !!address,
        gcTime: 0,
        select: ({ transaction, txDryRun }) => {
            return {
                transaction,
                gasSummary: getGasSummary(txDryRun),
            };
        },
    });
}
