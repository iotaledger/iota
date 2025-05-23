// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetInactiveValidator, useGetValidatorsApy, useGetValidatorsEvents } from '@iota/core';
import { useParams } from 'react-router-dom';
import { InactiveValidators, PageLayout, ValidatorMeta, ValidatorStats } from '~/components';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { getValidatorMoveEvent } from '~/lib/utils';
import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import type { LatestIotaSystemStateSummary } from '@iota/iota-sdk/client';
import { useIotaClientQuery } from '@iota/dapp-kit';

const getAtRiskRemainingEpochs = (
    data: LatestIotaSystemStateSummary | undefined,
    validatorId: string | undefined,
): number | null => {
    if (!data || !validatorId) return null;
    const atRisk = data.atRiskValidators.find(([address]) => address === validatorId);
    return atRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRisk[1]) : null;
};

function ValidatorDetails(): JSX.Element {
    const { id } = useParams();
    const { data: systemStateData, isLoading: isLoadingSystemState } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );

    const { data: inactiveValidatorData, isLoading: isInactiveValidatorLoading } =
        useGetInactiveValidator(id || '');

    const numberOfValidators = systemStateData?.activeValidators.length ?? null;
    const { data: rollingAverageApys, isLoading: isValidatorsApysLoading } = useGetValidatorsApy();
    const { data: validatorEvents, isLoading: isValidatorsEventsLoading } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });
    const epochId = systemStateData?.epoch;
    const validatorRewards = (() => {
        if (!validatorEvents || !id || !epochId) return 0;
        const rewards = (
            getValidatorMoveEvent(validatorEvents, id, epochId) as { pool_staking_reward: string }
        )?.pool_staking_reward;

        return rewards ? Number(rewards) : null;
    })();

    const activeValidatorData = systemStateData?.activeValidators.find(
        ({ iotaAddress, stakingPoolId }) => iotaAddress === id || stakingPoolId === id,
    );

    const atRiskRemainingEpochs = getAtRiskRemainingEpochs(systemStateData, id);

    if (
        isLoadingSystemState ||
        isValidatorsEventsLoading ||
        isValidatorsApysLoading ||
        isInactiveValidatorLoading
    ) {
        return <PageLayout content={<LoadingIndicator />} />;
    }

    if (inactiveValidatorData && !activeValidatorData) {
        return (
            <PageLayout
                content={
                    <div className="mb-10">
                        <InfoBox
                            title="Inactive validator"
                            icon={<Warning />}
                            type={InfoBoxType.Warning}
                            style={InfoBoxStyle.Elevated}
                        />
                        {inactiveValidatorData && (
                            <InactiveValidators validatorData={inactiveValidatorData} />
                        )}
                    </div>
                }
            />
        );
    }

    if (!activeValidatorData || !systemStateData || !validatorEvents || !id) {
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
                    <ValidatorMeta validatorData={activeValidatorData} />
                    <ValidatorStats
                        validatorData={activeValidatorData}
                        epoch={systemStateData.epoch}
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
                            type={InfoBoxType.Warning}
                            style={InfoBoxStyle.Elevated}
                        />
                    )}
                </div>
            }
        />
    );
}

export { ValidatorDetails };
