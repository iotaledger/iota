// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ButtonSegment,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';
import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { Loading, NoData, PageTemplate } from '_components';
import { useGetNFTs } from '_src/ui/app/hooks/useGetNFTs';
import { useEffect, useMemo, useRef, useState } from 'react';
import { HiddenAssets } from './HiddenAssets';
import { NonVisualAssets } from './NonVisualAssets';
import { VisualAssets } from './VisualAssets';
import { Warning } from '@iota/ui-icons';
import { useOnScreen } from '@iota/core';

enum AssetCategory {
    Visual = 'Visual',
    Other = 'Other',
    Hidden = 'Hidden',
}

const ASSET_CATEGORIES = [
    {
        label: 'Visual',
        value: AssetCategory.Visual,
    },
    {
        label: 'Other',
        value: AssetCategory.Other,
    },
    {
        label: 'Hidden',
        value: AssetCategory.Hidden,
    },
];

export function NftsPage() {
    const [selectedAssetCategory, setSelectedAssetCategory] = useState<AssetCategory | null>(null);
    const observerElem = useRef<HTMLDivElement | null>(null);
    const { isIntersecting } = useOnScreen(observerElem);

    const accountAddress = useActiveAddress();
    const {
        data: ownedAssets,
        hasNextPage,
        isFetchingNextPage,
        fetchNextPage,
        error,
        isPending,
        isError,
    } = useGetNFTs(accountAddress);

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

    useEffect(() => {
        if (isIntersecting && hasNextPage && !isFetchingNextPage) {
            fetchNextPage();
        }
    }, [isIntersecting, fetchNextPage, hasNextPage, isFetchingNextPage]);

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

    useEffect(() => {
        // Fetch the next page if there are no visual assets, other + hidden assets are present in multiples of 50, and there are more pages to fetch
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

    return (
        <PageTemplate title="Assets" isTitleCentered>
            <div className="flex h-full w-full flex-col items-start gap-md">
                {isError ? (
                    <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                        <InfoBox
                            type={InfoBoxType.Error}
                            title="Sync error (data might be outdated)"
                            supportingText={error?.message ?? 'An error occurred'}
                            icon={<Warning />}
                            style={InfoBoxStyle.Default}
                        />
                    </div>
                ) : (
                    <>
                        {isAssetsLoaded &&
                            Boolean(filteredAssets.length || filteredHiddenAssets.length) && (
                                <SegmentedButton type={SegmentedButtonType.Filled}>
                                    {ASSET_CATEGORIES.map(({ label, value }) => (
                                        <ButtonSegment
                                            key={value}
                                            onClick={() => setSelectedAssetCategory(value)}
                                            label={label}
                                            selected={selectedAssetCategory === value}
                                            disabled={
                                                AssetCategory.Hidden === value
                                                    ? !filteredHiddenAssets.length
                                                    : AssetCategory.Visual === value
                                                      ? !ownedAssets?.visual.length
                                                      : !ownedAssets?.other.length
                                            }
                                        />
                                    ))}
                                </SegmentedButton>
                            )}
                        <Loading loading={isPending}>
                            <div className="flex h-full w-full flex-col">
                                {selectedAssetCategory === AssetCategory.Visual ? (
                                    <VisualAssets items={filteredAssets} />
                                ) : selectedAssetCategory === AssetCategory.Other ? (
                                    <NonVisualAssets items={filteredAssets} />
                                ) : selectedAssetCategory === AssetCategory.Hidden ? (
                                    <HiddenAssets items={filteredHiddenAssets} />
                                ) : (
                                    <NoData message="No assets found yet." />
                                )}
                                <div ref={observerElem}>
                                    {isSpinnerVisible ? (
                                        <div className="mt-1 flex w-full justify-center">
                                            <LoadingIndicator />
                                        </div>
                                    ) : null}
                                </div>
                            </div>
                        </Loading>
                    </>
                )}
            </div>
        </PageTemplate>
    );
}
