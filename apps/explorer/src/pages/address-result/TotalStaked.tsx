// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedStake,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Text, Heading } from '@iota/ui';
import { Iota } from '@iota/icons';

export function TotalStaked({ address }: { address: string }): JSX.Element | null {
    const { data: delegatedStake } = useGetDelegatedStake({
        address,
    });

    const delegatedStakes = delegatedStake ? formatDelegatedStake(delegatedStake) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);
    const [formattedDelegatedStake, symbol, queryResultStake] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );

    return totalDelegatedStake ? (
        <div className="flex min-w-44 items-center justify-start gap-3 rounded-xl bg-white/60 px-4 py-3 backdrop-blur-sm">
            <Iota className="flex h-8 w-8 items-center justify-center rounded-full bg-iota-primaryBlue2023 py-1.5 text-white" />
            <div className="flex flex-col">
                <Text variant="pBody/semibold" color="steel-dark" uppercase>
                    Staking
                </Text>
                <Heading variant="heading6/semibold" color="hero-darkest" as="div">
                    {queryResultStake.isPending ? '-' : `${formattedDelegatedStake} ${symbol}`}
                </Heading>
            </div>
        </div>
    ) : null;
}
