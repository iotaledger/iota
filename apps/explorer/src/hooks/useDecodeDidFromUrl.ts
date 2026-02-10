// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaDID } from '@iota/identity-wasm/web';
import * as React from 'react';
import { tryDecodeDidFromUrl } from '~/lib/utils/trust-framework/identity';

interface DecodeDIDResult {
    decodedDid: IotaDID | null;
    isPending: boolean;
}

/**
 * A React hook that decodes a URL-encoded DID.
 *
 * This hook handles the asynchronous process of decoding a DID from its URL-encoded form.
 * It maintains state for the decoded DID and a loading indicator.
 *
 * @param {string} [encodedDid] - The URL-encoded DID string to decode. If not provided, decoding won't be attempted.
 * @returns {DecodeDIDResult} An object containing:
 *   - decodedDid: The decoded IOTA DID, or null if decoding failed or wasn't attempted
 *   - isPending: True while decoding is in progress, false when completed
 *
 * @example
 * const { decodedDid, isPending } = useDecodeDidFromUrl('did-iota-5bdeea9f-0x65b1eb600b5c49828858ae1fe21aebf914f7aa56ab5afb34c78fb8e3264ad648');
 *
 * if (isPending) {
 *   return <LoadingIndicator />;
 * }
 *
 * if (!decodedDid) {
 *   return <Error message="Failed to decode DID" />;
 * }
 *
 * return <DisplayDid did={decodedDid} />;
 */
export function useDecodeDidFromUrl(encodedDid?: string): DecodeDIDResult {
    const [decodedDid, setDecodedDid] = React.useState<IotaDID | null>(null);
    const [isPending, setIsPending] = React.useState(true);
    React.useEffect(() => {
        const decode = async () => {
            if (encodedDid) {
                const _decodedDid = await tryDecodeDidFromUrl(encodedDid);
                if (_decodedDid) {
                    setDecodedDid(_decodedDid);
                }
                setIsPending(false);
            }
        };
        decode();
    }, [encodedDid]);

    return {
        decodedDid,
        isPending,
    };
}
