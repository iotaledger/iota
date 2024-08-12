// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BottomMenuLayout, { Content } from '_app/shared/bottom-menu-layout';
import { Button } from '_app/shared/ButtonUI';
import { Card } from '_app/shared/card';
import { CardItem } from '_app/shared/card/CardItem';
import { Text } from '_app/shared/text';
import { IconTooltip } from '_app/shared/tooltip';
import { Alert, LoadingIndicator } from '_components';
import { useAppSelector } from '_hooks';
import { ampli } from '_src/shared/analytics/ampli';
import { MIN_NUMBER_IOTA_TO_STAKE } from '_src/shared/constants';
import FaucetRequestButton from '_src/ui/app/shared/faucet/FaucetRequestButton';
import {
    useBalance,
    useCoinMetadata,
    useGetDelegatedStake,
    useGetValidatorsApy,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { ArrowLeft16, StakeAdd16, StakeRemove16 } from '@iota/icons';
import { Network, type StakeObject } from '@iota/iota.js/client';
import { MICROS_PER_IOTA, IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import BigNumber from 'bignumber.js';
import { useMemo } from 'react';

import { useActiveAddress } from '../../hooks/useActiveAddress';
import { Heading } from '../../shared/heading';
import { getDelegationDataByStakeId } from '../getDelegationByStakeId';
import { StakeAmount } from '../home/StakeAmount';

interface DelegationDetailCardProps {
    validatorAddress: string;
    stakedId: string;
}

export function DelegationDetailCard({ validatorAddress, stakedId }: DelegationDetailCardProps) {
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
            <div className="p-2">
                <Alert>
                    <div className="mb-1 font-semibold">Something went wrong</div>
                </Alert>
            </div>
        );
    }

    return (
        <div className="flex h-full flex-grow flex-col flex-nowrap">
            <BottomMenuLayout>
                <Content>
                    <div className="flex w-full flex-col items-center justify-center">
                        {hasInactiveValidatorDelegation ? (
                            <div className="mb-3">
                                <Alert>
                                    Unstake IOTA from this inactive validator and stake on an active
                                    validator to start earning rewards again.
                                </Alert>
                            </div>
                        ) : null}
                        <div className="flex w-full">
                            <Card
                                header={
                                    <div className="divide-gray-45 grid w-full grid-cols-2 divide-x divide-y-0 divide-solid">
                                        <CardItem title="Your Stake">
                                            <StakeAmount balance={totalStake} variant="heading5" />
                                        </CardItem>

                                        <CardItem title="Earned">
                                            <StakeAmount
                                                balance={iotaEarned}
                                                variant="heading5"
                                                isEarnedRewards
                                            />
                                        </CardItem>
                                    </div>
                                }
                                padding="none"
                            >
                                <div className="divide-gray-45 flex divide-x divide-y-0 divide-solid">
                                    <CardItem
                                        title={
                                            <div className="text-steel-darker flex items-start gap-1">
                                                APY
                                                <div className="text-steel">
                                                    <IconTooltip
                                                        tip="Annual Percentage Yield"
                                                        placement="top"
                                                    />
                                                </div>
                                            </div>
                                        }
                                    >
                                        <div className="flex items-baseline gap-0.5">
                                            <Heading
                                                variant="heading4"
                                                weight="semibold"
                                                color="gray-90"
                                                leading="none"
                                            >
                                                {isApyApproxZero ? '~' : ''}
                                                {apy}
                                            </Heading>

                                            <Text
                                                variant="subtitleSmall"
                                                weight="medium"
                                                color="steel-dark"
                                            >
                                                %
                                            </Text>
                                        </div>
                                    </CardItem>

                                    <CardItem
                                        title={
                                            <div className="text-steel-darker flex gap-1">
                                                Commission
                                                <div className="text-steel">
                                                    <IconTooltip
                                                        tip="Validator commission"
                                                        placement="top"
                                                    />
                                                </div>
                                            </div>
                                        }
                                    >
                                        <div className="flex items-baseline gap-0.5">
                                            <Heading
                                                variant="heading4"
                                                weight="semibold"
                                                color="gray-90"
                                                leading="none"
                                            >
                                                {commission}
                                            </Heading>

                                            <Text
                                                variant="subtitleSmall"
                                                weight="medium"
                                                color="steel-dark"
                                            >
                                                %
                                            </Text>
                                        </div>
                                    </CardItem>
                                </div>
                            </Card>
                        </div>
                        <div className="my-3.75 flex w-full gap-2.5">
                            {!hasInactiveValidatorDelegation ? (
                                <Button
                                    size="tall"
                                    variant="outline"
                                    to={stakeByValidatorAddress}
                                    before={<StakeAdd16 />}
                                    text="Stake IOTA"
                                    onClick={() => {
                                        ampli.clickedStakeIota({
                                            isCurrentlyStaking: true,
                                            sourceFlow: 'Delegation detail card',
                                        });
                                    }}
                                    disabled={showRequestMoreIotaToken}
                                />
                            ) : null}

                            {Boolean(totalStake) && delegationId && (
                                <Button
                                    data-testid="unstake-button"
                                    size="tall"
                                    variant="outline"
                                    to={stakeByValidatorAddress + '&unstake=true'}
                                    onClick={() => {
                                        ampli.clickedUnstakeIota({
                                            stakedAmount: Number(totalStake / MICROS_PER_IOTA),
                                            validatorAddress,
                                        });
                                    }}
                                    text="Unstake IOTA"
                                    before={<StakeRemove16 />}
                                />
                            )}
                        </div>
                    </div>
                </Content>

                {/* show faucet request button on devnet or testnet whenever there is only one coin  */}
                {showRequestMoreIotaToken ? (
                    <div className="flex flex-col items-center gap-4">
                        <div className="w-8/12 text-center">
                            <Text variant="pSubtitle" weight="medium" color="steel-darker">
                                You need a minimum of {MIN_NUMBER_IOTA_TO_STAKE} IOTA to continue
                                staking.
                            </Text>
                        </div>
                        <FaucetRequestButton size="tall" />
                    </div>
                ) : (
                    <Button
                        size="tall"
                        variant="secondary"
                        to="/stake"
                        before={<ArrowLeft16 />}
                        text="Back"
                    />
                )}
            </BottomMenuLayout>
        </div>
    );
}
