// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { getValidatorCandidateObjects, sanitizeValidatorObjects } from '~/lib';
import type { IotaValidatorSummaryExtended } from '~/lib/types';

export function useGetValidatorCandidates(validatorAddress?: string) {
    const iotaClient = useIotaClient();
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    const validatorCandidatesId = systemState?.validatorCandidatesId;

    const {
        data: candidateObjects,
        isPending,
        isLoading,
        isError,
    } = useQuery({
        queryKey: ['validator-candidate-objects', validatorCandidatesId, iotaClient],
        async queryFn() {
            if (!validatorCandidatesId) {
                throw Error('Missing validatorCandidatesId');
            }
            return getValidatorCandidateObjects(iotaClient, validatorCandidatesId);
        },
        enabled: !!validatorCandidatesId,
    });

    const allCandidates: IotaValidatorSummaryExtended[] = sanitizeValidatorObjects(
        candidateObjects,
        { isCandidate: true },
    );

    const data = validatorAddress
        ? allCandidates.filter((v) => v.iotaAddress === validatorAddress)
        : allCandidates;

    return {
        data,
        isPending,
        isLoading,
        isError,
    };
}
