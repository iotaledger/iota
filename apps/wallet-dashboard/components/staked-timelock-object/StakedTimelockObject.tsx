// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';
import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { Card, CardImage, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { useFormatCoin, ImageIcon, ImageIconSize } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';

export interface StakedTimelockObjectProps {
    timelockedStakedObject: TimelockedStakedObjectsGrouped;
    handleUnstake: (timelockedStakedObject: TimelockedStakedObjectsGrouped) => void;
    getValidatorByAddress: (validatorAddress: string) => IotaValidatorSummary | undefined;
}

export function StakedTimelockObject({
    getValidatorByAddress,
    timelockedStakedObject,
    handleUnstake,
}: StakedTimelockObjectProps) {
    const name =
        getValidatorByAddress(timelockedStakedObject.validatorAddress)?.name ||
        timelockedStakedObject.validatorAddress;
    const sum = timelockedStakedObject.stakes.reduce(
        (acc, stake) => {
            const estimatedReward = stake.status === 'Active' ? stake.estimatedReward : 0;

            return {
                principal: Number(stake.principal) + acc.principal,
                estimatedReward: Number(estimatedReward) + acc.estimatedReward,
            };
        },
        {
            principal: 0,
            estimatedReward: 0,
        },
    );

    const [sumPrincipalFormatted, sumPrincipalSymbol] = useFormatCoin(sum.principal, IOTA_TYPE_ARG);
    const [estimatedRewardFormatted, estimatedRewardSymbol] = useFormatCoin(
        sum.estimatedReward,
        IOTA_TYPE_ARG,
    );

    const supportingText = (() => {
        if (timelockedStakedObject.stakes.every((s) => s.status === 'Active')) {
            return {
                title: 'Estimated Reward',
                subtitle: `${estimatedRewardFormatted} ${estimatedRewardSymbol}`,
            };
        }

        return {
            title: 'Stake Request Epoch',
            subtitle: timelockedStakedObject.stakeRequestEpoch,
        };
    })();

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
