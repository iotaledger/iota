// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { getGasSummary, getKioskIdFromOwnerCap, ORIGINBYTE_KIOSK_OWNER_TOKEN } from '../utils';
import { KioskTypes, useGetKioskContents } from './useGetKioskContents';
import { ORIGINBYTE_PACKAGE_ID } from './useTransferKioskItem';
import { Transaction } from '@iota/iota-sdk/transactions';
import { KioskTransaction } from '@iota/kiosk';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { Feature } from '../enums';
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
    const obPackageId = useFeatureValue(Feature.KioskOriginbytePackageId, ORIGINBYTE_PACKAGE_ID);
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

        if (kiosk.type === KioskTypes.ORIGINBYTE && objectData?.data?.data?.type) {
            const tx = new Transaction();
            const recipientKiosks = await client.getOwnedObjects({
                owner: to,
                options: { showContent: true },
                filter: { StructType: ORIGINBYTE_KIOSK_OWNER_TOKEN },
            });
            const recipientKiosk = recipientKiosks.data[0];
            const recipientKioskId = recipientKiosk ? getKioskIdFromOwnerCap(recipientKiosk) : null;

            if (recipientKioskId) {
                tx.moveCall({
                    target: `${obPackageId}::ob_kiosk::p2p_transfer`,
                    typeArguments: [objectType],
                    arguments: [
                        tx.object(kioskId),
                        tx.object(recipientKioskId),
                        tx.pure.id(objectId),
                    ],
                });
            } else {
                tx.moveCall({
                    target: `${obPackageId}::ob_kiosk::p2p_transfer_and_create_target_kiosk`,
                    typeArguments: [objectType],
                    arguments: [tx.object(kioskId), tx.pure.address(to), tx.pure.id(objectId)],
                });
            }
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
