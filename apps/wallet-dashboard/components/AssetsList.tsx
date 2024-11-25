// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AssetCategory } from '@/lib/enums';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetTileLink } from '@/components';
import { LoadingIndicator } from '@iota/apps-ui-kit';
import { useEffect, useRef } from 'react';
import { useOnScreen } from '@iota/core';
import cl from 'clsx';

interface AssetListProps {
    assets: IotaObjectData[];
    selectedCategory: AssetCategory;
    hasNextPage?: boolean;
    isFetchingNextPage?: boolean;
    fetchNextPage?: () => void;
}

const ASSET_LAYOUT: Record<AssetCategory, string> = {
    [AssetCategory.Visual]:
        'grid-template-visual-assets grid max-h-[600px] gap-md overflow-auto py-sm',
    [AssetCategory.Other]: 'flex flex-col overflow-auto py-sm',
};

export function AssetList({
    assets,
    selectedCategory,
    hasNextPage = false,
    isFetchingNextPage = false,
    fetchNextPage,
}: AssetListProps): React.JSX.Element {
    const observerElem = useRef<HTMLDivElement | null>(null);
    const { isIntersecting } = useOnScreen(observerElem);
    const isSpinnerVisible = isFetchingNextPage && hasNextPage;

    useEffect(() => {
        if (isIntersecting && hasNextPage && !isFetchingNextPage && fetchNextPage) {
            fetchNextPage();
        }
    }, [isIntersecting, fetchNextPage, hasNextPage, isFetchingNextPage]);

    return (
        <div className={cl('max-h-[600px]', ASSET_LAYOUT[selectedCategory])}>
            {assets.map((asset) => (
                <AssetTileLink key={asset.digest} asset={asset} type={selectedCategory} />
            ))}
            <div ref={observerElem}>
                {isSpinnerVisible ? (
                    <div className="mt-1 flex h-full w-full justify-center">
                        <LoadingIndicator />
                    </div>
                ) : null}
            </div>
        </div>
    );
}
