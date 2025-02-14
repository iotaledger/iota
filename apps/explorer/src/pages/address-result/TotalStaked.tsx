// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedStake,
} from '@iota/core';
import { DisplayStats } from '@iota/apps-ui-kit';

export function TotalStaked({ address }: { address: string }): JSX.Element | null {
    const { data: delegatedStake } = useGetDelegatedStake({
        address,
    });

    const delegatedStakes = delegatedStake ? formatDelegatedStake(delegatedStake) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);
    const [formattedDelegatedStake, symbol, queryResultStake] = useFormatCoin({
        balance: totalDelegatedStake,
    });

    return totalDelegatedStake ? (
        <DisplayStats
            label="Staking"
            value={queryResultStake.isPending ? '-' : `${formattedDelegatedStake}`}
            supportingLabel={symbol}
        />
    ) : null;
}
