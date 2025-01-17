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
import { useIotaClientQuery } from '@iota/dapp-kit';
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
import { Info, Warning } from '@iota/apps-ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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

    const [totalDelegatedStakeFormatted, symbol] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );

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
    const [totalDelegatedRewardsFormatted] = useFormatCoin(totalDelegatedRewards, IOTA_TYPE_ARG);

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
                    {system &&
                        delegations
                            ?.filter(({ inactiveValidator }) => inactiveValidator)
                            .map((delegation) => (
                                <StakedCard
                                    extendedStake={delegation}
                                    currentEpoch={Number(system.epoch)}
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
                    {system &&
                        delegations
                            ?.filter(({ inactiveValidator }) => !inactiveValidator)
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
