// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetObject } from './useGetObject';
import { useTryGetPastObject } from './useTryGetPastObject';

export function useGetObjectOrPastObject(objectId?: string | null, version?: string) {
    const {
        data: getObjectResponse,
        isPending: isPendginGetObject,
        isError: isErrorGetObject,
        isFetched: isFetchedGetObject,
    } = useGetObject(objectId);

    const shouldTryGetPastObject =
        version !== undefined &&
        (getObjectResponse?.error?.code === 'notExists' ||
            getObjectResponse?.error?.code === 'deleted');

    const {
        data: getPastObjectResponse,
        isPending: isPendingTryGetPastObject,
        isError: isErrorTryGetPastObject,
        isFetched: isFetchedTryGetPastObject,
        // We get the (deletedVersion - 1) to see the data on the object
    } = useTryGetPastObject(objectId, shouldTryGetPastObject ? Number(version) - 1 : undefined);

    return !shouldTryGetPastObject
        ? {
              data: getObjectResponse,
              isPending: isPendginGetObject,
              isError: isErrorGetObject,
              isFetched: isFetchedGetObject,
              isViewingPastVersion: false,
              isDeletedVersion: false,
          }
        : {
              data: getPastObjectResponse,
              isPending: isPendingTryGetPastObject,
              isError: isErrorTryGetPastObject,
              isFetched: isFetchedTryGetPastObject,
              isViewingPastVersion: true,
              isDeletedVersion: getPastObjectResponse?.error?.code === 'deleted',
          };
}
