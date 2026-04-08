// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { getInactiveValidatorsMetadata } from '../../utils';

export function useGetValidatorCandidate(validatorAddress: string) {
    const iotaClient = useIotaClient();
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const validatorCandidatesId = data?.validatorCandidatesId;
    return useQuery({
        queryKey: ['validator-candidates', validatorCandidatesId],
        async queryFn() {
            if (!validatorCandidatesId) {
                throw Error('Missing params');
            }

            const candidateValidators = await iotaClient.getDynamicFields({
                parentId: normalizeIotaAddress(validatorCandidatesId),
            });
            return Promise.all(
                candidateValidators.data.map((validator) =>
                    getInactiveValidatorsMetadata(iotaClient, validator.objectId),
                ),
            );
        },
        select(data) {
            return data.find((v) => v?.validatorAddress === validatorAddress) ?? null;
        },
        enabled: !!validatorCandidatesId,
    });
}
