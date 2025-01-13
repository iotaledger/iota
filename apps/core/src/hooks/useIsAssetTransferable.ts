// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaMoveNormalizedStruct, IotaObjectData } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

function getObjectTypeParams(obj: IotaObjectData | null | undefined) {
    const objectType =
        obj?.type ??
        (obj?.content?.dataType === 'package' ? 'package' : obj?.content?.type) ??
        null;

    return objectType?.split('<')[0]?.split('::') || [];
}

export function useIsAssetTransferable(obj: IotaObjectData | null | undefined) {
    const client = useIotaClient();
    const [packageId, moduleName, functionName] = getObjectTypeParams(obj);

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['is-asset-transferable', packageId, moduleName, functionName],
        queryFn: async () => {
            if (!packageId || !moduleName || !functionName) {
                return undefined;
            }

            return await client.getNormalizedMoveStruct({
                package: packageId,
                module: moduleName,
                struct: functionName,
            });
        },
        select: (moveNormalizedStruct: IotaMoveNormalizedStruct | undefined): boolean => {
            if (!moveNormalizedStruct) {
                return false;
            }

            const structAbilities = moveNormalizedStruct?.abilities?.abilities ?? null;

            if (!structAbilities) {
                return false;
            }

            return structAbilities.includes('Store');
        },
        enabled: !!packageId && !!moduleName && !!functionName,
    });
}
