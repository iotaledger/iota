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
import { useActiveAddress } from '_hooks';
import { Loading, PageTemplate } from '_components';
import { HiddenAssets } from './HiddenAssets';
import { NonVisualAssets } from './NonVisualAssets';
import { VisualAssets } from './VisualAssets';
import { Warning } from '@iota/apps-ui-icons';
import { useHiddenAssets, usePageAssets, AssetCategory, NoData } from '@iota/core';

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
    const accountAddress = useActiveAddress();
    const { hiddenAssets } = useHiddenAssets();

    const {
        isPending,
        isAssetsLoaded,
        isError,
        error,
        ownedAssets,
        filteredAssets,
        filteredHiddenAssets,
        selectedAssetCategory,
        setSelectedAssetCategory,
        isSpinnerVisible,
        observerElem,
    } = usePageAssets(accountAddress, hiddenAssets);

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
                                    <NoData message="No assets found yet." displayImage />
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
