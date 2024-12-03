// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Card, CardImage, CardType, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { useMemo } from 'react';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { ImageIcon } from '../icon';
import { determineCountDownText, ExtendedDelegatedStake } from '../../utils';
import { TimeUnit, useFormatCoin, useGetTimeBeforeEpochNumber, useTimeAgo } from '../../hooks';
import { NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE } from '../../constants';
import React from 'react';

export enum StakeState {
    WarmUp = 'WARM_UP',
    Earning = 'EARNING',
    CoolDown = 'COOL_DOWN',
    Withdraw = 'WITHDRAW',
    InActive = 'IN_ACTIVE',
}

const STATUS_COPY: { [key in StakeState]: string } = {
    [StakeState.WarmUp]: 'Starts Earning',
    [StakeState.Earning]: 'Staking Rewards',
    [StakeState.CoolDown]: 'Available to withdraw',
    [StakeState.Withdraw]: 'Withdraw',
    [StakeState.InActive]: 'Inactive',
};

interface StakedCardProps {
    extendedStake: ExtendedDelegatedStake;
    currentEpoch: number;
    inactiveValidator?: boolean;
    onClick: () => void;
}

// For delegationsRequestEpoch n  through n + 2, show Start Earning
// Show epoch number or date/time for n + 3 epochs
export function StakedCard({
    extendedStake,
    currentEpoch,
    inactiveValidator = false,
    onClick,
}: StakedCardProps) {
    const { principal, stakeRequestEpoch, estimatedReward, validatorAddress } = extendedStake;

    // TODO: Once two step withdraw is available, add cool down and withdraw now logic
    // For cool down epoch, show Available to withdraw add rewards to principal
    // Reward earning epoch is 2 epochs after stake request epoch
    const earningRewardsEpoch =
        Number(stakeRequestEpoch) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;
    const isEarnedRewards = currentEpoch >= Number(earningRewardsEpoch);
    const delegationState = inactiveValidator
        ? StakeState.InActive
        : isEarnedRewards
          ? StakeState.Earning
          : StakeState.WarmUp;

    const rewards = isEarnedRewards && estimatedReward ? BigInt(estimatedReward) : 0n;

    // For inactive validator, show principal + rewards
    const [principalStaked, symbol] = useFormatCoin(
        inactiveValidator ? principal + rewards : principal,
        IOTA_TYPE_ARG,
    );
    const [rewardsStaked] = useFormatCoin(rewards, IOTA_TYPE_ARG);

    // Applicable only for warm up
    const epochBeforeRewards = delegationState === StakeState.WarmUp ? earningRewardsEpoch : null;

    const statusText = {
        // Epoch time before earning
        [StakeState.WarmUp]: `Epoch #${earningRewardsEpoch}`,
        [StakeState.Earning]: `${rewardsStaked} ${symbol}`,
        // Epoch time before redrawing
        [StakeState.CoolDown]: `Epoch #`,
        [StakeState.Withdraw]: 'Now',
        [StakeState.InActive]: 'Not earning rewards',
    };

    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: rewardEpochTime } = useGetTimeBeforeEpochNumber(Number(epochBeforeRewards) || 0);
    const timeAgo = useTimeAgo({
        timeFrom: rewardEpochTime || null,
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const validatorMeta = useMemo(() => {
        if (!data) return null;

        return (
            data.activeValidators.find((validator) => validator.iotaAddress === validatorAddress) ||
            null
        );
    }, [validatorAddress, data]);

    const rewardTime = () => {
        if (Number(epochBeforeRewards) && rewardEpochTime > 0) {
            return determineCountDownText({
                timeAgo,
                label: 'in',
            });
        }

        return statusText[delegationState];
    };

    return (
        <Card testId="staked-card" type={CardType.Default} isHoverable onClick={onClick}>
            <CardImage>
                <ImageIcon
                    src={validatorMeta?.imageUrl || null}
                    label={validatorMeta?.name || ''}
                    fallback={validatorMeta?.name || ''}
                />
            </CardImage>
            <CardBody title={validatorMeta?.name || ''} subtitle={`${principalStaked} ${symbol}`} />
            <CardAction
                title={rewardTime()}
                subtitle={STATUS_COPY[delegationState]}
                type={CardActionType.SupportingText}
            />
        </Card>
    );
}
