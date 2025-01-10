// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useState, useMemo, useRef, useEffect } from 'react';
import { useGetNFTs, HiddenAssets, useOnScreen } from '../..';

export enum AssetCategory {
    Visual = 'Visual',
    Other = 'Other',
    Hidden = 'Hidden',
}

export function usePageAssets(address: string | null, hiddenAssets?: HiddenAssets) {
    const [selectedAssetCategory, setSelectedAssetCategory] = useState<AssetCategory | null>(null);
    const observerElem = useRef<HTMLDivElement | null>(null);
    const { isIntersecting } = useOnScreen(observerElem);
    const {
        data: ownedAssets,
        hasNextPage,
        isFetchingNextPage,
        fetchNextPage,
        error,
        isPending,
        isError,
        isFetching,
        refetch,
    } = useGetNFTs(address, hiddenAssets);

    const isAssetsLoaded = !!ownedAssets;
    const isSpinnerVisible = isFetchingNextPage && hasNextPage;

    const filteredAssets = (() => {
        if (!ownedAssets) return [];
        switch (selectedAssetCategory) {
            case AssetCategory.Visual:
                return ownedAssets.visual;
            case AssetCategory.Other:
                return ownedAssets.other;
            default:
                return [];
        }
    })();

    const filteredHiddenAssets = useMemo(() => {
        return (
            ownedAssets?.hidden
                .flatMap((data) => {
                    return {
                        data: data,
                        display: data?.display?.data,
                    };
                })
                .sort((nftA, nftB) => {
                    const nameA = nftA.display?.name || '';
                    const nameB = nftB.display?.name || '';

                    if (nameA < nameB) {
                        return -1;
                    } else if (nameA > nameB) {
                        return 1;
                    }
                    return 0;
                }) ?? []
        );
    }, [ownedAssets]);

    // Fetch the next page if the user scrolls to the bottom of the page
    useEffect(() => {
        if (isIntersecting && hasNextPage && !isFetchingNextPage) {
            fetchNextPage();
        }
    }, [isIntersecting, fetchNextPage, hasNextPage, isFetchingNextPage]);

    // select the default category if no category is selected and assets are loaded
    useEffect(() => {
        let computeSelectedCategory = false;
        if (
            (selectedAssetCategory === AssetCategory.Visual && ownedAssets?.visual.length === 0) ||
            (selectedAssetCategory === AssetCategory.Other && ownedAssets?.other.length === 0) ||
            (selectedAssetCategory === AssetCategory.Hidden && ownedAssets?.hidden.length === 0) ||
            !selectedAssetCategory
        ) {
            computeSelectedCategory = true;
        }
        if (computeSelectedCategory && ownedAssets) {
            const defaultCategory =
                ownedAssets.visual.length > 0
                    ? AssetCategory.Visual
                    : ownedAssets.other.length > 0
                      ? AssetCategory.Other
                      : ownedAssets.hidden.length > 0
                        ? AssetCategory.Hidden
                        : null;
            setSelectedAssetCategory(defaultCategory);
        }
    }, [ownedAssets]);

    // Fetch the next page if there are no visual assets, other + hidden assets are present in multiples of 50, and there are more pages to fetch
    useEffect(() => {
        if (
            hasNextPage &&
            ownedAssets?.visual.length === 0 &&
            ownedAssets?.other.length + ownedAssets?.hidden.length > 0 &&
            (ownedAssets.other.length + ownedAssets.hidden.length) % 50 === 0 &&
            !isFetchingNextPage
        ) {
            fetchNextPage();
            setSelectedAssetCategory(null);
        }
    }, [hasNextPage, ownedAssets, isFetchingNextPage]);

    return {
        // reexport from useGetNFTs
        ownedAssets,
        hasNextPage,
        isFetchingNextPage,
        fetchNextPage,
        error,
        isPending,
        isError,
        isFetching,
        refetch,

        isAssetsLoaded,
        filteredAssets,
        filteredHiddenAssets,
        selectedAssetCategory,
        setSelectedAssetCategory,
        observerElem,
        isSpinnerVisible,
    };
}
