// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useGetValidatorsApy } from '..';
import { getUniversalIotaSystemStateFields } from '../../utils';

export function useValidatorInfo({ validatorAddress }: { validatorAddress: string }) {
    const {
        epoch,
        atRiskValidators,
        committeeMembers,
        isPending: isPendingValidators,
        isError: errorValidators,
    } = getUniversalIotaSystemStateFields();
    const { data: rollingAverageApys } = useGetValidatorsApy();

    const validatorSummary =
        committeeMembers.find((validator) => validator.iotaAddress === validatorAddress) || null;

    const currentEpoch = Number(epoch || 0);

    const stakingPoolActivationEpoch = Number(validatorSummary?.stakingPoolActivationEpoch || 0);

    // flag as new validator if the validator was activated in the last epoch
    // for genesis validators, this will be false
    const newValidator = currentEpoch - stakingPoolActivationEpoch <= 1 && currentEpoch !== 0;

    // flag if the validator is at risk of being removed from the active set
    const isAtRisk = atRiskValidators.some((item) => item[0] === validatorAddress);

    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };

    const commission = validatorSummary ? Number(validatorSummary.commissionRate) / 100 : 0;

    return {
        epoch,
        isPendingValidators,
        errorValidators,
        currentEpoch,
        validatorSummary,
        name: validatorSummary?.name || '',
        stakingPoolActivationEpoch,
        commission,
        newValidator,
        isAtRisk,
        apy,
        isApyApproxZero,
    };
}
