// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LabelTextSize, Panel, Title } from '@iota/apps-ui-kit';
import {
    formatDelegatedStake,
    useFormatCoin,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
} from '@iota/core';
import { DelegatedStake } from '@iota/iota-sdk/client';
interface StakingDataProps {
    stakingData: DelegatedStake[] | undefined;
}

export function StakingData({ stakingData }: StakingDataProps) {
    const extendedStakes = stakingData ? formatDelegatedStake(stakingData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(extendedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(extendedStakes);
    const [formattedDelegatedStake, stakeSymbol, stakeResult] = useFormatCoin({
        balance: totalDelegatedStake,
    });
    const [formattedDelegatedRewards, rewardsSymbol, rewardsResult] = useFormatCoin({
        balance: totalDelegatedRewards,
    });

    return (
        <Panel>
            <Title title="Staking" />
            <div className="flex h-full w-full items-center gap-md p-md--rs">
                <div className="w-1/2 text-primary-80">
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
