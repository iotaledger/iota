// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';

import { useActiveAddress } from '../../hooks';
import { useConfig } from './useConfig';

export function useBuyNLargeAsset() {
    const config = useConfig();
    const address = useActiveAddress();
    const { data } = useIotaClientQuery(
        'getOwnedObjects',
        {
            owner: address ?? '',
            filter: { StructType: config?.objectType ?? '' },
            options: { showDisplay: true, showType: true },
        },
        {
            enabled: !!address && config?.enabled,
        },
    );

    return { objectType: config?.enabled ? config?.objectType : null, asset: data?.data[0] };
}
