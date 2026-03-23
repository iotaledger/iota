// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type OnChainNotarization } from '@iota/notarization/web';
import { useQuery, type UseQueryResult } from '@tanstack/react-query';
import { useNotarizationClient } from '~/contexts';

/**
 * A React hook that resolves an Object ID to its corresponding Notarization document on chain.
 *
 * @param {string} objectId - The Object ID to resolve.
 * @returns a Notarization document on chain.
 */
export function useResolveNotarization(objectId: string): UseQueryResult<OnChainNotarization> {
    const notarizationClient = useNotarizationClient();
    return useQuery({
        queryKey: ['resolve-notarization', objectId],
        queryFn: async () => notarizationClient?.getNotarizationById(objectId),
        enabled: !!notarizationClient,
    });
}
