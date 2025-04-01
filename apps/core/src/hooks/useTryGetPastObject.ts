// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { IotaObjectResponse } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';

const defaultOptions = {
    showType: true,
    showContent: true,
    showOwner: true,
    showPreviousTransaction: true,
    showStorageRebate: true,
    showDisplay: true,
};

export function useTryGetPastObject(objectId?: string | null, version?: number) {
    const client = useIotaClient();
    const normalizedObjId = objectId && normalizeIotaAddress(objectId);
    return useQuery({
        queryKey: ['object', normalizedObjId, version],
        queryFn: async () => {
            let result: IotaObjectResponse | undefined = undefined;

            if (version !== undefined) {
                const pastObjectResponse = await client.tryGetPastObject({
                    id: normalizedObjId!,
                    version: Number(version),
                    options: defaultOptions,
                });

                switch (pastObjectResponse?.status) {
                    case 'VersionFound':
                        result = { data: pastObjectResponse.details };
                        break;
                    case 'ObjectNotExists':
                        result = {
                            error: { object_id: pastObjectResponse.details, code: 'notExists' },
                        };
                        break;
                    case 'ObjectDeleted':
                        result = {
                            data: pastObjectResponse.details,
                            error: {
                                code: 'deleted',
                                digest: pastObjectResponse.details.digest,
                                object_id: pastObjectResponse.details.objectId,
                                version: pastObjectResponse.details.version,
                            },
                        };
                        break;
                    case 'VersionNotFound':
                        result = { error: { code: 'display', error: 'Object version not found' } };
                        break;
                    case 'VersionTooHigh':
                        result = { error: { code: 'display', error: 'Object version too high' } };
                        break;
                }
            }

            return result;
        },
        enabled: !!normalizedObjId && !!version,
    });
}
