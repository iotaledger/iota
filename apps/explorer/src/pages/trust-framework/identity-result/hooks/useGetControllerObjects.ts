// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getObjectOrPastObjectQuery } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import type { IotaObjectData } from '@iota/iota-sdk/src/client';
import { useQueries } from '@tanstack/react-query';
import { extractControllerCaps, getOwnerAddress, getOwnerType } from '../helper';
import type { IdentityController } from '../types';

/**
 * This hook:
 * 1. Extracts controller capabilities from the input object data
 * 2. Fetches the corresponding controller objects using parallel queries
 * 3. Processes the fetched data into standardized IdentityController objects
 * 4. Handles errors at both the query level and individual controller level
 *
 * @param {IotaObjectData} objectData - The IOTA object data containing controller capabilities
 * @returns An object containing:
 *   - controllers: Array of IdentityController objects with details about each controller
 *   - isPending: Boolean indicating if the data fetching is in progress
 *   - isError: Boolean indicating if error occurred in all queries during fetching
 */
export function useGetControllerObjects(objectData: IotaObjectData) {
    const client = useIotaClient();
    const controllerCaps = extractControllerCaps(objectData);
    return useQueries({
        queries: controllerCaps.map((controllerCap) =>
            getObjectOrPastObjectQuery<IdentityController>(
                client,
                controllerCap.objectId,
                (objectResponse) => {
                    // Transforms a controller object to IdentityController for each query
                    if (objectResponse?.error) {
                        return {
                            ...controllerCap,
                            isError: true,
                            error: objectResponse.error,
                        };
                    }

                    const objectData = objectResponse?.data;
                    return {
                        ...controllerCap,
                        isError: false,
                        objectType: objectData?.type,
                        owner: getOwnerAddress(objectData?.owner, objectData?.objectId),
                        ownerType: getOwnerType(objectData?.owner),
                    };
                },
            ),
        ),
        combine: (results) => ({
            results,
            isPending: results.some((result) => result.isPending),
            isError: results.every((result) => result.isError),
        }),
    });
}
