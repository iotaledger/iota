// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
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

export function useGetObject(objectId?: string | null) {
    const client = useIotaClient();
    const normalizedObjId = objectId && normalizeIotaAddress(objectId);
    return useQuery({
        queryKey: ['object', normalizedObjId],
        queryFn: () =>
            client.getObject({
                id: normalizedObjId!,
                options: defaultOptions,
            }),
        enabled: !!normalizedObjId,
    });
}
