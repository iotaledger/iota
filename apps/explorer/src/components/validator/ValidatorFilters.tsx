// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonSegment, SegmentedButton } from '@iota/apps-ui-kit';

export type ValidatorStatus = 'All' | 'Committee' | 'Active' | 'Pending' | 'At Risk';

interface ValidatorFiltersProps {
    selectedStatus: ValidatorStatus;
    onStatusChange: (status: ValidatorStatus) => void;
    validatorCounts: {
        all: number;
        committee: number;
        active: number;
        pending: number;
        atRisk: number;
    };
}

export function ValidatorFilters({
    selectedStatus,
    onStatusChange,
    validatorCounts,
}: ValidatorFiltersProps): JSX.Element {
    const options: { status: ValidatorStatus; count: number }[] = [
        { status: 'All', count: validatorCounts.all },
        { status: 'Committee', count: validatorCounts.committee },
        { status: 'Active', count: validatorCounts.active },
        { status: 'Pending', count: validatorCounts.pending },
        { status: 'At Risk', count: validatorCounts.atRisk },
    ];

    return (
        <SegmentedButton>
            {options.map(({ status, count }) => (
                <ButtonSegment
                    key={status}
                    label={`${status}  ${count}`}
                    selected={status === selectedStatus}
                    onClick={() => onStatusChange(status)}
                />
            ))}
        </SegmentedButton>
    );
}
