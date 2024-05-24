// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSuiClient } from '@mysten/dapp-kit';
import { CoinBalance } from '@mysten/sui.js/client';
import { MIST_PER_SUI } from '@mysten/sui.js/utils';
import { useQuery } from '@tanstack/react-query';

interface UseBalance extends CoinBalance {
    suiBalance: number;
}

export function useBalance(coinType: string, address?: string | null) {
    const client = useSuiClient();

    return useQuery<UseBalance>({
        queryKey: ['get-balance', address, coinType],
        queryFn: async () => {
            const data = await client.getBalance({
                owner: address!,
                coinType,
            });

            return {
                suiBalance: Number(data.totalBalance) / Number(MIST_PER_SUI),
                ...data,
            };
        },
        enabled: !!address,
    });
}
