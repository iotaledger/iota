// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useElementDimensions, useGetCoins, useOnScreen } from '@iota/core';
import clsx from 'clsx';
import { useEffect, useRef, useMemo } from 'react';
import { CoinItem } from './CoinItem';
import { LoadingIndicator } from '@iota/apps-ui-kit';
import { SortField, SortOrder } from './OwnedCoins';

const MIN_CONTAINER_WIDTH_SIZE = 500;

type CoinsPanelProps = {
    coinType: string;
    id: string;
    sortField: SortField;
    sortOrder: SortOrder;
};

export function CoinsPanel({ coinType, id, sortField, sortOrder }: CoinsPanelProps): JSX.Element {
    const containerRef = useRef(null);
    const coinsSectionRef = useRef(null);
    const { isIntersecting } = useOnScreen(containerRef);
    const { data, isPending, isFetchingNextPage, fetchNextPage, hasNextPage } = useGetCoins(
        coinType,
        id,
    );
    const [_, containerWidth] = useElementDimensions(coinsSectionRef);

    const isSpinnerVisible = (hasNextPage && isFetchingNextPage) || isPending;

    useEffect(() => {
        if (isIntersecting && hasNextPage && !isFetchingNextPage) {
            fetchNextPage();
        }
    }, [isIntersecting, hasNextPage, isFetchingNextPage, fetchNextPage]);

    const sortedCoins = useMemo(() => {
        if (!data) return [];

        const allCoins = data.pages.flatMap((page) => page.data);

        return allCoins.sort((a, b) => {
            if (sortField === SortField.Balance) {
                const diff = BigInt(a.balance) - BigInt(b.balance);
                const diffComparison = diff === 0n ? 0 : diff > 0n ? 1 : -1;
                return sortOrder === SortOrder.Asc ? diffComparison : -diffComparison;
            } else {
                const comparison = a.coinObjectId.localeCompare(b.coinObjectId);
                return sortOrder === SortOrder.Asc ? comparison : -comparison;
            }
        });
    }, [data, sortField, sortOrder]);

    const multiCols = containerWidth > MIN_CONTAINER_WIDTH_SIZE;

    return (
        <div className="max-h-[230px] overflow-auto">
            <div className="flex flex-col flex-wrap gap-xs firefox:pr-xs" ref={coinsSectionRef}>
                {sortedCoins.map((coin) => (
                    <div
                        key={coin.coinObjectId}
                        className={clsx('w-full', multiCols && 'basis-1/3')}
                    >
                        <CoinItem coin={coin} />
                    </div>
                ))}
            </div>
            <div className="flex justify-center" ref={containerRef}>
                {isSpinnerVisible && (
                    <div className="mt-5 flex">
                        <LoadingIndicator />
                    </div>
                )}
            </div>
        </div>
    );
}
