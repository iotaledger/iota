// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import {
    NftImage,
    isKioskOwnerToken,
    useGetNFTDisplay,
    useGetObject,
    useKioskClient,
    KioskTile,
} from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import { cva } from 'class-variance-authority';
import type { VariantProps } from 'class-variance-authority';
import { useResolveVideo, useActiveAddress } from '_hooks';

const nftDisplayCardStyles = cva('flex flex-nowrap items-center w-full h-full relative', {
    variants: {
        isHoverable: {
            true: 'group',
        },
        wideView: {
            true: 'gap-2 flex-row-reverse justify-between',
            false: '',
        },
    },
    defaultVariants: {
        wideView: false,
    },
});

export interface NFTDisplayCardProps extends VariantProps<typeof nftDisplayCardStyles> {
    objectId: string;
    isLocked?: boolean;
    icon?: React.ReactNode;
    onIconClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function NFTDisplayCard({
    objectId,
    wideView,
    isHoverable,
    icon,
    onIconClick,
}: NFTDisplayCardProps) {
    const { data: objectData } = useGetObject(objectId);
    const { data: nftMeta, isPending } = useGetNFTDisplay(objectId);
    const nftName = nftMeta?.name || formatAddress(objectId);
    const nftImageUrl = nftMeta?.imageUrl || '';
    const video = useResolveVideo(objectData);
    const kioskClient = useKioskClient();
    const isOwnerToken = isKioskOwnerToken(kioskClient.network, objectData);
    const address = useActiveAddress();

    return (
        <div className={nftDisplayCardStyles({ isHoverable, wideView })}>
            <Loading loading={isPending}>
                <div className="flex w-full max-w-[172px] flex-col items-center gap-xs">
                    {objectData?.data && isOwnerToken ? (
                        <KioskTile object={objectData} address={address} />
                    ) : (
                        <NftImage
                            title={nftName}
                            src={nftImageUrl}
                            isHoverable={isHoverable ?? false}
                            video={video}
                            icon={icon}
                            onIconClick={onIconClick}
                        />
                    )}
                    {wideView && (
                        <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                            {nftName}
                        </span>
                    )}
                </div>
            </Loading>
        </div>
    );
}
