// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface DisplayStatsProps {
    title: string;
    balance: bigint | number | string;
}

export function StatsDetail({ balance, title }: DisplayStatsProps) {
    const [formatted, symbol] = useFormatCoin(balance, IOTA_TYPE_ARG);

    return (
        <div className="flex h-[96px] flex-1 flex-col justify-between rounded-xl bg-neutral-96 p-md">
            <div className="text-label-sm text-neutral-10">{title}</div>

            <div className="flex items-baseline gap-xxs">
                <div className="text-title-md text-neutral-10">{formatted}</div>
                <div className="text-label-md text-neutral-10 opacity-40">{symbol}</div>
            </div>
        </div>
    );
}
