// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import {
    NFTMediaDisplayCard,
    isKioskOwnerToken,
    useGetNFTDisplay,
    useGetObject,
    useKioskClient,
    KioskTile,
} from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import { cva } from 'class-variance-authority';
import type { VariantProps } from 'class-variance-authority';
import { useActiveAddress } from '_hooks';

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
    const kioskClient = useKioskClient();
    const isOwnerToken = isKioskOwnerToken(kioskClient.network, objectData);
    const address = useActiveAddress();

    return (
        <div className={nftDisplayCardStyles({ isHoverable, wideView })}>
            <Loading loading={isPending}>
                <div className="flex w-full flex-col items-center gap-xs">
                    <div className="w-full max-w-[172px]">
                        {objectData?.data && isOwnerToken ? (
                            <KioskTile object={objectData} address={address} />
                        ) : (
                            <NFTMediaDisplayCard
                                title={nftName}
                                src={nftMeta?.imageUrl}
                                isHoverable={isHoverable ?? false}
                                icon={icon}
                                onIconClick={onIconClick}
                            />
                        )}
                    </div>
                    {wideView && (
                        <span className="max-w-full break-words text-title-lg text-iota-neutral-10 dark:text-iota-neutral-92">
                            {nftName}
                        </span>
                    )}
                </div>
            </Loading>
        </div>
    );
}
