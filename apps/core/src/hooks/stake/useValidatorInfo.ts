// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetValidatorsApy } from '..';
import { useQuery } from '@tanstack/react-query';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { getInactiveValidatorsData } from '../../utils';
import { InactiveValidatorData } from '../../types';

export function useValidatorInfo({ validatorAddress }: { validatorAddress: string }) {
    const {
        data: system,
        isPending: isPendingValidators,
        isError: errorValidators,
    } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: rollingAverageApys } = useGetValidatorsApy();
    const iotaClient = useIotaClient();
    const validatorSummary =
        system?.activeValidators.find((validator) => validator.iotaAddress === validatorAddress) ||
        null;
    const { data: inactiveValidatorData } = useQuery({
        queryKey: [system?.inactivePoolsId, validatorAddress],
        async queryFn() {
            if (!system?.inactivePoolsId || !validatorAddress) {
                throw Error('Missing params');
            }
            const inactiveValidators = await iotaClient.getDynamicFields({
                parentId: normalizeIotaAddress(system?.inactivePoolsId),
            });

            const pendingInactiveValidatorsData = await Promise.all(
                inactiveValidators.data.map(
                    async (validator) =>
                        await getInactiveValidatorsData(iotaClient, validator.objectId),
                ),
            );

            return pendingInactiveValidatorsData;
        },
        enabled: !!system?.inactivePoolsId && !!validatorAddress,
        select(validators) {
            return validators.find((validator) => validator?.validatorAddress === validatorAddress);
        },
    });
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
