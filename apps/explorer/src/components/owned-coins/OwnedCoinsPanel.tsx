// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useElementDimensions, useGetCoins, useOnScreen } from '@iota/core';
import clsx from 'clsx';
import { useEffect, useRef } from 'react';
import { CoinItem } from './CoinItem';
import { LoadingIndicator } from '@iota/apps-ui-kit';

const MIN_CONTAINER_WIDTH_SIZE = 500;

type CoinsPanelProps = {
    coinType: string;
    id: string;
};

export function CoinsPanel({ coinType, id }: CoinsPanelProps): JSX.Element {
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

    const multiCols = containerWidth > MIN_CONTAINER_WIDTH_SIZE;

    return (
        <div className="max-h-[230px] overflow-auto">
            <div className="flex flex-col flex-wrap gap-xs firefox:pr-xs" ref={coinsSectionRef}>
                {data &&
                    data.pages.map((page) =>
                        page.data.map((coin) => (
                            <div
                                key={coin.coinObjectId}
                                className={clsx('w-full', multiCols && 'basis-1/3')}
                            >
                                <CoinItem coin={coin} />
                            </div>
                        )),
                    )}
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
