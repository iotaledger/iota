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
import { InactiveValidators, PageLayout, ValidatorMeta, ValidatorStats } from '~/components';
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
import { CoinFormat } from '@iota/iota-sdk/utils';
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
    const { data: maxCommitteeSize } = useMaxCommitteeSize();
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

    const [formattedNextEpochStake, nextEpochStakeSymbol] = useFormatCoin({
        balance: Number(activeValidatorData?.nextEpochStake ?? 0),
    });
    const [formattedGasPrice, gasPriceSymbol] = useFormatCoin({
        balance: Number(activeValidatorData?.gasPrice ?? 0),
        format: CoinFormat.Full,
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
    const votingPower = Number(activeValidatorData.votingPower) / 100;

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
                    <div className="flex flex-col gap-lg md:flex-row">
                        <Panel>
                            <Title
                                title="Current Epoch"
                                trailingElement={
                                    <Tooltip
                                        text="Whether this validator is in the active committee and earning staking rewards this epoch."
                                        position={TooltipPosition.Top}
                                    >
                                        <div className="flex cursor-default items-center gap-1.5 px-md--rs">
                                            <span
                                                className={`h-2 w-2 shrink-0 rounded-full ${
                                                    isEarningCurrentEpoch
                                                        ? 'bg-iota-tertiary-50'
                                                        : 'bg-iota-neutral-40'
                                                }`}
                                            />
                                            <span
                                                className={`text-label-md ${
                                                    isEarningCurrentEpoch
                                                        ? 'text-iota-tertiary-50'
                                                        : 'label-text-secondary-neutral'
                                                }`}
                                            >
                                                {isEarningCurrentEpoch
                                                    ? 'Earning rewards'
                                                    : 'Not earning'}
                                            </span>
                                            <Info className="label-text-secondary-neutral h-3.5 w-3.5" />
                                        </div>
                                    </Tooltip>
                                }
                            />
                            <div className="grid grid-cols-2 gap-md p-md--rs">
                                <LabelText
                                    size={LabelTextSize.Medium}
                                    label="Voting Power"
                                    text={`${votingPower}%`}
                                    tooltipText="Share of total committee voting power held by this validator, proportional to its stake."
                                    tooltipPosition={TooltipPosition.Right}
                                />
                                <LabelText
                                    size={LabelTextSize.Medium}
                                    label="Gas Price"
                                    text={formattedGasPrice}
                                    supportingLabel={gasPriceSymbol}
                                    tooltipText="The reference gas price proposed by this validator for the current epoch."
                                    tooltipPosition={TooltipPosition.Right}
                                />
                            </div>
                        </Panel>
                        <Panel>
                            <Title
                                title="Next Epoch"
                                trailingElement={
                                    <Tooltip
                                        text="Whether this validator is projected to earn rewards next epoch, based on its stake ranking and at-risk status."
                                        position={TooltipPosition.Left}
                                    >
                                        <div className="flex cursor-default items-center gap-1.5 px-md--rs">
                                            <span
                                                className={`h-2 w-2 shrink-0 rounded-full ${
                                                    maxCommitteeSize !== undefined &&
                                                    isEarningNextEpoch
                                                        ? 'bg-iota-tertiary-50'
                                                        : 'bg-iota-neutral-40'
                                                }`}
                                            />
                                            <span
                                                className={`text-label-md ${
                                                    maxCommitteeSize !== undefined &&
                                                    isEarningNextEpoch
                                                        ? 'text-iota-tertiary-50'
                                                        : 'label-text-secondary-neutral'
                                                }`}
                                            >
                                                {maxCommitteeSize === undefined
                                                    ? 'Loading…'
                                                    : isEarningNextEpoch
                                                      ? 'Earning rewards'
                                                      : 'Not earning'}
                                            </span>
                                            <Info className="label-text-secondary-neutral h-3.5 w-3.5" />
                                        </div>
                                    </Tooltip>
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
                </div>
            }
        />
    );
}

export { ValidatorDetails };
