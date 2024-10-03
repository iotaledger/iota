// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO: This component really shouldn't use the `Tabs` component, it should just use radix,
// and should define it's own styles since the concerns here are pretty different.

import { ButtonSegment, SegmentedButton } from '@iota/apps-ui-kit';

export interface FilterListProps<T extends string = string> {
    selected: T;
    options: readonly T[];
    onSelected(value: T): void;
}

export function FilterList<T extends string>({
    options,
    selected,
    onSelected,
}: FilterListProps<T>): JSX.Element {
    return (
        <SegmentedButton>
            {options.map((option) => (
                <ButtonSegment
                    key={option}
                    label={option}
                    selected={option == selected}
                    onClick={() => onSelected(option)}
                />
            ))}
        </SegmentedButton>
    );
}
