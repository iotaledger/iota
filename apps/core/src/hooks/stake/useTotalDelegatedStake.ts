// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { type ExtendedDelegatedStake } from '../../utils/stake';
import { ExtendedDelegatedTimelockedStake } from '../../interfaces';

export function useTotalDelegatedStake(
    delegatedStakes: ExtendedDelegatedStake[] | ExtendedDelegatedTimelockedStake[],
) {
    return useMemo(() => {
        if (!delegatedStakes) return 0n;
        return delegatedStakes.reduce((acc, curr) => acc + BigInt(curr.principal), 0n);
    }, [delegatedStakes]);
}
