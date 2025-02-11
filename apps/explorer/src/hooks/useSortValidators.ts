// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useState, useMemo } from 'react';
import { type IotaValidatorSummary } from '@iota/iota-sdk/client';

type SortKey = keyof IotaValidatorSummary;

export function useSortValidators(validators: IotaValidatorSummary[]) {
    const [sortBy, setSortBy] = useState<SortKey>('stakingPoolIotaBalance');
    const [isAscending, setIsAscending] = useState<boolean>(false);

    const parseValue = (value: unknown): string | number | bigint => {
        if (value === undefined || value === null) return 0;

        if (typeof value === 'object') {
            return 0;
        }

        if (typeof value === 'string') {
            if (!isNaN(Number(value))) {
                return value.includes('.') ? parseFloat(value) : BigInt(value);
            }
            return value.trim();
        }

        return value as string | number | bigint;
    };

    const compareValues = (a: unknown, b: unknown, ascending: boolean): number => {
        const valueA = parseValue(a);
        const valueB = parseValue(b);

        if (typeof valueA === 'string' && typeof valueB === 'string') {
            return ascending ? valueA.localeCompare(valueB) : valueB.localeCompare(valueA);
        }

        if (typeof valueA === 'number' && typeof valueB === 'number') {
            return ascending ? valueA - valueB : valueB - valueA;
        }

        if (typeof valueA === 'bigint' && typeof valueB === 'bigint') {
            return ascending ? (valueA < valueB ? -1 : 1) : valueA > valueB ? -1 : 1;
        }

        return 0;
    };

    const sortedValidators = useMemo(
        () => [...validators].sort((a, b) => compareValues(a[sortBy], b[sortBy], isAscending)),
        [validators, sortBy, isAscending],
    );

    return {
        sortBy,
        setSortBy,
        isAscending,
        setIsAscending,
        sortedValidators,
    };
}
