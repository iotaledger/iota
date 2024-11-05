// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useGetDelegatedStake } from './stake';
import { useGetValidatorsApy } from './useGetValidatorsApy';
import {
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '../constants';
import { useMemo } from 'react';
import { calculateStakeShare, getStakeIotaByIotaId, getTokenStakeIotaForValidator } from '../utils';
import { useFormatCoin } from './useFormatCoin';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface UseGetStakingValidatorDetailsArgs {
    accountAddress: string | null;
    stakeId: string | null;
    validatorAddress: string;
    unstake?: boolean;
}

export function useGetStakingValidatorDetails({
    accountAddress,
    stakeId,
    validatorAddress,
    unstake,
}: UseGetStakingValidatorDetailsArgs) {
    const systemDataResult = useIotaClientQuery('getLatestIotaSystemState');

    const delegatedStakeDataResult = useGetDelegatedStake({
        address: accountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const { data: rollingAverageApys } = useGetValidatorsApy();
    const { data: system } = systemDataResult;
    const { data: stakeData } = delegatedStakeDataResult;

    const validatorData = useMemo(() => {
        if (!system) return null;
        return system.activeValidators.find((av) => av.iotaAddress === validatorAddress);
    }, [validatorAddress, systemDataResult]);

    //TODO: verify this is the correct validator stake balance
    const totalValidatorStake = validatorData?.stakingPoolIotaBalance || 0;

    const totalStake = useMemo(() => {
        if (!stakeData) return 0n;
        return unstake
            ? getStakeIotaByIotaId(stakeData, stakeId)
            : getTokenStakeIotaForValidator(stakeData, validatorAddress);
    }, [stakeData, stakeId, unstake, validatorAddress]);

    const totalValidatorsStake = useMemo(() => {
        if (!system) return 0;
        return system.activeValidators.reduce(
            (acc, curr) => (acc += BigInt(curr.stakingPoolIotaBalance)),
            0n,
        );
    }, [systemDataResult]);

    const totalStakePercentage = useMemo(() => {
        if (!systemDataResult || !validatorData) return null;

        return calculateStakeShare(
            BigInt(validatorData.stakingPoolIotaBalance),
            BigInt(totalValidatorsStake),
        );
    }, [systemDataResult, totalValidatorsStake, validatorData]);

    const validatorApy = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
        isApyApproxZero: undefined,
    };

    return {
        epoch: Number(system?.epoch) ?? 0,
        totalStake: useFormatCoin(totalStake, IOTA_TYPE_ARG),
        totalValidatorsStake: useFormatCoin(totalValidatorStake, IOTA_TYPE_ARG),
        totalStakePercentage,
        validatorApy,
        systemDataResult,
        delegatedStakeDataResult,
    };
}
