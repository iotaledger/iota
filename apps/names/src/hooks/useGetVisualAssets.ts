// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNameRegistrationType, getSubnameRegistrationType } from '@iota/iota-names-sdk';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { useIotaNamesClient } from '@/contexts';
import { isKioskOwnerToken } from '@/lib/utils/kiosk';

import { useGetAllOwnedObjects } from './useGetAllOwnedObjects';
import { useKioskClient } from './useKioskClient';

const FILTER_NONE_STRUCT_TYPES = [
    '0x2::coin::Coin',
    '0x3::staking_pool::StakedIota',
    IOTA_TYPE_ARG,
    '0x3::timelocked_staking::TimelockedStakedIota',
    '0x2::timelock::TimeLock<0x2::balance::Balance<0x2::iota::IOTA>>',
];

export function useGetVisualAssets(address: string) {
    const kioskClient = useKioskClient();
    const { iotaNamesClient } = useIotaNamesClient();
    const packageId = iotaNamesClient.getPackage('packageId', 'v1');

    const iotaNamesStructsToRemove = [
        getNameRegistrationType(packageId),
        getSubnameRegistrationType(packageId),
    ];

    return useGetAllOwnedObjects<IotaObjectData[]>(
        address,
        {
            MatchNone: [...FILTER_NONE_STRUCT_TYPES, ...iotaNamesStructsToRemove].map((type) => ({
                StructType: type,
            })),
        },
        {
            select: (data) => getAllVisualAssets(data, kioskClient),
        },
    );
}

function getAllVisualAssets(
    ownedObjects: IotaObjectData[],
    kioskClient: ReturnType<typeof useKioskClient>,
) {
    const visualAssets = ownedObjects?.filter((obj) => !!obj.display?.data) ?? [];

    const kioskAssets =
        ownedObjects?.reduce((acc, curr) => {
            if (isKioskOwnerToken(kioskClient.network, curr)) acc.push(curr);
            return acc;
        }, [] as IotaObjectData[]) ?? [];

    return [...visualAssets, ...kioskAssets];
}
