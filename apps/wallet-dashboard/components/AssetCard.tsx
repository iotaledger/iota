// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AssetCategory } from '@/lib/enums';
import { ArrowTopRight, VisibilityOff } from '@iota/ui-icons';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardType,
    VisualAssetCard,
    VisualAssetType,
} from '@iota/apps-ui-kit';
import { useGetNFTMeta } from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';

interface AssetCardProps {
    asset: IotaObjectData;
    type: AssetCategory;
    onClick?: () => void;
}

export function AssetCard({ asset, type, onClick }: AssetCardProps): React.JSX.Element {
    return type === AssetCategory.Visual ? (
        <VisualAsset asset={asset} icon={<VisibilityOff />} onClick={onClick} />
    ) : (
        <NonVisualAssetCard key={asset.digest} asset={asset} onClick={onClick} />
    );
}

type NonVisualAssetCardProps = {
    asset: IotaObjectData;
} & Pick<React.ComponentProps<typeof Card>, 'onClick'>;

export function NonVisualAssetCard({ asset, onClick }: NonVisualAssetCardProps): React.JSX.Element {
    const { address, module, name } = parseStructTag(asset.type!);
    return (
        <Card type={CardType.Default} isHoverable onClick={onClick}>
            <CardBody
                title={formatAddress(asset.objectId!)}
                subtitle={`${formatAddress(address)}::${module}::${name}`}
                isTextTruncated
            />
            <CardAction type={CardActionType.Link} icon={<ArrowTopRight />} />
        </Card>
    );
}

type VisualAssetProps = {
    asset: IotaObjectData;
} & Pick<React.ComponentProps<typeof VisualAssetCard>, 'onClick' | 'onIconClick' | 'icon'>;

export function VisualAsset({
    asset,
    icon,
    onClick,
    onIconClick,
}: VisualAssetProps): React.JSX.Element | null {
    const { data: nftMeta } = useGetNFTMeta(asset.objectId);
    return asset.display && nftMeta && nftMeta.imageUrl ? (
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
    ) : null;
}
