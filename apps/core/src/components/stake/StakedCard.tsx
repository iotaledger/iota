// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardImage, CardType, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { useMemo } from 'react';
import { ImageIcon } from '../icon';
import { ExtendedDelegatedStake } from '../../utils';
import { useFormatCoin, useStakeRewardStatus, getInactiveValidatorsData } from '../../hooks';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

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
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const iotaClient = useIotaClient();

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

    const validatorMeta = useMemo(() => {
        if (!data) return null;

        return (
            data.activeValidators.find((validator) => validator.iotaAddress === validatorAddress) ||
            null
        );
    }, [validatorAddress, data]);

    const { data: inactiveValidatorData } = useQuery({
        queryKey: [data?.inactivePoolsId, validatorAddress],
        async queryFn() {
            if (!data?.inactivePoolsId || !validatorAddress) {
                throw Error('Missing params');
            }
            const inactiveValidators = await iotaClient.getDynamicFields({
                parentId: normalizeIotaAddress(data?.inactivePoolsId),
            });

            const pendingInactiveValidatorsData = await Promise.all(
                inactiveValidators.data.map(
                    async (validator) =>
                        await getInactiveValidatorsData(iotaClient, validator.objectId),
                ),
            );

            return pendingInactiveValidatorsData;
        },
        enabled: !!data?.inactivePoolsId && !!validatorAddress,
        select(validators) {
            return validators.find((validator) => validator?.validatorAddress === validatorAddress);
        },
    });

    const combinedValidatorData = useMemo(() => {
        if (!validatorMeta && !inactiveValidatorData) return null;
        return {
            ...validatorMeta,
            ...inactiveValidatorData,
        };
    }, [validatorMeta, inactiveValidatorData]);

    return (
        <Card testId="staked-card" type={CardType.Default} isHoverable onClick={onClick}>
            <CardImage>
                <ImageIcon
                    src={combinedValidatorData?.imageUrl || null}
                    label={combinedValidatorData?.name || ''}
                    fallback={combinedValidatorData?.name || ''}
                />
            </CardImage>
            <CardBody
                title={combinedValidatorData?.name || '--'}
                subtitle={`${principalStaked} ${symbol}`}
            />
            <CardAction title={title} subtitle={subtitle} type={CardActionType.SupportingText} />
        </Card>
    );
}
