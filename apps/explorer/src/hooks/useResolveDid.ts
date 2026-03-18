// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaDID } from '@iota/identity-wasm/web';
import { useQuery } from '@tanstack/react-query';
import { useIdentityClient } from '~/contexts';

/**
 * A React hook that resolves a DID to its corresponding DID document.
 *
 * @param {IotaDID | null} did - The DID to resolve, or null if no DID is available
 */
export function useResolveDid(did: IotaDID | null) {
    const identityClient = useIdentityClient();
    return useQuery({
        queryKey: ['did-document', did],
        queryFn: async () => identityClient?.resolveDid(did!),
        enabled: !!(did && identityClient),
    });
}
