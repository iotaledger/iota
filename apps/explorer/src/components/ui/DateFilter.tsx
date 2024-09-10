// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';

import { FilterList } from './FilterList';

export type DateFilterOption = 'D' | 'W' | 'M' | 'ALL';

export function useDateFilterState(
    defaultFilter: DateFilterOption,
): [DateFilterOption, (value: DateFilterOption) => void] {
    return useState(defaultFilter);
}

export interface DateFilterProps {
    options?: DateFilterOption[];
    value: DateFilterOption;
    onChange(value: DateFilterOption): void;
}

export function DateFilter({
    options = ['D', 'W', 'M', 'ALL'],
    value,
    onChange,
}: DateFilterProps): JSX.Element {
    return <FilterList {...{ options, value, onChange }} />;
}
