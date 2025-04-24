// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardImage, CardType, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { useMemo } from 'react';
import { ImageIcon } from '../icon';
import { ExtendedDelegatedStake } from '../../utils';
import { useFormatCoin, useStakeRewardStatus } from '../../hooks';
import { useIotaClientQuery } from '@iota/dapp-kit';

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

    const { data } = useIotaClientQuery('getLatestIotaSystemState');

    const validatorMeta = useMemo(() => {
        if (!data) return null;

        return (
            data.activeValidators.find((validator) => validator.iotaAddress === validatorAddress) ||
            null
        );
    }, [validatorAddress, data]);

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
