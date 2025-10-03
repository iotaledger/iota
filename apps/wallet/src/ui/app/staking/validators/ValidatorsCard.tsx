// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import {
    formatDelegatedStake,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    useFormatCoin,
    StakedCard,
} from '@iota/core';
import { useMemo } from 'react';
import { useActiveAddress } from '_hooks';
import {
    Title,
    TitleSize,
    Button,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    DisplayStats,
} from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';
import { Warning } from '@iota/apps-ui-icons';
import { useIotaClientQuery } from '@iota/dapp-kit';

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
    const committeeMembers = system?.committeeMembers;
    const activeValidators = system?.activeValidators;
    const delegatedStake = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];

    // Total active stake for all Staked validators
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStake);

    const [totalDelegatedStakeFormatted, symbol] = useFormatCoin({ balance: totalDelegatedStake });

    const delegations = useMemo(() => {
        return delegatedStakeData?.flatMap((delegation) => {
            const isInCommittee = committeeMembers?.find(
                (member) => member.stakingPoolId === delegation.stakingPool,
            );
            const isActive = activeValidators?.find(
                (validator) => validator.stakingPoolId === delegation.stakingPool,
            );
            return delegation.stakes.map((d) => ({
                ...d,
                // flag any inactive validator for the stakeIota object
                // if the stakingPoolId is not found in the committeeMembers list flag as inactive
                activeButNotInTheCommittee: !isInCommittee && isActive,
                inactiveValidator: !isActive,
                validatorAddress: delegation.validatorAddress,
            }));
        });
    }, [committeeMembers, delegatedStake]);

    // Get total rewards for all delegations
    const delegatedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedRewards = useTotalDelegatedRewards(delegatedStakes);
    const [totalDelegatedRewardsFormatted] = useFormatCoin({ balance: totalDelegatedRewards });

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
                <InfoBox
                    type={InfoBoxType.Error}
                    title="Something went wrong"
                    supportingText={error?.message ?? 'An error occurred'}
                    icon={<Warning />}
                    style={InfoBoxStyle.Default}
                />
            </div>
        );
    }

    return (
        <div className="flex h-full w-full flex-col flex-nowrap">
            <div className="flex gap-xs py-md">
                <DisplayStats
                    label="Your stake"
                    value={totalDelegatedStakeFormatted}
                    supportingLabel={symbol}
                />
                <DisplayStats
                    label="Earned"
                    value={totalDelegatedRewardsFormatted}
                    supportingLabel={symbol}
                />
            </div>
            <Title title="In progress" size={TitleSize.Small} />
            <div className="flex max-h-[420px] w-full flex-1 flex-col items-start overflow-auto">
                <div className="w-full gap-2">
                    {system &&
                        delegations
                            ?.filter(({ inactiveValidator }) => inactiveValidator)
                            .map((delegation) => (
                                <div className="w-full gap-2" key={delegation.stakedIotaId}>
                                    <StakedCard
                                        extendedStake={delegation}
                                        inactiveValidator
                                        currentEpoch={Number(system.epoch)}
                                        onClick={() =>
                                            navigate(
                                                `/stake/delegation-detail?${new URLSearchParams({
                                                    validator: delegation.validatorAddress,
                                                    staked: delegation.stakedIotaId,
                                                }).toString()}`,
                                            )
                                        }
                                    />
                                </div>
                            ))}
                </div>
                <div className="w-full gap-2">
                    {system &&
                        delegations
                            ?.filter(({ activeButNotInTheCommittee }) => activeButNotInTheCommittee)
                            .map((delegation) => (
                                <StakedCard
                                    extendedStake={delegation}
                                    currentEpoch={Number(system.epoch)}
                                    key={delegation.stakedIotaId}
                                    activeButNotInTheCommittee
                                    onClick={() =>
                                        navigate(
                                            `/stake/delegation-detail?${new URLSearchParams({
                                                validator: delegation.validatorAddress,
                                                staked: delegation.stakedIotaId,
                                            }).toString()}`,
                                        )
                                    }
                                />
                            ))}
                </div>

                <div className="w-full gap-2">
                    {system &&
                        delegations
                            ?.filter(
                                ({ activeButNotInTheCommittee, inactiveValidator }) =>
                                    !activeButNotInTheCommittee && !inactiveValidator,
                            )
                            .map((delegation) => (
                                <StakedCard
                                    extendedStake={delegation}
                                    currentEpoch={Number(system.epoch)}
                                    key={delegation.stakedIotaId}
                                    onClick={() =>
                                        navigate(
                                            `/stake/delegation-detail?${new URLSearchParams({
                                                validator: delegation.validatorAddress,
                                                staked: delegation.stakedIotaId,
                                            }).toString()}`,
                                        )
                                    }
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
