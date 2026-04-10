// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useGetInactiveValidator,
    useGetValidatorsApy,
    useGetValidatorsEvents,
    useFormatCoin,
    useMaxCommitteeSize,
} from '@iota/core';
import { useParams } from 'react-router-dom';
import {
    InactiveValidators,
    PageLayout,
    ValidatorMeta,
    ValidatorStats,
    ValidatorStatusLegend,
} from '~/components';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { getValidatorMoveEvent } from '~/lib/utils';
import {
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LabelText,
    LabelTextSize,
    LoadingIndicator,
    Panel,
    Title,
    Tooltip,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { Info, Warning } from '@iota/apps-ui-icons';
import type { LatestIotaSystemStateSummary } from '@iota/iota-sdk/client';
import { useIotaClientQuery } from '@iota/dapp-kit';

type PrevEpochEventData = {
    pool_staking_reward?: string;
    stake?: string;
    reference_gas_survey_quote?: string;
    commission_rate?: string;
};

const getAtRiskRemainingEpochs = (
    data: LatestIotaSystemStateSummary | undefined,
    validatorId: string | undefined,
): number | null => {
    if (!data || !validatorId) return null;

    const atRiskList = data.atRiskValidators.length ? data.atRiskValidators : data.atRiskValidators;
    const atRisk = atRiskList.find(([address]) => address === validatorId);
    return atRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRisk[1]) : null;
};

