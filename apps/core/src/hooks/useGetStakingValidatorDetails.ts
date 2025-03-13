// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useGetDelegatedStake } from './stake';
import { useGetValidatorsApy } from './useGetValidatorsApy';
import {
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '../constants';
import {
    calculateStakeShare,
    getStakeIotaByIotaId,
    getTokenStakeIotaForValidator,
    getValidatorCommission,
} from '../utils';
import { useFormatCoin } from './useFormatCoin';

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

    const validatorData = system?.activeValidators.find(
        (av) => av.iotaAddress === validatorAddress,
    );

    //TODO: verify this is the correct validator stake balance
    const totalValidatorStake = validatorData?.stakingPoolIotaBalance || 0;

    const totalStake = !stakeData
        ? 0n
        : unstake
          ? getStakeIotaByIotaId(stakeData, stakeId)
          : getTokenStakeIotaForValidator(stakeData, validatorAddress);

    const totalValidatorsStake =
        system?.activeValidators.reduce(
            (acc, curr) => (acc += BigInt(curr.stakingPoolIotaBalance)),
            0n,
        ) ?? 0n;

    const totalStakePercentage =
        !systemDataResult || !validatorData
            ? null
            : calculateStakeShare(
                  BigInt(validatorData.stakingPoolIotaBalance),
                  BigInt(totalValidatorsStake),
              );

    const validatorApy = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
        isApyApproxZero: undefined,
    };

    const totalStakeFormatted = useFormatCoin({ balance: totalStake });
    const totalValidatorsStakeFormatted = useFormatCoin({ balance: totalValidatorStake });

    return {
        epoch: Number(system?.epoch) || 0,
        totalStake: totalStakeFormatted,
        totalStakeOriginal: totalStake,
        totalValidatorsStake: totalValidatorsStakeFormatted,
        totalStakePercentage,
        validatorApy,
        systemDataResult,
        delegatedStakeDataResult,
        commission: getValidatorCommission(validatorData),
    };
}
