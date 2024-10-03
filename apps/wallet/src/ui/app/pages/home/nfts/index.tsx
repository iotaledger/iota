// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import {
    Alert,
    AlertStyle,
    AlertType,
    Loading,
    LoadingIndicator,
    NoData,
    PageTemplate,
} from '_components';
import { useGetNFTs } from '_src/ui/app/hooks/useGetNFTs';
import { useMultiGetObjects } from '@iota/core';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useHiddenAssets } from '../assets/HiddenAssetsProvider';
import NonVisualAssets from './NonVisualAssets';
import VisualAssets from './VisualAssets';
import { ButtonSegment, SegmentedButton, SegmentedButtonType } from '@iota/apps-ui-kit';
import HiddenAssets from './HiddenAssets';

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

function NftsPage() {
    const [selectedAssetCategory, setSelectedAssetCategory] = useState<AssetCategory | null>(null);
    const [isAssetsLoaded, setIsAssetsLoaded] = useState(false);
    const observerElem = useRef<HTMLDivElement | null>(null);

    const accountAddress = useActiveAddress();
    const { hiddenAssetIds } = useHiddenAssets();
    const {
        data: ownedAssets,
        hasNextPage,
        isLoading,
        isFetchingNextPage,
        error,
        isPending,
        isError,
    } = useGetNFTs(accountAddress);

    const { data } = useMultiGetObjects(hiddenAssetIds, {
        showDisplay: true,
        showType: true,
    });

    const isSpinnerVisible = isFetchingNextPage && hasNextPage;

    const filteredAssets = useMemo(() => {
        if (!ownedAssets) return [];

        switch (selectedAssetCategory) {
            case AssetCategory.Visual:
                return ownedAssets.visual;
            case AssetCategory.Other:
                return ownedAssets.other;
            default:
                return [];
        }
    }, [selectedAssetCategory, ownedAssets, hiddenAssetIds]);

    const filteredHiddenAssets = useMemo(() => {
        const hiddenNfts =
            data?.flatMap((data) => {
                return {
                    data: data.data,
                    display: data.data?.display?.data,
                };
            }) || [];

        return hiddenNfts
            ?.filter((nft) => nft.data && hiddenAssetIds.includes(nft?.data?.objectId))
            .sort((nftA, nftB) => {
                const nameA = nftA.display?.name || '';
                const nameB = nftB.display?.name || '';

                if (nameA < nameB) {
                    return -1;
                } else if (nameA > nameB) {
                    return 1;
                }
                return 0;
            });
    }, [hiddenAssetIds, data]);

    useEffect(() => {
        if (
            ownedAssets &&
            (selectedAssetCategory === null || // hasn't loaded a category
                (selectedAssetCategory === AssetCategory.Hidden &&
                    filteredHiddenAssets.length === 0) || // In the HiddenAssets page and no more items there
                (filteredAssets.length === 0 && selectedAssetCategory !== AssetCategory.Hidden)) // Other page and no items left
        ) {
            setIsAssetsLoaded(true);
            // Determine the default category based on available assets
            const defaultCategory =
                ownedAssets.visual.length > 0
                    ? AssetCategory.Visual
                    : ownedAssets.other.length > 0
                      ? AssetCategory.Other
                      : hiddenAssetIds.length > 0
                        ? AssetCategory.Hidden
                        : null;

            if (defaultCategory) {
                setSelectedAssetCategory(defaultCategory);
            } else {
                setSelectedAssetCategory(null);
            }
        }
    }, [ownedAssets, hiddenAssetIds, filteredHiddenAssets]);

    if (isLoading) {
        return (
            <div className="mt-1 flex w-full justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <PageTemplate title="Assets" isTitleCentered>
            <div className="flex h-full w-full flex-col items-start gap-md">
                {isError ? (
                    <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                        <Alert
                            title="Sync error (data might be outdated)"
                            supportingText={error?.message ?? 'An error occurred'}
                            style={AlertStyle.Default}
                            type={AlertType.Warning}
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
                                                    ? !hiddenAssetIds.length
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
                            </div>
                        </Loading>
                    </>
                )}
                <div ref={observerElem}>
                    {isSpinnerVisible ? (
                        <div className="mt-1 flex w-full justify-center">
                            <LoadingIndicator />
                        </div>
                    ) : null}
                </div>
            </div>
        </PageTemplate>
    );
}

export default NftsPage;
