// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getObjectOrPastObjectQuery } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import type { IotaObjectData } from '@iota/iota-sdk/src/client';
import { useQueries } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import type { IdentityController } from '../types';
import { extractControllerCaps, getOwnerAddress, getOwnerType } from '../helper';

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
    const [controllers, setControllers] = useState<IdentityController[]>([]);
    const controllerCaps = extractControllerCaps(objectData);
    const {
        results: controllerObjectResults,
        isPending,
        isError,
    } = useQueries({
        queries: controllerCaps.map((controllerCap) =>
            getObjectOrPastObjectQuery(client, controllerCap.objectId),
        ),
        combine: (results) => ({
            results,
            isPending: results.some((result) => result.isPending),
            isError: results.every((result) => result.isError),
        }),
    });

    useEffect(() => {
        if (!isPending && !isError) {
            const controllers: IdentityController[] = controllerCaps.map((controllerCap, index) => {
                const objectResult = controllerObjectResults.at(index)!;
                if (objectResult.isError) {
                    return {
                        ...controllerCap,
                        isError: objectResult.isError,
                        error: objectResult.error,
                    };
                }

                const objectData = objectResult.data?.data;
                return {
                    ...controllerCap,
                    objectType: objectData?.type,
                    owner: getOwnerAddress(objectData?.owner, objectData?.objectId),
                    ownerType: getOwnerType(objectData?.owner),
                    isError: false,
                };
            });
            setControllers(controllers);
        }
    }, [controllerObjectResults, isPending, isError]);

    return {
        controllers,
        isPending,
        isError,
    };
}
