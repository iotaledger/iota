// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE } from '_src/shared/constants';
import { determineCountDownText } from '_src/ui/app/shared/countdown-timer';
import {
    type ExtendedDelegatedStake,
    TimeUnit,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Card, CardImage, CardType, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { useMemo } from 'react';
import { Link } from 'react-router-dom';
import { ImageIcon } from '../../shared/image-icon';

import { useIotaClientQuery } from '@iota/dapp-kit';

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

interface StakeCardProps {
    extendedStake: ExtendedDelegatedStake;
    currentEpoch: number;
    inactiveValidator?: boolean;
}

// For delegationsRequestEpoch n  through n + 2, show Start Earning
// Show epoch number or date/time for n + 3 epochs
export function StakeCard({
    extendedStake,
    currentEpoch,
    inactiveValidator = false,
}: StakeCardProps) {
    const { stakedIotaId, principal, stakeRequestEpoch, estimatedReward, validatorAddress } =
        extendedStake;

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
        <Link
            data-testid="stake-card"
            to={`/stake/delegation-detail?${new URLSearchParams({
                validator: validatorAddress,
                staked: stakedIotaId,
            }).toString()}`}
            className="no-underline"
        >
            <Card type={CardType.Default}>
                <CardImage>
                    <ImageIcon
                        src={validatorMeta?.imageUrl || null}
                        label={validatorMeta?.name || ''}
                        fallback={validatorMeta?.name || ''}
                    />
                </CardImage>
                <CardBody
                    title={validatorMeta?.name || ''}
                    subtitle={`${principalStaked} ${symbol}`}
                />
                <CardAction
                    title={rewardTime()}
                    subtitle={STATUS_COPY[delegationState]}
                    type={CardActionType.SupportingText}
                />
            </Card>
        </Link>
    );
}
