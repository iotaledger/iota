// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useQuery, UseQueryResult } from '@tanstack/react-query';
import { IotaObjectResponse } from '@iota/iota-sdk/client';

const DEFAULT_GET_OBJECT_OPTIONS = {
    showType: true,
    showContent: true,
    showOwner: true,
    showPreviousTransaction: true,
    showStorageRebate: true,
    showDisplay: true,
};

interface UseGetObjectOrPastObject extends IotaObjectResponse {
    isViewingPastVersion: boolean;
}

export function useGetObjectOrPastObject(
    objectId?: string | null,
): UseQueryResult<UseGetObjectOrPastObject> {
    const normalizedObjId = objectId && normalizeIotaAddress(objectId);
    const client = useIotaClient();
    return useQuery({
        queryKey: ['object-or-past-object', normalizedObjId],
        async queryFn() {
            if (!normalizedObjId) {
                return null;
            }

            const getObjectResponse = await client.getObject({
                id: normalizedObjId,
                options: DEFAULT_GET_OBJECT_OPTIONS,
            });

            const shouldTryFindPastVersion =
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

                        // Check for coin object also
                        if (previousVersion === null) {
                            for (const paymentObject of previousTxData.gasData.payment) {
                                if (paymentObject.objectId === objectId) {
                                    previousVersion = Number(paymentObject.version);
                                    break;
                                }
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
                    options: DEFAULT_GET_OBJECT_OPTIONS,
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

            const iotaObjectResponse = shouldTryFindPastVersion
                ? await tryFindPastVersionOfObject(normalizedObjId)
                : getObjectResponse;

            const isViewingPastVersion = shouldTryFindPastVersion && iotaObjectResponse.data;

            return {
                ...iotaObjectResponse,
                isViewingPastVersion,
            };
        },
        enabled: !!normalizedObjId,
    });
}
