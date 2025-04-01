// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useGetObject } from './useGetObject';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import { IotaObjectResponse } from '@iota/iota-sdk/client';

const defaultGetPastObjectOptions = {
    showType: true,
    showContent: true,
    showOwner: true,
    showPreviousTransaction: true,
    showStorageRebate: true,
    showDisplay: true,
};

export function useGetObjectOrPastObject(objectId?: string | null, version?: string) {
    const normalizedObjId = objectId && normalizeIotaAddress(objectId);
    const client = useIotaClient();
    const {
        data: getObjectResponse,
        isPending: isPendginGetObject,
        isError: isErrorGetObject,
        isFetched: isFetchedGetObject,
    } = useGetObject(normalizedObjId);

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
            options: defaultGetPastObjectOptions,
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

    const isNotExistsOrDeleted =
        getObjectResponse?.error?.code === 'notExists' ||
        getObjectResponse?.error?.code === 'deleted';
    const shouldTryGetPastObject = version !== undefined && isNotExistsOrDeleted;

    const objectOrPastObjectResponse = useQuery({
        queryKey: ['objectOrPastObject', normalizedObjId, version, shouldTryGetPastObject],
        queryFn: async () => {
            if (shouldTryGetPastObject) {
                return tryGetPastObject(normalizedObjId!, Number(version));
            } else {
                return getObjectResponse;
            }
        },
        enabled: !!normalizedObjId,
    });

    const isDeletedVersion =
        shouldTryGetPastObject && objectOrPastObjectResponse?.data?.error?.code === 'deleted';

    return {
        data: objectOrPastObjectResponse.data,
        isPending: isPendginGetObject || objectOrPastObjectResponse.isPending,
        isError: isErrorGetObject || objectOrPastObjectResponse.isError,
        isFetched: isFetchedGetObject || objectOrPastObjectResponse.isFetched,
        isViewingPastVersion: shouldTryGetPastObject,
        isDeletedVersion,
    };
}
