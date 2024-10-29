// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { IotaObjectData } from '@iota/iota-sdk/client';
import React from 'react';
import { useGetNFTMeta } from '@iota/core';
import { FlexDirection } from '@/lib/ui/enums';
import { VisualAssetCard, VisualAssetType } from '@iota/apps-ui-kit';

type AssetCardProps = {
    asset: IotaObjectData;
    flexDirection?: FlexDirection;
} & Pick<React.ComponentProps<typeof VisualAssetCard>, 'onClick' | 'onIconClick' | 'icon'>;

function AssetCard({ asset, onClick, onIconClick, icon }: AssetCardProps): React.JSX.Element {
    const { data: nftMeta } = useGetNFTMeta(asset.objectId);
    return (
        <>
            {asset.display && nftMeta && nftMeta.imageUrl && (
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
            )}
        </>
    );
}

export default AssetCard;
