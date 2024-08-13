// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LargeButton } from '_app/shared/LargeButton';
import { ampli } from '_src/shared/analytics/ampli';
import { Text } from '_src/ui/app/shared/text';
import {
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { WalletActionStake24 } from '@iota/icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export function TokenIconLink({
    accountAddress,
    disabled,
}: {
    accountAddress: string;
    disabled: boolean;
}) {
    const { data: delegatedStake, isPending } = useGetDelegatedStake({
        address: accountAddress,
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    // Total active stake for all delegations
    const delegatedStakes = delegatedStake ? formatDelegatedStake(delegatedStake) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);
    const [formattedDelegatedStake, symbol, queryResultStake] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );

    return (
        <LargeButton
            to="/stake"
            spacing="sm"
            center={!totalDelegatedStake}
            disabled={disabled}
            onClick={() => {
                ampli.clickedStakeIota({
                    isCurrentlyStaking: totalDelegatedStake > 0,
                    sourceFlow: 'Home page',
                });
            }}
            loading={isPending || queryResultStake.isPending}
            before={<WalletActionStake24 />}
            data-testid={`stake-button-${formattedDelegatedStake}-${symbol}`}
        >
            <div className="flex flex-col">
                <Text variant="pBody" weight="semibold">
                    {totalDelegatedStake ? 'Currently Staked' : 'Stake and Earn IOTA'}
                </Text>

                {!!totalDelegatedStake && (
                    <Text variant="pBody" weight="semibold">
                        {formattedDelegatedStake} {symbol}
                    </Text>
                )}
            </div>
        </LargeButton>
    );
}
