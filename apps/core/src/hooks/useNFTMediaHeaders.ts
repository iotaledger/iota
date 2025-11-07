// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery, UseQueryResult } from '@tanstack/react-query';
import { transformURL } from '../utils';

export interface NFTMediaHeadersResult {
    contentType: string | null;
    contentLength: string | null;
}

export function useNFTMediaHeaders(
    src: string | null | undefined,
): UseQueryResult<NFTMediaHeadersResult> {
    return useQuery({
        queryKey: ['nft-media-headers', src],
        queryFn: async ({ signal }) => {
            if (!src) {
                return { contentType: null, contentLength: null };
            }

            try {
                const res = await fetch(transformURL(src), { signal });
                return {
                    contentType: res.headers.get('Content-Type'),
                    contentLength: res.headers.get('Content-Length'),
                };
            } catch (_) {
                return { contentType: null, contentLength: null };
            }
        },
        enabled: !!src,
        refetchOnWindowFocus: false,
        staleTime: 10 * 60 * 1000,
    });
}
