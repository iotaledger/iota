// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonSegment, SegmentedButton, SegmentedButtonType } from '@iota/apps-ui-kit';
import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { Alert, Loading, LoadingIndicator, NoData, PageTemplate } from '_components';
import { useGetNFTs } from '_src/ui/app/hooks/useGetNFTs';
import { useEffect, useMemo, useRef, useState } from 'react';
import HiddenAssets from './HiddenAssets';
import NonVisualAssets from './NonVisualAssets';
import VisualAssets from './VisualAssets';

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
    const {
        data: ownedAssets,
        hasNextPage,
        isLoading,
        isFetchingNextPage,
        error,
        isPending,
        isError,
    } = useGetNFTs(accountAddress);

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
    }, [selectedAssetCategory, ownedAssets]);

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
        if (ownedAssets) {
            setIsAssetsLoaded(true);
        }
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
                    {isError ? (
                        <Alert>
                            <div>
                                <strong>Sync error (data might be outdated)</strong>
                            </div>
                            <small>{(error as Error).message}</small>
                        </Alert>
                    ) : null}
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
