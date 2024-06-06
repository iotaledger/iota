// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useNormalizedMoveModule(packageId?: string | null, moduleName?: string | null) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['normalized-module', packageId, moduleName],
        queryFn: async () =>
            await client.getNormalizedMoveModule({
                package: packageId!,
                module: moduleName!,
            }),
        enabled: !!(packageId && moduleName),
    });
}
