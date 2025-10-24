// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { getGasSummary } from '../utils';
import { KioskTypes, useGetKioskContents } from './useGetKioskContents';
import { Transaction } from '@iota/iota-sdk/transactions';
import { KioskTransaction } from '@iota/kiosk';
import { useKioskClient } from './useKioskClient';
import { useGetObject } from './useGetObject';

interface UseAssetGasBudgetEstimationOptions {
    objectId: string;
    objectType?: string | null;
    activeAddress?: string | null;
    to?: string | null;
}

export function useAssetGasBudgetEstimation({
    objectId,
    objectType,
    activeAddress,
    to,
}: UseAssetGasBudgetEstimationOptions) {
    const client = useIotaClient();
    const { data: kioskData } = useGetKioskContents(activeAddress); // show personal kiosks too
    const objectData = useGetObject(objectId);
    const kioskClient = useKioskClient();

    const isContainedInKiosk = kioskData?.list.some(
        (kioskItem) => kioskItem.data?.objectId === objectId,
    );

    const calculateKioskTransferGasBudget = async (to: string) => {
        if (!to || !activeAddress || !objectType) {
            throw new Error('Missing data');
        }

        const kioskId = kioskData?.lookup.get(objectId);
        const kiosk = kioskData?.kiosks.get(kioskId!);

        if (!kioskId || !kiosk) {
            throw new Error('Failed to find object in a kiosk');
        }

        if (kiosk.type === KioskTypes.IOTA && objectData?.data?.data?.type && kiosk?.ownerCap) {
            const tx = new Transaction();

            new KioskTransaction({ transaction: tx, kioskClient, cap: kiosk.ownerCap })
                .transfer({
                    itemType: objectData.data.data.type as string,
                    itemId: objectId,
                    address: to,
                })
                .finalize();

            tx.setSender(activeAddress);
            return await calculateGasFee(tx);
        }
    };

    const calculateDirectAssetTransfer = async (to: string) => {
        if (!to || !activeAddress) {
            throw new Error('Missing data');
        }
        const tx = new Transaction();
        tx.transferObjects([tx.object(objectId)], to);
        tx.setSender(activeAddress);
        return await calculateGasFee(tx);
    };

    const calculateGasFee = async (transaction: Transaction) => {
        const txBytes = await transaction.build({ client });
        const txDryRun = await client.dryRunTransactionBlock({
            transactionBlock: txBytes,
        });
        const gasSummary = getGasSummary(txDryRun);
        return gasSummary?.totalGas ?? transaction.getData().gasData.budget;
    };

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'asset-transaction-gas-budget-estimate',
            {
                objectId,
                objectType,
                activeAddress,
                to,
            },
        ],
        queryFn: async () => {
            if (!objectId || !objectType || !activeAddress || !to) {
                return null;
            }

            return isContainedInKiosk
                ? calculateKioskTransferGasBudget(to)
                : calculateDirectAssetTransfer(to);
        },
    });
}
