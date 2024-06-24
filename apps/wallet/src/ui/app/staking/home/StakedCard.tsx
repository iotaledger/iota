// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE } from '_src/shared/constants';
import { CountDownTimer } from '_src/ui/app/shared/countdown-timer';
import { Text } from '_src/ui/app/shared/text';
import { IconTooltip } from '_src/ui/app/shared/tooltip';
import {
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    type ExtendedDelegatedStake,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { cva, cx, type VariantProps } from 'class-variance-authority';
import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';

import { ValidatorLogo } from '../validators/ValidatorLogo';

export enum StakeState {
    WARM_UP = 'WARM_UP',
    EARNING = 'EARNING',
    COOL_DOWN = 'COOL_DOWN',
    WITHDRAW = 'WITHDRAW',
    IN_ACTIVE = 'IN_ACTIVE',
}

const STATUS_COPY = {
    [StakeState.WARM_UP]: 'Starts Earning',
    [StakeState.EARNING]: 'Staking Rewards',
    [StakeState.COOL_DOWN]: 'Available to withdraw',
    [StakeState.WITHDRAW]: 'Withdraw',
    [StakeState.IN_ACTIVE]: 'Inactive',
};

const STATUS_VARIANT = {
    [StakeState.WARM_UP]: 'warmUp',
    [StakeState.EARNING]: 'earning',
    [StakeState.COOL_DOWN]: 'coolDown',
    [StakeState.WITHDRAW]: 'withDraw',
    [StakeState.IN_ACTIVE]: 'inActive',
} as const;

const cardStyle = cva(
    [
        'group flex no-underline flex-col p-3.75 pr-2 py-3 box-border w-full rounded-2xl border border-solid h-36',
    ],
    {
        variants: {
            variant: {
                warmUp: 'bg-white border border-gray-45 text-steel-dark hover:bg-iota/10 hover:border-iota/30',
                earning:
                    'bg-white border border-gray-45 text-steel-dark hover:bg-iota/10 hover:border-iota/30',
                coolDown:
                    'bg-warning-light border-transparent text-steel-darker hover:border-warning',
                withDraw:
                    'bg-success-light border-transparent text-success-dark hover:border-success',
                inActive: 'bg-issue-light border-transparent text-issue hover:border-issue',
            },
        },
    },
);

export interface StakeCardContentProps extends VariantProps<typeof cardStyle> {
    statusLabel: string;
    statusText: string;
    children?: ReactNode;
    earnColor?: boolean;
    earningRewardEpoch?: number | null;
}

function StakeCardContent({
    children,
    statusLabel,
    statusText,
    variant,
    earnColor,
    earningRewardEpoch,
}: StakeCardContentProps) {
    const { data: rewardEpochTime } = useGetTimeBeforeEpochNumber(earningRewardEpoch || 0);
    return (
        <div className={cardStyle({ variant })}>
            {children}
            <div className="flex flex-col gap-1">
                <div className="text-subtitle font-medium">{statusLabel}</div>
                <div
                    className={cx(
                        'text-bodySmall font-semibold',
                        earnColor ? 'text-success-dark' : '',
                    )}
                >
                    {earningRewardEpoch && rewardEpochTime > 0 ? (
                        <CountDownTimer
                            timestamp={rewardEpochTime}
                            variant="bodySmall"
                            label="in"
                        />
                    ) : (
                        statusText
                    )}
                </div>
            </div>
        </div>
    );
}

interface StakeCardProps {
    delegationObject: ExtendedDelegatedStake;
    currentEpoch: number;
    inactiveValidator?: boolean;
}

// For delegationsRequestEpoch n  through n + 2, show Start Earning
// Show epoch number or date/time for n + 3 epochs
export function StakeCard({
    delegationObject,
    currentEpoch,
    inactiveValidator = false,
}: StakeCardProps) {
    const { stakedIotaId, principal, stakeRequestEpoch, estimatedReward, validatorAddress } =
        delegationObject;

    // TODO: Once two step withdraw is available, add cool down and withdraw now logic
    // For cool down epoch, show Available to withdraw add rewards to principal
    // Reward earning epoch is 2 epochs after stake request epoch
    const earningRewardsEpoch =
        Number(stakeRequestEpoch) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;
    const isEarnedRewards = currentEpoch >= Number(earningRewardsEpoch);
    const delegationState = inactiveValidator
        ? StakeState.IN_ACTIVE
        : isEarnedRewards
          ? StakeState.EARNING
          : StakeState.WARM_UP;

    const rewards = isEarnedRewards && estimatedReward ? BigInt(estimatedReward) : 0n;

    // For inactive validator, show principal + rewards
    const [principalStaked, symbol] = useFormatCoin(
        inactiveValidator ? principal + rewards : principal,
        IOTA_TYPE_ARG,
    );
    const [rewardsStaked] = useFormatCoin(rewards, IOTA_TYPE_ARG);
    const isEarning = delegationState === StakeState.EARNING && rewards > 0n;

    // Applicable only for warm up
    const epochBeforeRewards = delegationState === StakeState.WARM_UP ? earningRewardsEpoch : null;

    const statusText = {
        // Epoch time before earning
        [StakeState.WARM_UP]: `Epoch #${earningRewardsEpoch}`,
        [StakeState.EARNING]: `${rewardsStaked} ${symbol}`,
        // Epoch time before redrawing
        [StakeState.COOL_DOWN]: `Epoch #`,
        [StakeState.WITHDRAW]: 'Now',
        [StakeState.IN_ACTIVE]: 'Not earning rewards',
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
            <StakeCardContent
                variant={STATUS_VARIANT[delegationState]}
                statusLabel={STATUS_COPY[delegationState]}
                statusText={statusText[delegationState]}
                earnColor={isEarning}
                earningRewardEpoch={Number(epochBeforeRewards)}
            >
                <div className="mb-1 flex">
                    <ValidatorLogo
                        validatorAddress={validatorAddress}
                        size="subtitle"
                        iconSize="md"
                        stacked
                        activeEpoch={delegationObject.stakeRequestEpoch}
                    />

                    <div className="text-pBody text-steel opacity-0 group-hover:opacity-100">
                        <IconTooltip
                            tip="Object containing the delegated staked IOTA tokens, owned by each delegator"
                            placement="top"
                        />
                    </div>
                </div>
                <div className="flex-1">
                    <div className="flex items-baseline gap-1">
                        <Text variant="body" weight="semibold" color="gray-90">
                            {principalStaked}
                        </Text>
                        <Text variant="subtitle" weight="normal" color="gray-90">
                            {symbol}
                        </Text>
                    </div>
                </div>
            </StakeCardContent>
        </Link>
    );
}
