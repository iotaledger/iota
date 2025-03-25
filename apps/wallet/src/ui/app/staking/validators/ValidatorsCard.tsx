// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Info, Warning } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonType,
    DisplayStats,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import {
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    formatDelegatedStake,
    getUniversalIotaSystemStateFields,
    StakedCard,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
} from '@iota/core';
import { useActiveAddress } from '_hooks';
import { ampli } from '_src/shared/analytics/ampli';
import { useMemo } from 'react';
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

    const { committeeMembers, epoch } = getUniversalIotaSystemStateFields();

    const delegatedStake = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    // Total active stake for all Staked validators
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStake);

    const [totalDelegatedStakeFormatted, symbol] = useFormatCoin({ balance: totalDelegatedStake });

    const delegations = useMemo(() => {
        return delegatedStakeData?.flatMap((delegation) => {
            return delegation.stakes.map((d) => ({
                ...d,
                // flag any inactive validator for the stakeIota object
                // if the stakingPoolId is not found in the activeValidators list flag as inactive
                inactiveValidator: !committeeMembers?.find(
                    ({ stakingPoolId }) => stakingPoolId === delegation.stakingPool,
                ),
                validatorAddress: delegation.validatorAddress,
            }));
        });
    }, [committeeMembers, delegatedStake]);

    // Check if there are any inactive validators
    const hasInactiveValidatorDelegation = delegations?.some(
        ({ inactiveValidator }) => inactiveValidator,
    );

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
                {hasInactiveValidatorDelegation ? (
                    <div className="mb-3">
                        <InfoBox
                            type={InfoBoxType.Default}
                            title="Earn with active validators"
                            supportingText="Unstake IOTA from the inactive validators and stake on an active
validator to start earning rewards again."
                            icon={<Info />}
                            style={InfoBoxStyle.Elevated}
                        />
                    </div>
                ) : null}
                <div className="w-full gap-2">
                    {epoch &&
                        delegations
                            ?.filter(({ inactiveValidator }) => inactiveValidator)
                            .map((delegation) => (
                                <StakedCard
                                    extendedStake={delegation}
                                    currentEpoch={Number(epoch)}
                                    key={delegation.stakedIotaId}
                                    inactiveValidator
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
                    {epoch &&
                        delegations
                            ?.filter(({ inactiveValidator }) => !inactiveValidator)
                            .map((delegation) => (
                                <StakedCard
                                    extendedStake={delegation}
                                    currentEpoch={Number(epoch)}
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
