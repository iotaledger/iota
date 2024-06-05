// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, useGetDelegatedStake } from '@iota/core';
import { useMemo } from 'react';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { Text, Heading } from '@iota/ui';
import { Iota } from '@iota/icons';

export function TotalStaked({ address }: { address: string }) {
    const { data: delegatedStake } = useGetDelegatedStake({
        address,
    });

    // Total active stake for all delegations
    const totalActivePendingStake = useMemo(() => {
        if (!delegatedStake) return 0n;
        return delegatedStake.reduce(
            (acc, curr) =>
                curr.stakes.reduce((total, { principal }) => total + BigInt(principal), acc),
            0n,
        );
    }, [delegatedStake]);

    const [formatted, symbol] = useFormatCoin(totalActivePendingStake, IOTA_TYPE_ARG);
    return totalActivePendingStake ? (
        <div className="flex min-w-44 items-center justify-start gap-3 rounded-xl bg-white/60 px-4 py-3 backdrop-blur-sm">
            <Iota className="flex h-8 w-8 items-center justify-center rounded-full bg-iota-primaryBlue2023 py-1.5 text-white" />
            <div className="flex flex-col">
                <Text variant="pBody/semibold" color="steel-dark" uppercase>
                    Staking
                </Text>
                <Heading variant="heading6/semibold" color="hero-darkest" as="div">
                    {formatted} {symbol}
                </Heading>
            </div>
        </div>
    ) : null;
}
