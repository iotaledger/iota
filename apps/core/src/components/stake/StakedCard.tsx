// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardAction, CardActionType, CardBody, CardImage, CardType } from '@iota/apps-ui-kit';
import { useMemo } from 'react';
import { useFormatCoin, useStakeRewardStatus } from '../../hooks';
import { ExtendedDelegatedStake, getUniversalIotaSystemStateFields } from '../../utils';
import { ImageIcon } from '../icon';

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

    const { rewards, title, subtitle } = useStakeRewardStatus({
        stakeRequestEpoch,
        currentEpoch,
        estimatedReward,
        inactiveValidator,
    });

    // For inactive validator, show principal + rewards
    const [principalStaked, symbol] = useFormatCoin({
        balance: inactiveValidator ? BigInt(principal) + rewards : principal,
    });

    const { committeeMembers, epoch } = getUniversalIotaSystemStateFields();

    const validatorMeta = useMemo(() => {
        if (!epoch) return null;

        return (
            committeeMembers.find((validator) => validator.iotaAddress === validatorAddress) || null
        );
    }, [validatorAddress, committeeMembers]);

    return (
        <Card testId="staked-card" type={CardType.Default} isHoverable onClick={onClick}>
            <CardImage>
                <ImageIcon
                    src={validatorMeta?.imageUrl || null}
                    label={validatorMeta?.name || ''}
                    fallback={validatorMeta?.name || ''}
                />
            </CardImage>
            <CardBody
                title={validatorMeta?.name || '--'}
                subtitle={`${principalStaked} ${symbol}`}
            />
            <CardAction title={title} subtitle={subtitle} type={CardActionType.SupportingText} />
        </Card>
    );
}
