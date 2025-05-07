// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector, useActiveAddress } from '_hooks';
import { ampli } from '_src/shared/analytics/ampli';
import {
    useBalance,
    useCoinMetadata,
    useGetDelegatedStake,
    useGetValidatorsApy,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    useFormatCoin,
    formatPercentageDisplay,
    MIN_NUMBER_IOTA_TO_STAKE,
    Validator,
    getValidatorCommission,
    toast,
    useIsValidatorCommitteeMember,
    useIsActiveValidator,
    useGetNextEpochCommitteeMember,
} from '@iota/core';
import { Network, type StakeObject } from '@iota/iota-sdk/client';
import { NANOS_PER_IOTA, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';
import { useMemo } from 'react';
import { getDelegationDataByStakeId } from '../getDelegationByStakeId';
import {
    CardType,
    Panel,
    KeyValueInfo,
    Divider,
    Button,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    TooltipPosition,
    Badge,
    BadgeType,
} from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';
import { Warning } from '@iota/apps-ui-icons';
import { useIotaClientQuery } from '@iota/dapp-kit';

interface DelegationDetailCardProps {
    validatorAddress: string;
    stakedId: string;
}

export function DelegationDetailCard({ validatorAddress, stakedId }: DelegationDetailCardProps) {
    const navigate = useNavigate();
    const {
        data: system,
        isPending: loadingValidators,
        isError: errorValidators,
    } = useIotaClientQuery('getLatestIotaSystemState');

    const accountAddress = useActiveAddress();
    const {
        data: allDelegation,
        isPending,
        isError,
        error,
    } = useGetDelegatedStake({
        address: accountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const network = useAppSelector(({ app }) => app.network);
    const { data: coinBalance } = useBalance(accountAddress!);
    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const { isCommitteeMember } = useIsValidatorCommitteeMember();
    const { isActiveValidator } = useIsActiveValidator();
    const { isValidatorExpectedToBeInTheCommittee } =
        useGetNextEpochCommitteeMember(validatorAddress);
    // set minimum stake amount to 1 IOTA
    const showRequestMoreIotaToken = useMemo(() => {
        if (!coinBalance?.totalBalance || !metadata?.decimals || network === Network.Mainnet)
            return false;
        const currentBalance = new BigNumber(coinBalance.totalBalance);
        const minStakeAmount = new BigNumber(MIN_NUMBER_IOTA_TO_STAKE).shiftedBy(metadata.decimals);
        return currentBalance.lt(minStakeAmount.toString());
    }, [network, metadata?.decimals, coinBalance?.totalBalance]);

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validatorData = useMemo(() => {
        if (!system) return null;
        return system.activeValidators.find((av) => av.iotaAddress === validatorAddress);
    }, [validatorAddress, system]);

    const delegationData = useMemo(() => {
        return allDelegation ? getDelegationDataByStakeId(allDelegation, stakedId) : null;
    }, [allDelegation, stakedId]);

    const totalStake = BigInt(delegationData?.principal || 0n);
    const iotaEarned = BigInt(
        (delegationData as Extract<StakeObject, { estimatedReward: string }>)?.estimatedReward ||
            0n,
    );
    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: 0,
    };

    const [iotaEarnedFormatted, iotaEarnedSymbol] = useFormatCoin({ balance: iotaEarned });
    const [totalStakeFormatted, totalStakeSymbol] = useFormatCoin({ balance: totalStake });

    const delegationId = delegationData?.stakedIotaId;

    const stakeByValidatorAddress = `/stake/new?${new URLSearchParams({
        address: validatorAddress,
        staked: stakedId,
    }).toString()}`;

    const isValidatorCommitteeMember = isCommitteeMember(validatorAddress);
    const isValidatorActive = isActiveValidator(validatorAddress);
    const isActiveButNotInTheCommittee = isValidatorActive && !isValidatorCommitteeMember;

    if (isPending || loadingValidators) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isError || errorValidators) {
        toast.error(error?.message ?? 'An error occurred fetching validator information');
    }
    function handleAddNewStake() {
        navigate(stakeByValidatorAddress);
        ampli.clickedStakeIota({
            isCurrentlyStaking: true,
            sourceFlow: 'Delegation detail card',
        });
    }

    function handleUnstake() {
        navigate(stakeByValidatorAddress + '&unstake=true');
        ampli.clickedUnstakeIota({
            stakedAmount: Number(totalStake / NANOS_PER_IOTA),
            validatorAddress,
        });
    }

    return (
        <div className="flex h-full w-full flex-col justify-between">
            <div className="flex flex-col gap-y-md">
                <Validator address={validatorAddress} type={CardType.Filled} />
                {isActiveButNotInTheCommittee ? (
                    <InfoBox
                        type={InfoBoxType.Warning}
                        title="Validator is not earning rewards."
                        supportingText="Validator is active but not in the current committee, so not earning rewards this epoch. It may earn in future epochs. Stake at your discretion."
                        icon={<Warning />}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : !isValidatorActive ? (
                    <InfoBox
                        type={InfoBoxType.Error}
                        title="Inactive Validator is not earning rewards"
                        supportingText="This validator is inactive and will no longer earn rewards. Stake at your own risk."
                        icon={<Warning />}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : null}
                <Panel hasBorder>
                    <div className="flex flex-col gap-y-sm p-md">
                        <KeyValueInfo
                            keyText="Your Stake"
                            value={totalStakeFormatted}
                            supportingLabel={totalStakeSymbol}
                            fullwidth
                        />
                        <KeyValueInfo
                            keyText="Earned"
                            value={iotaEarnedFormatted}
                            supportingLabel={iotaEarnedSymbol}
                            fullwidth
                        />
                        <Divider />
                        <KeyValueInfo
                            keyText="APY"
                            value={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                            fullwidth
                        />
                        <KeyValueInfo
                            keyText="Commission"
                            value={getValidatorCommission(validatorData)}
                            fullwidth
                            tooltipText="The charge imposed by the validator for their staking services."
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                </Panel>
                {!isValidatorExpectedToBeInTheCommittee ? (
                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            <KeyValueInfo
                                keyText="Rewards next Epoch"
                                value={<Badge label="Not Earning" type={BadgeType.Warning} />}
                                fullwidth
                                tooltipPosition={TooltipPosition.Top}
                                tooltipText="Currently, the validator does not meet the criteria required to generate rewards in the next epoch, but this may change."
                            />
                        </div>
                    </Panel>
                ) : null}
            </div>
            <div className="flex w-full gap-2.5">
                {Boolean(totalStake) && delegationId && (
                    <Button
                        type={ButtonType.Secondary}
                        onClick={handleUnstake}
                        text="Unstake"
                        fullWidth
                    />
                )}
                {isValidatorActive ? (
                    <Button
                        type={ButtonType.Primary}
                        text="Stake"
                        onClick={handleAddNewStake}
                        disabled={showRequestMoreIotaToken}
                        fullWidth
                    />
                ) : null}
            </div>
        </div>
    );
}
