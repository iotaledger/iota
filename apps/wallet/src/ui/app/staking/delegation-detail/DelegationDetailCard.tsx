// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert, AlertStyle, AlertType, LoadingIndicator } from '_components';
import { useAppSelector } from '_hooks';
import { ampli } from '_src/shared/analytics/ampli';
import { MIN_NUMBER_IOTA_TO_STAKE } from '_src/shared/constants';
import {
    useBalance,
    useCoinMetadata,
    useGetDelegatedStake,
    useGetValidatorsApy,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    useFormatCoin,
    formatPercentageDisplay,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Network, type StakeObject } from '@iota/iota-sdk/client';
import { NANOS_PER_IOTA, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';
import { useMemo } from 'react';
import { useActiveAddress } from '../../hooks/useActiveAddress';
import { getDelegationDataByStakeId } from '../getDelegationByStakeId';
import { CardType, Panel, KeyValueInfo, Divider, Button, ButtonType } from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';
import { ValidatorLogo } from '../validators/ValidatorLogo';

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

    const [iotaEarnedFormatted, iotaEarnedSymbol] = useFormatCoin(iotaEarned, IOTA_TYPE_ARG);
    const [totalStakeFormatted, totalStakeSymbol] = useFormatCoin(totalStake, IOTA_TYPE_ARG);

    const delegationId = delegationData?.status === 'Active' && delegationData?.stakedIotaId;

    const stakeByValidatorAddress = `/stake/new?${new URLSearchParams({
        address: validatorAddress,
        staked: stakedId,
    }).toString()}`;

    // check if the validator is in the active validator list, if not, is inactive validator
    const hasInactiveValidatorDelegation = !system?.activeValidators?.find(
        ({ stakingPoolId }) => stakingPoolId === validatorData?.stakingPoolId,
    );

    const commission = validatorData ? Number(validatorData.commissionRate) / 100 : 0;

    if (isPending || loadingValidators) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isError || errorValidators) {
        return (
            <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                <Alert
                    title="Something went wrong"
                    supportingText={error?.message ?? 'An error occurred'}
                    style={AlertStyle.Default}
                    type={AlertType.Warning}
                />
            </div>
        );
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
                <ValidatorLogo validatorAddress={validatorAddress} type={CardType.Filled} />
                {hasInactiveValidatorDelegation ? (
                    <Alert
                        title="Earn with active validators"
                        supportingText="Unstake IOTA from the inactive validators and stake on an active
                                    validator to start earning rewards again."
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
                            value={`${commission.toString()}%`}
                            fullwidth
                        />
                    </div>
                </Panel>
            </div>
            <div className="my-3.75 flex w-full gap-2.5">
                {Boolean(totalStake) && delegationId && (
                    <Button
                        type={ButtonType.Secondary}
                        onClick={handleUnstake}
                        text="Unstake"
                        fullWidth
                    />
                )}
                {!hasInactiveValidatorDelegation ? (
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
