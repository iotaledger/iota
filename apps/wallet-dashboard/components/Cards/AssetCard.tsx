// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota-sdk/client';
import React from 'react';
import { useGetNFTMeta } from '@iota/core';
import { FlexDirection } from '@/lib/ui/enums';
import { VisualAssetCard, VisualAssetType, type VisualAssetCardProps } from '@iota/apps-ui-kit';

interface AssetCardProps extends Pick<VisualAssetCardProps, 'onClick' | 'onIconClick' | 'icon'> {
    asset: IotaObjectData;
    flexDirection?: FlexDirection;
}

export function AssetCard({
    asset,
    onClick,
    onIconClick,
    icon,
}: AssetCardProps): React.JSX.Element | null {
    const { data: nftMeta } = useGetNFTMeta(asset.objectId);

    if (!asset.display || !nftMeta || !nftMeta.imageUrl) {
        return null;
    }

    return (
        <VisualAssetCard
            assetSrc={nftMeta?.imageUrl ?? asset?.display?.data?.imageUrl ?? ''}
            assetTitle={nftMeta?.name ?? asset?.display?.data?.name}
            assetType={VisualAssetType.Image}
            altText={nftMeta?.name ?? (asset?.display?.data?.name || 'NFT')}
            isHoverable
            icon={icon}
            onClick={onClick}
            onIconClick={onIconClick}
        />
    );
}
