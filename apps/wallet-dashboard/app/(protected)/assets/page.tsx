// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState } from 'react';
import cl from 'clsx';
import { usePageAssets, AssetCategory } from '@iota/core';
import {
    Panel,
    Title,
    TitleSize,
    LoadingIndicator,
    InfoBox,
    InfoBoxType,
    InfoBoxStyle,
    Chip,
} from '@iota/apps-ui-kit';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { Warning } from '@iota/ui-icons';

import { AssetTileLink, Loading } from '@/components';
import { AssetDialog } from '@/components/dialogs/assets';

const ASSET_CATEGORIES: { label: string; value: AssetCategory }[] = [
    {
        label: 'Visual',
        value: AssetCategory.Visual,
    },
    {
        label: 'Other',
        value: AssetCategory.Other,
    },
];

const ASSET_LAYOUT: Record<AssetCategory, string> = {
    [AssetCategory.Visual]:
        'grid-template-visual-assets grid max-h-[600px] gap-md overflow-auto py-sm',
    [AssetCategory.Other]: 'flex flex-col overflow-auto py-sm',
    [AssetCategory.Hidden]: 'flex flex-col overflow-auto py-sm',
};

export default function AssetsDashboardPage(): React.JSX.Element {
    const [selectedAsset, setSelectedAsset] = useState<IotaObjectData | null>(null);
    const account = useCurrentAccount();
    const accountAddress = account?.address || null;

    const {
        isPending,
        refetch,
        error,
        isError,

        ownedAssets,
        isAssetsLoaded,
        filteredAssets,
        selectedAssetCategory,
        setSelectedAssetCategory,
        observerElem,
        isSpinnerVisible,
    } = usePageAssets(accountAddress);

    function onAssetClick(asset: IotaObjectData) {
        setSelectedAsset(asset);
    }

    return (
        <Panel>
            <Title title="Assets" size={TitleSize.Medium} />
            <div className="px-lg">
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
                        Boolean(ownedAssets?.visual.length || ownedAssets?.other.length) ? (
                            <div className="flex flex-row items-center justify-start gap-xs py-xs">
                                {ASSET_CATEGORIES.map(({ value, label }) => (
                                    <Chip
                                        key={value}
                                        onClick={() => setSelectedAssetCategory(value)}
                                        label={label}
                                        selected={selectedAssetCategory === value}
                                        disabled={
                                            AssetCategory.Visual === value
                                                ? !ownedAssets?.visual.length
                                                : !ownedAssets?.other.length
                                        }
                                    />
                                ))}
                            </div>
                        ) : null}
                        <Loading loading={isPending}>
                            <div
                                className={cl(
                                    'max-h-[600px]',
                                    selectedAssetCategory && ASSET_LAYOUT[selectedAssetCategory],
                                )}
                            >
                                {filteredAssets.map((asset) => (
                                    <AssetTileLink
                                        key={asset.digest}
                                        asset={asset}
                                        type={selectedAssetCategory}
                                        onClick={onAssetClick}
                                    />
                                ))}
                                <div ref={observerElem}>
                                    {isSpinnerVisible ? (
                                        <div className="mt-1 flex h-full w-full justify-center">
                                            <LoadingIndicator />
                                        </div>
                                    ) : null}
                                </div>
                            </div>
                        </Loading>

                        {selectedAsset && (
                            <AssetDialog
                                onClose={() => setSelectedAsset(null)}
                                asset={selectedAsset}
                                refetchAssets={refetch}
                            />
                        )}
                    </>
                )}
            </div>
        </Panel>
    );
}
