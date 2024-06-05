// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaParsedData } from '@iota/iota.js/client';
import { useMemo } from 'react';

export const parseIpfsUrl = (ipfsUrl: string) =>
    ipfsUrl.replace(/^ipfs:\/\//, 'https://ipfs.io/ipfs/');

export default function useMediaUrl(objData: IotaParsedData | null) {
    const { fields } =
        ((objData?.dataType === 'moveObject' && objData) as {
            fields: { url?: string; metadata?: { fields: { url: string } } };
        }) || {};
    return useMemo(() => {
        if (fields) {
            const mediaUrl = fields.url || fields.metadata?.fields.url;
            if (typeof mediaUrl === 'string') {
                return parseIpfsUrl(mediaUrl);
            }
        }
        return null;
    }, [fields]);
}
