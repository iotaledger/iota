// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO: This component really shouldn't use the `Tabs` component, it should just use radix,
// and should define it's own styles since the concerns here are pretty different.

import { ButtonSegment, SegmentedButton, Chip } from '@iota/apps-ui-kit';

export interface FilterListProps<T extends string = string> {
    selected: T;
    options: readonly T[];
    onSelected(value: T): void;
    filtersAsChip?: boolean;
}

export function FilterList<T extends string>({
    options,
    selected,
    onSelected,
    filtersAsChip,
}: FilterListProps<T>): JSX.Element {
    const FilterComponent = filtersAsChip ? Chip : ButtonSegment;

    return (
        <SegmentedButton>
            {options.map((option) => (
                <FilterComponent
                    key={option}
                    label={option}
                    selected={option == selected}
                    onClick={() => onSelected(option)}
                />
            ))}
        </SegmentedButton>
    );
}
