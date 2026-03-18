// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaDID } from '@iota/identity-wasm/web';
import { useQuery } from '@tanstack/react-query';
import { tryDecodeDidFromUrl } from '~/lib/utils/trust-framework/identity';

/**
 * A React hook that decodes a URL-encoded DID.
 *
 * This hook handles the asynchronous process of decoding a DID from its URL-encoded form.
 * It maintains state for the decoded DID and a loading indicator.
 *
 * @param {string} [encodedDid] - The URL-encoded DID string to decode. If not provided, decoding won't be attempted.
 *
 * @example
 * const { data: decodedDid, isPending } = useDecodeDidFromUrl('did-iota-5bdeea9f-0x65b1eb600b5c49828858ae1fe21aebf914f7aa56ab5afb34c78fb8e3264ad648');
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
export function useDecodeDidFromUrl(encodedDid?: string) {
    return useQuery<IotaDID | null>({
        queryKey: ['decoded-did', encodedDid],
        queryFn: () => tryDecodeDidFromUrl(encodedDid!),
        enabled: !!encodedDid,
    });
}
