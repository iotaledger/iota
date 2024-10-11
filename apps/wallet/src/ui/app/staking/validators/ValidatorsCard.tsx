// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert } from '_components';
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
import { useMemo } from 'react';
import { useActiveAddress } from '../../hooks/useActiveAddress';
import { StakeCard } from '../home/StakedCard';
import { StatsDetail } from '_app/staking/validators/StatsDetail';
import { Title, TitleSize, Button, ButtonType, LoadingIndicator } from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';

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
    const navigate = useNavigate();

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

    const handleNewStake = () => {
        ampli.clickedStakeIota({
            isCurrentlyStaking: true,
            sourceFlow: 'Validator card',
        });
        navigate('new');
    };

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
            <div className="flex gap-xs py-md">
                <StatsDetail title="Your stake" balance={totalDelegatedStake} />
                <StatsDetail title="Earned" balance={totalDelegatedRewards} />
            </div>
            <Title title="In progress" size={TitleSize.Small} />
            <div className="flex max-h-[420px] w-full flex-1 flex-col items-start overflow-auto">
                {hasInactiveValidatorDelegation ? (
                    <div className="mb-3">
                        <Alert>
                            Unstake IOTA from the inactive validators and stake on an active
                            validator to start earning rewards again.
                        </Alert>
                    </div>
                ) : null}
                <div className="gap-2">
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

                <div className="gap-2">
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
            <div className="pt-md">
                <Button fullWidth type={ButtonType.Primary} text="Stake" onClick={handleNewStake} />
            </div>
        </div>
    );
}
