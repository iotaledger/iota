import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { useMaxCommitteeSize } from './useMaxCommitteeSize';

export function useGetNextEpochCommitteeMember(validatorAddress: string): {
    isValidatorExpectedToBeInTheCommittee: boolean;
    isValidatorExpectedToBeInTheCommitteeLoading: boolean;
} {
    const { data: systemState, isLoading: isSystemStateLoading } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );
    const { data: maxCommitteeSize, isLoading: isMaxCommitteeSizeLoading } = useMaxCommitteeSize();

    const isValidatorExpectedToBeInTheCommitteeLoading =
        isSystemStateLoading || isMaxCommitteeSizeLoading;

    const isValidatorExpectedToBeInTheCommittee = useMemo(() => {
        if (!systemState || !maxCommitteeSize) return false;

        // Sort and slice only if data is available
        const sortedActiveValidatorsByTotalStaked = [...systemState.activeValidators].sort(
            (a, b) => Number(b.stakingPoolIotaBalance) - Number(a.stakingPoolIotaBalance),
        );

        return sortedActiveValidatorsByTotalStaked
            .slice(0, maxCommitteeSize)
            .some((v) => v.iotaAddress === validatorAddress);
    }, [systemState, maxCommitteeSize, validatorAddress]);

    return { isValidatorExpectedToBeInTheCommittee, isValidatorExpectedToBeInTheCommitteeLoading };
}
