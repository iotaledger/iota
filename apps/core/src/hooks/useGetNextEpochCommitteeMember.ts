// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { useMaxCommitteeSize } from './useMaxCommitteeSize';

export function useGetNextEpochCommitteeMember(validatorAddress: string): {
    isValidatorExpectedToBeInTheCommittee: boolean;
    isLoading: boolean;
} {
    const { data: systemState, isLoading: isSystemStateLoading } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );
    const { data: maxCommitteeSize, isLoading: isMaxCommitteeSizeLoading } = useMaxCommitteeSize();

    const isLoading = isSystemStateLoading || isMaxCommitteeSizeLoading;

    const isValidatorExpectedToBeInTheCommittee = useMemo(() => {
        if (!systemState || !maxCommitteeSize) return false;

        const sortedActiveValidatorsByTotalStaked = [...systemState.activeValidators].sort(
            (a, b) => Number(b.stakingPoolIotaBalance) - Number(a.stakingPoolIotaBalance),
        );

        return sortedActiveValidatorsByTotalStaked
            .slice(0, maxCommitteeSize)
            .some((v) => v.iotaAddress === validatorAddress);
    }, [systemState, maxCommitteeSize, validatorAddress]);

    return { isValidatorExpectedToBeInTheCommittee, isLoading };
}
