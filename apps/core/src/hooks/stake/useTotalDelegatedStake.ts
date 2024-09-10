// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { type ExtendedDelegatedStake } from '../../utils/stake';

export function useTotalDelegatedStake(delegatedStake: ExtendedDelegatedStake[]) {
    return useMemo(() => {
        if (!delegatedStake) return 0n;
        return delegatedStake.reduce((acc, curr) => acc + BigInt(curr.principal), 0n);
    }, [delegatedStake]);
}
