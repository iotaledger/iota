// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';
import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { Card, CardImage, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { useFormatCoin, ImageIcon, ImageIconSize, useStakeRewardStatus } from '@iota/core';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';

export interface StakedTimelockObjectProps {
    timelockedStakedObject: TimelockedStakedObjectsGrouped;
    handleUnstake: (timelockedStakedObject: TimelockedStakedObjectsGrouped) => void;
    getValidatorByAddress: (validatorAddress: string) => IotaValidatorSummary | undefined;
    currentEpoch: number;
}

export function StakedTimelockObject({
    getValidatorByAddress,
    timelockedStakedObject,
    handleUnstake,
    currentEpoch,
}: StakedTimelockObjectProps) {
    const name =
        getValidatorByAddress(timelockedStakedObject.validatorAddress)?.name ||
        timelockedStakedObject.validatorAddress;

    // TODO probably we could calculate estimated reward on grouping stage.
    const summary = timelockedStakedObject.stakes.reduce(
        (acc, stake) => {
            const estimatedReward = stake.status === 'Active' ? BigInt(stake.estimatedReward) : 0n;

            return {
                principal: BigInt(stake.principal) + acc.principal,
                estimatedReward: estimatedReward + acc.estimatedReward,
                stakeRequestEpoch: stake.stakeRequestEpoch,
            };
        },
        {
            principal: 0n,
            estimatedReward: 0n,
            stakeRequestEpoch: '',
        },
    );

    const supportingText = useStakeRewardStatus({
        currentEpoch,
        stakeRequestEpoch: summary.stakeRequestEpoch,
        estimatedReward: summary.estimatedReward,
        inactiveValidator: false,
        activeButNotInTheCommittee: false,
    });

    const [sumPrincipalFormatted, sumPrincipalSymbol] = useFormatCoin({
        balance: summary.principal,
    });

    return (
        <Card onClick={() => handleUnstake(timelockedStakedObject)}>
            <CardImage>
                <ImageIcon src={null} label={name} fallback={name} size={ImageIconSize.Large} />
            </CardImage>
            <CardBody
                title={name}
                subtitle={`${sumPrincipalFormatted} ${sumPrincipalSymbol}`}
                isTextTruncated
            />
            <CardAction
                type={CardActionType.SupportingText}
                title={supportingText.title}
                subtitle={supportingText.subtitle}
            />
        </Card>
    );
}
