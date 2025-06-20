// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota-sdk/client';
import { NFTMediaRenderer, useGetNFTDisplay } from '@iota/core';
import { FlexDirection } from '@/lib/ui/enums';
import { VisualAssetCard, VisualAssetType, type VisualAssetCardProps } from '@iota/apps-ui-kit';

interface AssetCardProps extends Pick<VisualAssetCardProps, 'onClick' | 'onIconClick' | 'icon'> {
    asset: IotaObjectData;
    flexDirection?: FlexDirection;
    disableVideoControls?: boolean;
}

export function VisualAssetTile({
    asset,
    onClick,
    onIconClick,
    icon,
    disableVideoControls,
}: AssetCardProps): React.JSX.Element | null {
    const { data: nftMeta } = useGetNFTDisplay(asset.objectId);

    if (!asset.display || !nftMeta || !nftMeta.imageUrl) {
        return null;
    }

    return (
        <VisualAssetCard
            renderAsset={
                <NFTMediaRenderer
                    src={nftMeta?.imageUrl ?? asset?.display?.data?.imageUrl ?? ''}
                    alt={nftMeta?.name ?? (asset?.display?.data?.name || 'NFT')}
                    disableVideoControls={disableVideoControls}
                />
            }
            assetTitle={nftMeta?.name ?? asset?.display?.data?.name}
            assetType={VisualAssetType.Image}
            isHoverable
            icon={icon}
            onClick={onClick}
            onIconClick={onIconClick}
        />
    );
}
