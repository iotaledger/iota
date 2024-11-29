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
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
interface StakingDataProps {
    stakingData: DelegatedStake[] | undefined;
}

export function StakingData({ stakingData }: StakingDataProps) {
    const extendedStakes = stakingData ? formatDelegatedStake(stakingData) : [];
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
                <div className="w-1/2 text-primary-80">
                    <LabelText
                        size={LabelTextSize.Large}
                        label="Staked LAna"
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