function ValidatorDetails(): JSX.Element {
    const { id } = useParams();
    const { data: systemStateData, isLoading: isLoadingSystemState } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );
    const { data: maxCommitteeSize } = useMaxCommitteeSize();
    const { data: inactiveValidatorData, isLoading: isInactiveValidatorLoading } =
        useGetInactiveValidator(id || '');

    const numberOfValidators = systemStateData?.activeValidators.length ?? null;
    const { data: rollingAverageApys, isLoading: isValidatorsApysLoading } = useGetValidatorsApy();
    const { data: validatorEvents, isLoading: isValidatorsEventsLoading } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });
    const epochId = systemStateData?.epoch;
    const prevEpochEvent =
        validatorEvents && id && epochId
            ? (getValidatorMoveEvent(validatorEvents, id, epochId) as PrevEpochEventData | null)
            : null;
    const validatorRewards = prevEpochEvent?.pool_staking_reward
        ? Number(prevEpochEvent.pool_staking_reward)
        : null;

    const activeValidatorData = systemStateData?.activeValidators.find(
        ({ iotaAddress, stakingPoolId }) => iotaAddress === id || stakingPoolId === id,
    );

    const atRiskRemainingEpochs = getAtRiskRemainingEpochs(systemStateData, id);

    const [formattedNextEpochStake, nextEpochStakeSymbol] = useFormatCoin({
        balance: Number(activeValidatorData?.nextEpochStake ?? 0),
    });

    const [formattedPrevEpochRewards, prevEpochRewardsSymbol] = useFormatCoin({
        balance: validatorRewards,
    });
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

    const nextEpochCommission = Number(activeValidatorData.nextEpochCommissionRate) / 100;

    const isEarningCurrentEpoch = systemStateData.committeeMembers.some(
        (member) => member.iotaAddress === id,
    );
    const validatorsSortedByStake = [...systemStateData.activeValidators].sort((a, b) =>
        BigInt(b.stakingPoolIotaBalance) > BigInt(a.stakingPoolIotaBalance) ? 1 : -1,
    );
    const topValidators = validatorsSortedByStake.slice(0, maxCommitteeSize ?? 0);
    const isInTopStakers = topValidators.some((v) => v.iotaAddress === id);
    const isEarningNextEpoch =
        (atRiskRemainingEpochs === null || atRiskRemainingEpochs > 1) && isInTopStakers;
    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-xl">
                    <ValidatorMeta
                        validatorData={activeValidatorData}
                        atRiskRemainingEpochs={atRiskRemainingEpochs}
                    />
                    <ValidatorStats
                        validatorData={activeValidatorData}
                        epoch={systemStateData.epoch}
                        epochRewards={validatorRewards}
                        apy={isApyApproxZero ? '~0' : apy}
                        isEarningCurrentEpoch={isEarningCurrentEpoch}
                    />
                    <div className="flex flex-col gap-lg md:flex-row">
                        <Panel>
                            <Title
                                title="Previous Epoch"
                                trailingElement={
                                    prevEpochEvent ? (
                                        <EpochStatusIndicator
                                            active={!!validatorRewards}
                                            activeLabel="Earned rewards"
                                            inactiveLabel="No rewards"
                                            tooltipText="Whether this validator earned staking rewards in the previous epoch."
                                        />
                                    ) : undefined
                                }
                            />
                            <div className="p-md--rs">
                                <LabelText
                                    size={LabelTextSize.Medium}
                                    label="Last Epoch Rewards"
                                    text={
                                        validatorRewards === null ? '--' : formattedPrevEpochRewards
                                    }
                                    supportingLabel={
                                        validatorRewards !== null
                                            ? prevEpochRewardsSymbol
                                            : undefined
                                    }
                                    tooltipText="Total staking rewards distributed to this validator's pool at the last epoch boundary."
                                    tooltipPosition={TooltipPosition.Right}
                                />
                            </div>
                        </Panel>
                        <Panel>
                            <Title
                                title="Next Epoch"
                                trailingElement={
                                    <EpochStatusIndicator
                                        active={
                                            maxCommitteeSize !== undefined && isEarningNextEpoch
                                        }
                                        activeLabel="Earning rewards"
                                        inactiveLabel="Not earning"
                                        tooltipText="Whether this validator is projected to earn rewards next epoch, based on its stake ranking and at-risk status."
                                        tooltipPosition={TooltipPosition.Left}
                                        loadingLabel={
                                            maxCommitteeSize === undefined ? 'Loading…' : undefined
                                        }
                                    />
                                }
                            />
                            <div className="grid grid-cols-2 gap-md p-md--rs">
                                <LabelText
                                    size={LabelTextSize.Medium}
                                    label="Stake"
                                    text={formattedNextEpochStake}
                                    supportingLabel={nextEpochStakeSymbol}
                                    tooltipText="The projected total stake at the next epoch boundary, after all pending delegations and withdrawals are settled."
                                    tooltipPosition={TooltipPosition.Right}
                                />
                                <LabelText
                                    size={LabelTextSize.Medium}
                                    label="Commission"
                                    text={`${nextEpochCommission}%`}
                                    tooltipText="The commission rate this validator will charge from the next epoch onwards."
                                    tooltipPosition={TooltipPosition.Right}
                                />
                            </div>
                        </Panel>
                    </div>
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
                    <ValidatorStatusLegend />
                </div>
            }
        />
    );
}

export { ValidatorDetails };

type EpochStatusIndicatorProps = {
    active: boolean;
    activeLabel: string;
    inactiveLabel: string;
    tooltipText: string;
    tooltipPosition?: TooltipPosition;
    loadingLabel?: string;
};

export function EpochStatusIndicator({
    active,
    activeLabel,
    inactiveLabel,
    tooltipText,
    tooltipPosition = TooltipPosition.Top,
    loadingLabel,
}: EpochStatusIndicatorProps): JSX.Element {
    return (
        <Tooltip text={tooltipText} position={tooltipPosition}>
            <div className="flex cursor-default items-center gap-1.5">
                <span
                    className={`h-2 w-2 shrink-0 rounded-full ${
                        active ? 'bg-iota-tertiary-50' : 'bg-iota-neutral-40'
                    }`}
                />
                <span
                    className={`shrink-0 text-label-md ${
                        active ? 'text-iota-tertiary-50' : 'label-text-secondary-neutral'
                    }`}
                >
                    {loadingLabel ?? (active ? activeLabel : inactiveLabel)}
                </span>
                <Info className="label-text-secondary-neutral h-3.5 w-3.5 shrink-0" />
            </div>
        </Tooltip>
    );
}
