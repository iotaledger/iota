// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetValidatorsApy, useGetValidatorsEvents } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type IotaSystemStateSummary } from '@iota/iota-sdk/client';
import { LoadingIndicator, Text } from '@iota/ui';
import React, { useMemo } from 'react';
import { useParams } from 'react-router-dom';

import { PageLayout, ValidatorMeta, ValidatorStats } from '~/components';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { getValidatorMoveEvent } from '~/lib/utils';
import { Banner } from '~/components/ui';

const getAtRiskRemainingEpochs = (
    data: IotaSystemStateSummary | undefined,
    validatorId: string | undefined,
): number | null => {
    if (!data || !validatorId) return null;
    const atRisk = data.atRiskValidators.find(([address]) => address === validatorId);
    return atRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRisk[1]) : null;
};

function ValidatorDetails(): JSX.Element {
    const { id } = useParams();
    const { data, isPending } = useIotaClientQuery('getLatestIotaSystemState');

    const validatorData = useMemo(() => {
        if (!data) return null;
        return (
            data.activeValidators.find(
                ({ iotaAddress, stakingPoolId }) => iotaAddress === id || stakingPoolId === id,
            ) || null
        );
    }, [id, data]);

    const atRiskRemainingEpochs = getAtRiskRemainingEpochs(data, id);

    const numberOfValidators = data?.activeValidators.length ?? null;
    const { data: rollingAverageApys, isPending: validatorsApysLoading } = useGetValidatorsApy();

    const { data: validatorEvents, isPending: validatorsEventsLoading } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });

    const validatorRewards = useMemo(() => {
        if (!validatorEvents || !id) return 0;
        const rewards = (
            getValidatorMoveEvent(validatorEvents, id) as { pool_staking_reward: string }
        )?.pool_staking_reward;

        return rewards ? Number(rewards) : null;
    }, [id, validatorEvents]);

    if (isPending || validatorsEventsLoading || validatorsApysLoading) {
        return (
            <PageLayout
                content={
                    <div className="mb-10 flex items-center justify-center">
                        <LoadingIndicator />
                    </div>
                }
            />
        );
    }

    if (!validatorData || !data || !validatorEvents || !id) {
        return (
            <PageLayout
                content={
                    <div className="mb-10 flex items-center justify-center">
                        <Banner variant="error" spacing="lg" fullWidth>
                            No validator data found for {id}
                        </Banner>
                    </div>
                }
            />
        );
    }
    const { apy, isApyApproxZero } = rollingAverageApys?.[id] ?? { apy: null };

    const tallyingScore =
        (
            validatorEvents as {
                parsedJson?: { tallying_rule_global_score?: string; validator_address?: string };
            }[]
        )?.find(({ parsedJson }) => parsedJson?.validator_address === id)?.parsedJson
            ?.tallying_rule_global_score || null;

    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-2xl">
                    <ValidatorMeta validatorData={validatorData} />
                    <ValidatorStats
                        validatorData={validatorData}
                        epoch={data.epoch}
                        epochRewards={validatorRewards}
                        apy={isApyApproxZero ? '~0' : apy}
                        tallyingScore={tallyingScore}
                    />
                    {atRiskRemainingEpochs !== null && (
                        <div className="mt-5">
                            <Banner
                                fullWidth
                                border
                                variant="error"
                                title={
                                    <Text uppercase variant="bodySmall/semibold">
                                        at risk of being removed as a validator after{' '}
                                        {atRiskRemainingEpochs} epoch
                                        {atRiskRemainingEpochs > 1 ? 's' : ''}
                                    </Text>
                                }
                            >
                                <Text variant="bodySmall/medium">
                                    Staked IOTA is below the minimum IOTA stake threshold to remain
                                    a validator.
                                </Text>
                            </Banner>
                        </div>
                    )}
                </div>
            }
        />
    );
}

export { ValidatorDetails };
