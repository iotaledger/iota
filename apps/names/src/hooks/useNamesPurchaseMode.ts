// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

import { useIotaNamesClient } from '@/contexts';

import { queryKey } from './queryKey';

export function useNamesPurchaseMode() {
    const { iotaNamesClient } = useIotaNamesClient();
    const iotaClient = useIotaClient();
    const packageId = iotaNamesClient.getPackage('packageId', 'v1');
    const paymentsPackageId = iotaNamesClient.getPackage('paymentsPackageId', 'v1');
    const auctionPackageId = iotaNamesClient.getPackage('auctionPackageId', 'v1');
    const iotaNamesObjectId = iotaNamesClient.getPackage('iotaNamesObjectId', 'v1');

    const authKeyType = `${packageId}::iota_names::AuthKey`;

    const paymentType = `${authKeyType}<${paymentsPackageId}::payments::PaymentsAuth>`;
    const auctionType = `${authKeyType}<${auctionPackageId}::auction::AuctionAuth>`;

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [...queryKey.purchaseConfig(paymentType, auctionType)],
        queryFn: async () => {
            if (!iotaNamesClient || !iotaClient) {
                throw new Error('Missing clients');
            }

            const { data: dynamicFields } = await iotaClient.getDynamicFields({
                parentId: iotaNamesObjectId,
            });

            const fieldTypes = dynamicFields.map((field) => field.name.type);

            return {
                isPaymentAuthorized: fieldTypes.includes(paymentType),
                isAuctionAuthorized: fieldTypes.includes(auctionType),
            };
        },
        enabled: !!iotaNamesClient && !!iotaClient,
        staleTime: 10 * 60 * 1000,
        placeholderData: {
            isPaymentAuthorized: false,
            isAuctionAuthorized: false,
        },
    });
}
