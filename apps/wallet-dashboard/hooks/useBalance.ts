// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { CoinBalance } from '@iota/iota.js/client';
import { MICROS_PER_IOTA } from '@iota/iota.js/utils';
import { useQuery } from '@tanstack/react-query';

interface UseBalance extends CoinBalance {
    iotaBalance: number;
}

export function useBalance(coinType: string, address?: string | null) {
    const client = useIotaClient();

    return useQuery<UseBalance>({
        queryKey: ['get-balance', address, coinType],
        queryFn: async () => {
            const data = await client.getBalance({
                owner: address!,
                coinType,
            });

            return {
                iotaBalance: Number(data.totalBalance) / Number(MICROS_PER_IOTA),
                ...data,
            };
        },
        enabled: !!address,
    });
}
