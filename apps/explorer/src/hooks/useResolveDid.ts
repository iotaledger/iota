// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaDID, type IotaDocument } from '@iota/identity-wasm/web';
import * as React from 'react';
import { useIdentityClient } from '~/contexts';

interface DidDocumentResult {
    didDocument: IotaDocument | null;
    isPending: boolean;
}

/**
 * A React hook that resolves a DID to its corresponding DID document.
 *
 * @param {IotaDID | null} did - The DID to resolve, or null if no DID is available
 * @returns {DidDocumentResult} An object containing:
 *   - didDocument: The resolved DID document, or null if resolution is pending or failed
 *   - isPending: A boolean indicating if the resolution is still in progress
 */
export function useResolveDid(did: IotaDID | null): DidDocumentResult {
    const identityClient = useIdentityClient();
    const [didDocument, setDidDocument] = React.useState<IotaDocument | null>(null);

    React.useEffect(() => {
        const resolveDid = async () => {
            if (did && identityClient) {
                const _didDocument = await identityClient.resolveDid(did);
                setDidDocument(_didDocument);
            }
        };
        resolveDid();
    }, [did, identityClient]);

    return {
        didDocument,
        isPending: didDocument == null,
    };
}
