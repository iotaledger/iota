// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Card,
    CardImage,
    CardType,
    CardBody,
    CardAction,
    CardActionType,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { useMemo } from 'react';
import { ImageIcon } from '../icon';
import { ExtendedDelegatedStake } from '../../utils';
import { useFormatCoin, useStakeRewardStatus } from '../../hooks';
import { RewardsOff, Warning } from '@iota/apps-ui-icons';
import { useIotaClientQuery } from '@iota/dapp-kit';

interface StakedCardProps {
    extendedStake: ExtendedDelegatedStake;
    currentEpoch: number;
    inactiveValidator?: boolean;
    activeButNotInTheCommittee?: boolean;
    onClick: () => void;
}

// For delegationsRequestEpoch n  through n + 2, show Start Earning
// Show epoch number or date/time for n + 3 epochs
export function StakedCard({
    extendedStake,
    currentEpoch,
    inactiveValidator = false,
    activeButNotInTheCommittee = false,
    onClick,
}: StakedCardProps) {
    const { principal, stakeRequestEpoch, estimatedReward, validatorAddress } = extendedStake;

    const { title, subtitle } = useStakeRewardStatus({
        stakeRequestEpoch,
        currentEpoch,
        estimatedReward,
        inactiveValidator,
        activeButNotInTheCommittee,
    });

    const [principalStaked, symbol] = useFormatCoin({
        balance: principal,
    });

    const { data } = useIotaClientQuery('getLatestIotaSystemState');

    const validatorMeta = useMemo(() => {
        if (!data) return null;

        return (
            data.activeValidators.find((validator) => validator.iotaAddress === validatorAddress) ||
            null
        );
    }, [validatorAddress, data]);

    const name = validatorMeta?.name || validatorAddress;

    return (
        <Card testId="staked-card" type={CardType.Default} isHoverable onClick={onClick}>
            <CardImage>
                <ImageIcon src={validatorMeta?.imageUrl || null} label={name} fallback={name} />
            </CardImage>
            <CardBody
                title={name}
                subtitle={`${principalStaked} ${symbol}`}
                icon={
                    activeButNotInTheCommittee ? (
                        <RewardsOff className="text-warning-60" />
                    ) : inactiveValidator ? (
                        <Warning className="text-error-30" />
                    ) : null
                }
                tooltipText={
                    activeButNotInTheCommittee
                        ? 'This validator is active but not in the current committee, so it’s not earning rewards right now. It may earn in future epochs.'
                        : inactiveValidator
                          ? 'This validator is inactive and will no longer earn rewards. '
                          : ''
                }
                tooltipPosition={TooltipPosition.Bottom}
            />
            <CardAction title={title} subtitle={subtitle} type={CardActionType.SupportingText} />
        </Card>
    );
}
