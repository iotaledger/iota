// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card } from '_app/shared/card';
import { Alert, LoadingIndicator } from '_components';
import { Text } from '_src/ui/app/shared/text';
import { IconTooltip } from '_src/ui/app/shared/tooltip';
import {
    calculateStakeShare,
    formatPercentageDisplay,
    useGetDelegatedStake,
    useGetValidatorsApy,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';

import { useActiveAddress } from '../../hooks/useActiveAddress';
import { getStakeIotaByIotaId } from '../getStakeIotaByIotaId';
import { getTokenStakeIotaForValidator } from '../getTokenStakeIotaForValidator';
import { StakeAmount } from '../home/StakeAmount';
import { ValidatorLogo } from '../validators/ValidatorLogo';

interface ValidatorFormDetailProps {
    validatorAddress: string;
    unstake?: boolean;
}

export function ValidatorFormDetail({ validatorAddress, unstake }: ValidatorFormDetailProps) {
    const accountAddress = useActiveAddress();

    const [searchParams] = useSearchParams();
    const stakeIdParams = searchParams.get('staked');
    const {
        data: system,
        isPending: loadingValidators,
        isError: errorValidators,
    } = useIotaClientQuery('getLatestIotaSystemState');

    const {
        data: stakeData,
        isPending,
        isError,
        error,
    } = useGetDelegatedStake({
        address: accountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validatorData = useMemo(() => {
        if (!system) return null;
        return system.activeValidators.find((av) => av.iotaAddress === validatorAddress);
    }, [validatorAddress, system]);

    //TODO: verify this is the correct validator stake balance
    const totalValidatorStake = validatorData?.stakingPoolIotaBalance || 0;

    const totalStake = useMemo(() => {
        if (!stakeData) return 0n;
        return unstake
            ? getStakeIotaByIotaId(stakeData, stakeIdParams)
            : getTokenStakeIotaForValidator(stakeData, validatorAddress);
    }, [stakeData, stakeIdParams, unstake, validatorAddress]);

    const totalValidatorsStake = useMemo(() => {
        if (!system) return 0;
        return system.activeValidators.reduce(
            (acc, curr) => (acc += BigInt(curr.stakingPoolIotaBalance)),
            0n,
        );
    }, [system]);

    const totalStakePercentage = useMemo(() => {
        if (!system || !validatorData) return null;

        return calculateStakeShare(
            BigInt(validatorData.stakingPoolIotaBalance),
            BigInt(totalValidatorsStake),
        );
    }, [system, totalValidatorsStake, validatorData]);

    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };

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
                    <div className="mb-1 font-semibold">
                        {error?.message ?? 'Error loading validator data'}
                    </div>
                </Alert>
            </div>
        );
    }

    return (
        <div className="w-full">
            {validatorData && (
                <Card
                    titleDivider
                    header={
                        <div className="flex items-center gap-2 px-3.75 py-2.5">
                            <ValidatorLogo
                                validatorAddress={validatorAddress}
                                iconSize="sm"
                                size="body"
                            />
                        </div>
                    }
                    footer={
                        !unstake && (
                            <>
                                <Text variant="body" weight="medium" color="steel-darker">
                                    Your Staked IOTA
                                </Text>

                                <StakeAmount balance={totalStake} variant="body" />
                            </>
                        )
                    }
                >
                    <div className="flex flex-col gap-3.5">
                        <div className="flex items-center justify-between gap-2">
                            <div className="text-steel flex items-center gap-1">
                                <Text variant="body" weight="medium" color="steel-darker">
                                    Staking APY
                                </Text>
                                <IconTooltip
                                    noFullWidth
                                    tip="This is the Annualized Percentage Yield of the a specific validator’s past operations. Note there is no guarantee this APY will be true in the future."
                                />
                            </div>

                            <Text variant="body" weight="semibold" color="gray-90">
                                {formatPercentageDisplay(apy, '--', isApyApproxZero)}
                            </Text>
                        </div>
                        <div className="flex items-center justify-between gap-2">
                            <div className="text-steel flex items-center gap-1">
                                <Text variant="body" weight="medium" color="steel-darker">
                                    Stake Share
                                </Text>
                                <IconTooltip
                                    noFullWidth
                                    tip="The percentage of total stake managed by this validator"
                                />
                            </div>

                            <Text variant="body" weight="semibold" color="gray-90">
                                {formatPercentageDisplay(totalStakePercentage)}
                            </Text>
                        </div>

                        {!unstake && (
                            <div className="mb-3.5 flex items-center justify-between gap-2">
                                <div className="text-steel flex items-center gap-1">
                                    <Text variant="body" weight="medium" color="steel-darker">
                                        Total Staked
                                    </Text>
                                    <IconTooltip
                                        noFullWidth
                                        tip="The total IOTA staked on the network by this validator and its delegators, to validate the network and earn rewards."
                                    />
                                </div>
                                <StakeAmount balance={totalValidatorStake} variant="body" />
                            </div>
                        )}
                    </div>
                </Card>
            )}
        </div>
    );
}
