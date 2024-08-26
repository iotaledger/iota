// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading, NftImage } from '_components';
import { isKioskOwnerToken, useGetNFTMeta, useGetObject, useKioskClient } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import { cva } from 'class-variance-authority';
import type { VariantProps } from 'class-variance-authority';
import { useResolveVideo } from '../../hooks/useResolveVideo';
import { Kiosk } from './Kiosk';

const nftDisplayCardStyles = cva('flex flex-nowrap items-center h-full relative', {
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
}

export function NFTDisplayCard({ objectId, wideView, isHoverable }: NFTDisplayCardProps) {
    const { data: objectData } = useGetObject(objectId);
    const { data: nftMeta, isPending } = useGetNFTMeta(objectId);
    const nftName = nftMeta?.name || formatAddress(objectId);
    const nftImageUrl = nftMeta?.imageUrl || '';
    const video = useResolveVideo(objectData);
    const kioskClient = useKioskClient();
    const isOwnerToken = isKioskOwnerToken(kioskClient.network, objectData);

    return (
        <div className={nftDisplayCardStyles({ isHoverable, wideView })}>
            <Loading loading={isPending}>
                <div className="flex flex-col justify-center gap-sm text-center">
                    {objectData?.data && isOwnerToken ? (
                        <Kiosk object={objectData} />
                    ) : (
                        <NftImage
                            title={nftName}
                            src={nftImageUrl}
                            isHoverable={isHoverable ?? false}
                            video={video}
                        />
                    )}
                    {wideView && <span className="text-title-lg text-neutral-10">{nftName}</span>}
                </div>
            </Loading>
        </div>
    );
}
