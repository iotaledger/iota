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
): UseQueryResult<UseGetObjectOrPastObject> {
    const normalizedObjId = objectId && normalizeIotaAddress(objectId);
    const client = useIotaClient();
    return useQuery({
        queryKey: ['objectOrPastObject', normalizedObjId],
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

            /**
             * Calls tryGetPastObject and maps cases to a IotaObjectResponse
             */
            const tryFindPastVersionOfObject = async (
                objectId: string,
            ): Promise<IotaObjectResponse> => {
                const txsWithObjectInput = await client.queryTransactionBlocks({
                    filter: { InputObject: objectId },
                    options: {
                        showInput: true,
                    },
                });

                let previousVersion: number | null = null;

                if (txsWithObjectInput?.data.length > 0) {
                    const previousTxData = txsWithObjectInput.data[0].transaction?.data;
                    if (previousTxData?.transaction.kind === 'ProgrammableTransaction') {
                        for (const input of previousTxData.transaction.inputs) {
                            if (
                                input.type === 'object' &&
                                // Only works for immOrOwnedObject and receiving object types
                                (input.objectType === 'immOrOwnedObject' ||
                                    input.objectType === 'receiving') &&
                                input.objectId === objectId
                            ) {
                                previousVersion = Number(input.version);
                                break;
                            }
                        }
                    }
                }

                if (previousVersion === null) {
                    return {
                        error: { code: 'display', error: 'Object version not found' },
                    };
                }

                const pastObjectResponse = await client.tryGetPastObject({
                    id: objectId,
                    version: previousVersion,
                    options: defaultGetObjectOptions,
                });

                switch (pastObjectResponse?.status) {
                    case 'VersionFound':
                        return { data: pastObjectResponse.details };
                    default:
                        return {
                            error: { code: 'display', error: 'Object version not found' },
                        };
                }
            };

            const iotaObjectResponse = isNotExistsOrDeleted
                ? await tryFindPastVersionOfObject(normalizedObjId)
                : getObjectResponse;

            const isViewingPastVersion = isNotExistsOrDeleted;
            const isDeletedVersion =
                isNotExistsOrDeleted && iotaObjectResponse?.error?.code === 'deleted';
            return {
                ...iotaObjectResponse,
                isDeletedVersion,
                isViewingPastVersion,
            };
        },
        enabled: !!normalizedObjId,
    });
}
