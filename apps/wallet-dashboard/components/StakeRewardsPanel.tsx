// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import {
    TimeUnit,
    useFormatCoin,
    useGetLatestIotaSystemState,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
} from '@iota/core';

interface StakeRewardsPanelProps {
    stakingRewards: string | number | undefined;
    totalStaked: number | bigint;
    isTimelocked?: boolean;
}
export function StakeRewardsPanel({
    stakingRewards,
    totalStaked,
    isTimelocked,
}: StakeRewardsPanelProps) {
    const { epoch = '0' } = useGetLatestIotaSystemState()?.data || {};
    const [rewards, symbol] = useFormatCoin({ balance: stakingRewards ?? 0 });
    const [stakedBalance] = useFormatCoin({ balance: totalStaked });
    const [stakedAndRewards] = useFormatCoin({
        balance: BigInt(stakingRewards || 0) + BigInt(totalStaked),
    });

    const { data: currentEpochEndTime } = useGetTimeBeforeEpochNumber(Number(epoch) + 1);
    const currentEpochEndTimeAgo = useTimeAgo({
        timeFrom: currentEpochEndTime,
        endLabel: '--',
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const currentEpochEndTimeFormatted =
        currentEpochEndTime > 0 ? currentEpochEndTimeAgo : `Epoch #${epoch}`;

    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm p-md">
                <KeyValueInfo
                    keyText="Current Epoch Ends"
                    value={currentEpochEndTimeFormatted}
                    fullwidth
                />
                <Divider />
                <KeyValueInfo
                    keyText="Your Stake"
                    value={stakedBalance}
                    supportingLabel={symbol}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Rewards Earned"
                    value={rewards}
                    supportingLabel={symbol}
                    fullwidth
                />
                <Divider />
                <KeyValueInfo
                    keyText={'Total unstaked ' + (isTimelocked ? 'Timelocked' : 'IOTA')}
                    value={stakedAndRewards}
                    supportingLabel={symbol}
                    fullwidth
                />
            </div>
        </Panel>
    );
}
