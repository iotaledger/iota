// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

import { queryKey } from './queryKey';

export function useIsMethodSupported({
    packageId,
    module,
    functionName,
}: {
    packageId: string;
    module: string;
    functionName: string;
}) {
    const client = useIotaClient();

    return useQuery({
        queryKey: [...queryKey.methodSupported(packageId, module, functionName)],
        async queryFn() {
            try {
                const result = await client.getNormalizedMoveFunction({
                    package: packageId,
                    module,
                    function: functionName,
                });
                return result != null;
            } catch {
                return false;
            }
        },
    });
}
