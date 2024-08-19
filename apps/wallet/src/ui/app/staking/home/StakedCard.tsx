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
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { cva, cx, type VariantProps } from 'class-variance-authority';
import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';

import { ValidatorLogo } from '../validators/ValidatorLogo';

export enum StakeState {
    WarmUp = 'WARM_UP',
    Earning = 'EARNING',
    CoolDown = 'COOL_DOWN',
    Withdraw = 'WITHDRAW',
    InActive = 'IN_ACTIVE',
}

const STATUS_COPY = {
    [StakeState.WarmUp]: 'Starts Earning',
    [StakeState.Earning]: 'Staking Rewards',
    [StakeState.CoolDown]: 'Available to withdraw',
    [StakeState.Withdraw]: 'Withdraw',
    [StakeState.InActive]: 'Inactive',
};

const STATUS_VARIANT = {
    [StakeState.WarmUp]: 'warmUp',
    [StakeState.Earning]: 'earning',
    [StakeState.CoolDown]: 'coolDown',
    [StakeState.Withdraw]: 'withDraw',
    [StakeState.InActive]: 'inActive',
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
    const isEarning = delegationState === StakeState.Earning && rewards > 0n;

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
                        stacked
                        activeEpoch={extendedStake.stakeRequestEpoch}
                    />

                    <div className="text-steel text-pBody opacity-0 group-hover:opacity-100">
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
