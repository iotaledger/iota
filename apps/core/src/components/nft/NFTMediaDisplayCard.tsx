// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetCard } from '@iota/apps-ui-kit';
import { NFTMediaRenderer } from './NFTMediaRenderer';

export interface NftMediaDisplayCardProps {
    src?: string | null;
    title?: string;
    className?: string;
    isHoverable?: boolean;
    icon?: React.ReactNode;
    onIconClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function NFTMediaDisplayCard({
    src,
    title,
    isHoverable,
    icon,
    onIconClick,
}: NftMediaDisplayCardProps) {
    const mediaSrc = src ? src.replace(/^ipfs:\/\//, 'https://ipfs.io/ipfs/') : '';

    return (
        <VisualAssetCard
            renderAsset={<NFTMediaRenderer src={mediaSrc} alt={title} />}
            assetTitle={title || 'NFT'}
            altText={title || 'NFT'}
            isHoverable={isHoverable}
            icon={icon}
            onIconClick={onIconClick}
        />
    );
}
