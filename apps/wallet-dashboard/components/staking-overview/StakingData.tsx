// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LabelTextSize, Panel, Title } from '@iota/apps-ui-kit';
import {
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export function StakingData(): JSX.Element {
    const account = useCurrentAccount();
    const { data: delegatedStakeData } = useGetDelegatedStake({
        address: account?.address || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const extendedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(extendedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(extendedStakes);
    const [formattedDelegatedStake, stakeSymbol, stakeResult] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );
    const [formattedDelegatedRewards, rewardsSymbol, rewardsResult] = useFormatCoin(
        totalDelegatedRewards,
        IOTA_TYPE_ARG,
    );

    return (
        <Panel>
            <Title title="Staking" />
            <div className="flex h-full w-full items-center gap-md p-md--rs">
                <div className="w-1/2">
                    <LabelText
                        size={LabelTextSize.Large}
                        label="Staked"
                        text={stakeResult.isPending ? '-' : `${formattedDelegatedStake}`}
                        supportingLabel={stakeSymbol}
                    />
                </div>
                <div className="w-1/2">
                    <LabelText
                        size={LabelTextSize.Large}
                        label="Earned"
                        text={`${rewardsResult.isPending ? '-' : formattedDelegatedRewards}`}
                        supportingLabel={rewardsSymbol}
                    />
                </div>
            </div>
        </Panel>
    );
}
