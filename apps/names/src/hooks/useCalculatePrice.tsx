// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SelectOption } from '@iota/apps-ui-kit';
import { useQueries } from '@tanstack/react-query';
import React from 'react';

import { useIotaNamesClient } from '@/contexts';
import { formatNanosToIota } from '@/lib/utils';

import { queryKey } from './queryKey';

export function useCalculatePrice(
    name: string,
    maxYears: number,
    isRegistration: boolean = true,
): SelectOption[] {
    const { iotaNamesClient } = useIotaNamesClient();

    const yearsArray = Array.from({ length: maxYears }, (_, i) => i + 1);

    const pricesQueries = useQueries({
        queries: yearsArray.map((year) => ({
            queryKey: [...queryKey.renewalPrice(name, year), isRegistration],
            queryFn: async () => {
                return await iotaNamesClient.calculatePrice({
                    name,
                    years: year,
                    isRegistration,
                });
            },
            enabled: !!iotaNamesClient && !!name && maxYears > 0,
        })),
    });

    const options: SelectOption[] = yearsArray.map((year, idx) => {
        const labelYears = `${year} Year${year > 1 ? 's' : ''}`;
        const priceNanos = pricesQueries[idx]?.data;
        const priceIota = priceNanos ? formatNanosToIota(priceNanos) : null;
        return {
            id: String(year),
            renderLabel: () => (
                <div className="flex w-full items-center justify-between">
                    <span className="text-body-lg">{labelYears}</span>
                    {priceIota && (
                        <span className="rounded-full bg-names-neutral-10 px-sm py-xs text-label-md text-names-neutral-100">
                            {priceIota}
                        </span>
                    )}
                </div>
            ),
        };
    });

    return options;
}
