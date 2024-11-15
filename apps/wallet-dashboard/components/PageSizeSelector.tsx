// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';
import { Select, SelectSize } from '@iota/apps-ui-kit';

interface PageSizeSelectorProps {
    limit: string;
    setLimit: (limit: number) => void;
    pagination: {
        onFirst: () => void;
    };
    range: number[];
    dropdownPosition?: React.ComponentProps<typeof Select>['dropdownPosition'];
}
export function PageSizeSelector({
    limit,
    setLimit,
    pagination,
    range,
    dropdownPosition,
}: PageSizeSelectorProps): React.JSX.Element {
    return (
        <Select
            value={limit.toString()}
            options={range.map((size) => ({
                label: `${size} / page`,
                id: size.toString(),
            }))}
            size={SelectSize.Small}
            dropdownPosition={dropdownPosition}
            onValueChange={(e) => {
                setLimit(Number(e));
                pagination.onFirst();
            }}
        />
    );
}
