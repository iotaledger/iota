// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { getInactiveValidatorsMetadata } from '../../utils';
import type { InactiveValidatorData } from '../../types';

export function useGetInactiveValidator(validatorAddress: string) {
    const iotaClient = useIotaClient();
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const inactivePoolsId = data?.inactivePoolsId;
    const validatorCandidatesId = data?.validatorCandidatesId;
    return useQuery({
        queryKey: ['inactive-validators', inactivePoolsId, validatorCandidatesId],
        async queryFn() {
            if (!inactivePoolsId && !validatorCandidatesId) {
                throw Error('Missing params');
            }

            const results: (InactiveValidatorData | null)[] = [];

            if (inactivePoolsId) {
                const inactiveValidators = await iotaClient.getDynamicFields({
                    parentId: normalizeIotaAddress(inactivePoolsId),
                });
                const inactiveData = await Promise.all(
                    inactiveValidators.data.map((validator) =>
                        getInactiveValidatorsMetadata(iotaClient, validator.objectId),
                    ),
                );
                results.push(...inactiveData);
            }

            if (validatorCandidatesId) {
                const candidateValidators = await iotaClient.getDynamicFields({
                    parentId: normalizeIotaAddress(validatorCandidatesId),
                });
                const candidateData = await Promise.all(
                    candidateValidators.data.map((validator) =>
                        getInactiveValidatorsMetadata(iotaClient, validator.objectId),
                    ),
                );
                results.push(...candidateData);
            }

            return results;
        },
        select(data) {
            return data.find((v) => v?.validatorAddress === validatorAddress) ?? null;
        },
        enabled: !!inactivePoolsId || !!validatorCandidatesId,
    });
}
