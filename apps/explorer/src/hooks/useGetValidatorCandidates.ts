// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { getValidatorCandidateObjects, sanitizeValidatorObjects } from '~/lib';
import type { IotaValidatorSummaryExtended } from '~/lib/types';

interface ValidatorCandidatesResult<T> {
    data: T;
    isPending: boolean;
    isLoading: boolean;
    isError: boolean;
}

export function useGetValidatorCandidates(
    validatorAddress: string,
): ValidatorCandidatesResult<IotaValidatorSummaryExtended | null>;
export function useGetValidatorCandidates(): ValidatorCandidatesResult<
    IotaValidatorSummaryExtended[]
>;
export function useGetValidatorCandidates(validatorAddress?: string) {
    const iotaClient = useIotaClient();
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    const validatorCandidatesId = systemState?.validatorCandidatesId;

    const { data, isPending, isLoading, isError } = useQuery({
        queryKey: ['validator-candidate-objects', validatorCandidatesId, iotaClient],
        async queryFn() {
            if (!validatorCandidatesId) {
                throw Error('Missing validatorCandidatesId');
            }
            return getValidatorCandidateObjects(iotaClient, validatorCandidatesId);
        },
        enabled: !!validatorCandidatesId,
        select(candidateObjects) {
            const allCandidates = sanitizeValidatorObjects(candidateObjects, {
                isCandidate: true,
            });
            return validatorAddress
                ? (allCandidates.find((v) => v.iotaAddress === validatorAddress) ?? null)
                : allCandidates;
        },
    });

    return {
        data,
        isPending,
        isLoading,
        isError,
    };
}
