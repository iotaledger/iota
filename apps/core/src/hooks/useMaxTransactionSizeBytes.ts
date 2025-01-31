// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export const MAX_SIZE_BYTES_ERROR =
    'Attempting to serialize to BCS, but buffer does not have enough size';

export function useMaxTransactionSizeBytes() {
    const client = useIotaClient();

    return useQuery({
        queryKey: ['protocol-config-max-tx-size-bytes'],
        queryFn: async () => {
            const config = await client.getProtocolConfig();
            const max_tx_size_bytes = config.attributes['max_tx_size_bytes'];
            return max_tx_size_bytes && 'u64' in max_tx_size_bytes
                ? Number(max_tx_size_bytes.u64)
                : Infinity;
        },
        enabled: !!client,
    });
}
