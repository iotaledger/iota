// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetValidatorsApy, useGetValidatorsEvents } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type IotaSystemStateSummary } from '@iota/iota-sdk/client';
import { useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { PageLayout, ValidatorMeta, ValidatorStats } from '~/components';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { getValidatorMoveEvent } from '~/lib/utils';
import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

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
        return <PageLayout content={<LoadingIndicator />} />;
    }

    if (!validatorData || !data || !validatorEvents || !id) {
        return (
            <PageLayout
                content={
                    <div className="mb-10">
                        <InfoBox
                            title="Failed to load validator data"
                            supportingText={`No validator data found for ${id}`}
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
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
                        <InfoBox
                            title={`At risk of being removed as a validator after ${atRiskRemainingEpochs} epoch${
                                atRiskRemainingEpochs > 1 ? 's' : ''
                            }`}
                            supportingText="Staked IOTA is below the minimum IOTA stake threshold to remain
                                    a validator."
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    )}
                </div>
            }
        />
    );
}

export { ValidatorDetails };
