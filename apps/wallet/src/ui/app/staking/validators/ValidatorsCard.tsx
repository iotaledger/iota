// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BottomMenuLayout, { Content, Menu } from '_app/shared/bottom-menu-layout';
import { Button } from '_app/shared/ButtonUI';
import { Card, CardItem } from '_app/shared/card';
import { Text } from '_app/shared/text';
import { Alert, LoadingIndicator } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import {
    formatDelegatedStake,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Plus12 } from '@iota/icons';
import { useMemo } from 'react';

import { useActiveAddress } from '../../hooks/useActiveAddress';
import { StakeAmount } from '../home/StakeAmount';
import { StakeCard } from '../home/StakedCard';

export function ValidatorsCard() {
    const accountAddress = useActiveAddress();
    const {
        data: delegatedStakeData,
        isPending,
        isError,
        error,
    } = useGetDelegatedStake({
        address: accountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const activeValidators = system?.activeValidators;
    const delegatedStake = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];

    // Total active stake for all Staked validators
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStake);

    const delegations = useMemo(() => {
        return delegatedStakeData?.flatMap((delegation) => {
            return delegation.stakes.map((d) => ({
                ...d,
                // flag any inactive validator for the stakeIota object
                // if the stakingPoolId is not found in the activeValidators list flag as inactive
                inactiveValidator: !activeValidators?.find(
                    ({ stakingPoolId }) => stakingPoolId === delegation.stakingPool,
                ),
                validatorAddress: delegation.validatorAddress,
            }));
        });
    }, [activeValidators, delegatedStake]);

    // Check if there are any inactive validators
    const hasInactiveValidatorDelegation = delegations?.some(
        ({ inactiveValidator }) => inactiveValidator,
    );

    // Get total rewards for all delegations
    const delegatedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedRewards = useTotalDelegatedRewards(delegatedStakes);

    const numberOfValidators = delegatedStakeData?.length || 0;

    if (isPending) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isError) {
        return (
            <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                <Alert>
                    <strong>{error?.message}</strong>
                </Alert>
            </div>
        );
    }

    return (
        <div className="flex h-full w-full flex-col flex-nowrap">
            <BottomMenuLayout>
                <Content>
                    <div className="mb-4">
                        {hasInactiveValidatorDelegation ? (
                            <div className="mb-3">
                                <Alert>
                                    Unstake IOTA from the inactive validators and stake on an active
                                    validator to start earning rewards again.
                                </Alert>
                            </div>
                        ) : null}
                        <div className="mb-4 grid grid-cols-2 gap-2.5">
                            {system &&
                                delegations
                                    ?.filter(({ inactiveValidator }) => inactiveValidator)
                                    .map((delegation) => (
                                        <StakeCard
                                            extendedStake={delegation}
                                            currentEpoch={Number(system.epoch)}
                                            key={delegation.stakedIotaId}
                                            inactiveValidator
                                        />
                                    ))}
                        </div>
                        <Card
                            padding="none"
                            header={
                                <div className="flex w-full justify-center px-3.75 py-2.5">
                                    <Text
                                        variant="captionSmall"
                                        weight="semibold"
                                        color="steel-darker"
                                    >
                                        Staking on {numberOfValidators}
                                        {numberOfValidators > 1 ? ' Validators' : ' Validator'}
                                    </Text>
                                </div>
                            }
                        >
                            <div className="divide-gray-45 flex divide-x divide-y-0 divide-solid">
                                <CardItem title="Your Stake">
                                    <StakeAmount balance={totalDelegatedStake} variant="heading5" />
                                </CardItem>
                                <CardItem title="Earned">
                                    <StakeAmount
                                        balance={totalDelegatedRewards}
                                        variant="heading5"
                                        isEarnedRewards
                                    />
                                </CardItem>
                            </div>
                        </Card>

                        <div className="mt-4 grid grid-cols-2 gap-2.5">
                            {system &&
                                delegations
                                    ?.filter(({ inactiveValidator }) => !inactiveValidator)
                                    .map((delegation) => (
                                        <StakeCard
                                            extendedStake={delegation}
                                            currentEpoch={Number(system.epoch)}
                                            key={delegation.stakedIotaId}
                                        />
                                    ))}
                        </div>
                    </div>
                </Content>
                <Menu stuckClass="staked-cta" className="mx-0 w-full px-0 pb-0">
                    <Button
                        size="tall"
                        variant="secondary"
                        to="new"
                        onClick={() =>
                            ampli.clickedStakeIota({
                                isCurrentlyStaking: true,
                                sourceFlow: 'Validator card',
                            })
                        }
                        before={<Plus12 />}
                        text="Stake IOTA"
                    />
                </Menu>
            </BottomMenuLayout>
        </div>
    );
}
