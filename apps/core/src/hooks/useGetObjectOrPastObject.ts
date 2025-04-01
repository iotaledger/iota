// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useQuery, UseQueryResult } from '@tanstack/react-query';
import { IotaObjectResponse } from '@iota/iota-sdk/client';

const defaultGetObjectOptions = {
    showType: true,
    showContent: true,
    showOwner: true,
    showPreviousTransaction: true,
    showStorageRebate: true,
    showDisplay: true,
};

interface UseGetObjectOrPastObject extends IotaObjectResponse {
    isDeletedVersion: boolean;
    isViewingPastVersion: boolean;
}

export function useGetObjectOrPastObject(
    objectId?: string | null,
    pastVersionHint?: string,
): UseQueryResult<UseGetObjectOrPastObject> {
    const normalizedObjId = objectId && normalizeIotaAddress(objectId);
    const client = useIotaClient();
    return useQuery({
        queryKey: ['objectOrPastObject', normalizedObjId, pastVersionHint],
        async queryFn() {
            if (!normalizedObjId) {
                return null;
            }

            const getObjectResponse = await client.getObject({
                id: normalizedObjId,
                options: defaultGetObjectOptions,
            });

            const isNotExistsOrDeleted =
                getObjectResponse?.error?.code === 'notExists' ||
                getObjectResponse?.error?.code === 'deleted';
            const shouldTryGetPastObject = pastVersionHint !== undefined && isNotExistsOrDeleted;

            /**
             * Calls tryGetPastObject and maps cases to a IotaObjectResponse
             */
            const tryGetPastObject = async (
                objectId: string,
                version: number,
            ): Promise<IotaObjectResponse> => {
                // We get the (deletedVersion - 1) to see the data on the object
                const pastObjectResponse = await client.tryGetPastObject({
                    id: objectId,
                    version: Number(version) - 1,
                    options: defaultGetObjectOptions,
                });

                switch (pastObjectResponse?.status) {
                    case 'VersionFound':
                        return { data: pastObjectResponse.details };
                    case 'ObjectNotExists':
                        return {
                            error: { object_id: pastObjectResponse.details, code: 'notExists' },
                        };
                    case 'ObjectDeleted':
                        return {
                            data: pastObjectResponse.details,
                            error: {
                                code: 'deleted',
                                digest: pastObjectResponse.details.digest,
                                object_id: pastObjectResponse.details.objectId,
                                version: pastObjectResponse.details.version,
                            },
                        };
                    case 'VersionNotFound':
                        return { error: { code: 'display', error: 'Object version not found' } };
                    case 'VersionTooHigh':
                        return { error: { code: 'display', error: 'Object version too high' } };
                }
            };

            const iotaObjectResponse = shouldTryGetPastObject
                ? await tryGetPastObject(normalizedObjId, Number(pastVersionHint))
                : getObjectResponse;

            const isViewingPastVersion = shouldTryGetPastObject;
            const isDeletedVersion =
                shouldTryGetPastObject && iotaObjectResponse?.error?.code === 'deleted';
            return {
                ...iotaObjectResponse,
                isDeletedVersion,
                isViewingPastVersion,
            };
        },
        enabled: !!normalizedObjId,
    });
}
