// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetInactiveValidator, useGetValidatorsApy } from '..';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { InactiveValidatorData } from '../../types';

export function useValidatorInfo({ validatorAddress }: { validatorAddress: string }) {
    const {
        data: system,
        isPending: isPendingValidators,
        isError: errorValidators,
    } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: rollingAverageApys } = useGetValidatorsApy();
    const validatorSummary =
        system?.activeValidators.find((validator) => validator.iotaAddress === validatorAddress) ||
        null;
    const inactiveValidatorData = useGetInactiveValidator(validatorAddress);
    let inactiveValidatorSummary: InactiveValidatorData | null = null;
    if (validatorSummary === null && inactiveValidatorData !== null) {
        inactiveValidatorSummary = {
            name: inactiveValidatorData?.name || '',
            validatorAddress: inactiveValidatorData?.validatorAddress || '',
            validatorStakingPoolId: inactiveValidatorData?.validatorStakingPoolId || '',
            validatorPublicKey: inactiveValidatorData?.validatorPublicKey || '',
            imageUrl: inactiveValidatorData?.imageUrl || '',
            description: inactiveValidatorData?.description || '',
            projectUrl: inactiveValidatorData?.projectUrl || '',
        };
    }

    const currentEpoch = Number(system?.epoch || 0);
    const stakingPoolActivationEpoch = Number(validatorSummary?.stakingPoolActivationEpoch || 0);

    // flag as new validator if the validator was activated in the last epoch
    // for genesis validators, this will be false
    const newValidator = currentEpoch - stakingPoolActivationEpoch <= 1 && currentEpoch !== 0;

    // flag if the validator is at risk of being removed from the active set
    const isAtRisk = system?.atRiskValidators.some((item) => item[0] === validatorAddress);

    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };

    const commission = validatorSummary ? Number(validatorSummary.commissionRate) / 100 : 0;

    return {
        system,
        isPendingValidators,
        errorValidators,
        currentEpoch,
        validatorSummary,
        inactiveValidatorSummary,
        name: validatorSummary?.name || '',
        stakingPoolActivationEpoch,
        commission,
        newValidator,
        isAtRisk,
        apy,
        isApyApproxZero,
    };
}
