// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useGetDelegatedStake } from './stake';
import { useGetValidatorsApy } from './useGetValidatorsApy';
import {
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '../constants';
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

    const totalStakeFormatted = useFormatCoin(totalStake, IOTA_TYPE_ARG);
    const totalValidatorsStakeFormatted = useFormatCoin(totalValidatorStake, IOTA_TYPE_ARG);

    return {
        epoch: Number(system?.epoch) || 0,
        totalStake: totalStakeFormatted,
        totalValidatorsStake: totalValidatorsStakeFormatted,
        totalStakePercentage,
        validatorApy,
        systemDataResult,
        delegatedStakeDataResult,
    };
}
