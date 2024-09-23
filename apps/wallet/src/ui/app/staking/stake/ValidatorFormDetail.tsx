// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert, LoadingIndicator } from '_components';
import {
    calculateStakeShare,
    formatPercentageDisplay,
    useGetDelegatedStake,
    useGetValidatorsApy,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    useFormatCoin,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useActiveAddress } from '../../hooks/useActiveAddress';
import { getStakeIotaByIotaId } from '../getStakeIotaByIotaId';
import { getTokenStakeIotaForValidator } from '../getTokenStakeIotaForValidator';
import { KeyValueInfo, Panel, TooltipPosition } from '@iota/apps-ui-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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
    const [totalValidatorStakeFormatted, totalValidatorStakeSymbol] = useFormatCoin(
        totalValidatorStake,
        IOTA_TYPE_ARG,
    );
    const [totalStakeFormatted, totalStakeSymbol] = useFormatCoin(totalStake, IOTA_TYPE_ARG);

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
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Staking APY"
                        tooltipPosition={TooltipPosition.Right}
                        tooltipText="Annualized percentage yield based on past validator performance. Future APY may vary"
                        value={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        fullwidth
                    />
                    <KeyValueInfo
                        keyText="Stake Share"
                        tooltipPosition={TooltipPosition.Right}
                        tooltipText="Stake percentage managed by this validator."
                        value={formatPercentageDisplay(totalStakePercentage)}
                        fullwidth
                    />
                    {!unstake && (
                        <>
                            <KeyValueInfo
                                keyText="Total Staked"
                                tooltipPosition={TooltipPosition.Right}
                                tooltipText="Stake percentage managed by this validator."
                                value={totalValidatorStakeFormatted}
                                supportingLabel={totalValidatorStakeSymbol}
                                fullwidth
                            />
                            <KeyValueInfo
                                keyText="Your Staked IOTA"
                                tooltipPosition={TooltipPosition.Right}
                                tooltipText="Your current staked balance."
                                value={totalStakeFormatted}
                                supportingLabel={totalStakeSymbol}
                                fullwidth
                            />
                        </>
                    )}
                </div>
            </Panel>
        </div>
    );
}
